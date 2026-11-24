# MLAI Bridge UI: recommended stack (late 2026)

Scope: a Tauri 2 app on Windows with a Vite + React 19 + TS frontend in `ui/`. It has three screens (Proxy, Usage, Accounts), a request log with a detail drawer, and polling for live stats.
All versions below are the `latest` npm dist-tags as fetched on 2026-11-24 (`registry.npmjs.org/<pkg>/latest`). Sizes are bundlephobia min+gzip for the whole package (`bundlephobia.com/api/size`). Tree-shaken real cost is lower for component libraries.

## 0. Repo starting point (observed)

- `ui/package.json`: React 19.2, Vite 8, `@vitejs/plugin-react` 6, TS 5.9, and no other dependencies. Styling is one hand-written `styles.css`.
- `crates/app/tauri.conf.json`:
  - `"withGlobalTauri": true`
  - `"csp": null`, so no CSP at all
  - `"windows": []`, because the window is built in Rust (`gui.rs:469`, `WebviewWindowBuilder::new(app, "main", …)`)
  - NSIS, `currentUser`
- Tauri itself: `tauri = { version = "2", features = [] }`. No `tray-icon` feature, no plugins, and no `capabilities/` directory.
- `ui/src/tauri.ts` calls `window.__TAURI__.core.invoke` and has a browser preview stub. Keep this pattern; it makes UI work possible without Rust.

---

## 1. The recommended stack

| Concern | Package(s) (version) | Why |
|---|---|---|
| Styling | `tailwindcss` 4.3.5, `@tailwindcss/vite` 4.3.5, `tw-animate-css` 1.4.0 | CSS-first config, tokens as CSS variables, no runtime. It is the base shadcn, HeroUI and Tremor all assume. Tailwind v4 needs Chrome ≥111 ([compat](https://tailwindcss.com/docs/compatibility)), which Evergreen WebView2 meets. |
| Components | **shadcn/ui on Base UI**: `shadcn` CLI 4.23.0 → `@base-ui/react` 1.9.0, `class-variance-authority` 0.7.1, `lucide-react` 1.78.0 | Base UI became shadcn's default in July 2026. Base UI is at 1.6+ and "stable", Radix is still supported, and every component ships for both ([changelog](https://ui.shadcn.com/docs/changelog/2026-07-base-ui-default)). The code is copied into the repo, so we own it and there is no theme API to fight. The catalogue covers every screen we have (details below). |
| Data fetching | `@tanstack/react-query` 5.108.4 (13 KB gz) | Wraps `invoke()` as `queryFn`. Gives built-in `refetchInterval` polling, cache, dedupe and mutations with invalidation ([polling guide](https://tanstack.com/query/latest/docs/framework/react/guides/polling)). |
| Request log table | `@tanstack/react-table` 9.3.1 + `@tanstack/react-virtual` 3.14.28 | shadcn's Data Table is built on TanStack Table **v9** ([docs](https://ui.shadcn.com/docs/components/base/data-table)). v9 is tree-shaken per feature, uses up to 86% less heap than v8 and has a faster row pipeline ([v9 blog](https://tanstack.com/blog/announcing-tanstack-table-v9)). Virtualize because the log grows without bound; Vercel's guidelines say "MUST: Virtualize large lists (>50 items)" ([AGENTS.md](https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/AGENTS.md)). |
| Charts | shadcn `chart` component → `recharts` 3.11 | Its colours come from `--chart-1..5` CSS vars and follow `.dark`, so charts match the theme for free ([comparison](https://dev.to/malikabimanyu/best-shadcn-chart-components-and-collections-compared-honestly-jbb), [shadcn chart](https://ui.shadcn.com/docs/components/base/chart)). Recharts is big (177 KB gz), so load the Usage route with `React.lazy`. |
| Body viewer | `@uiw/react-codemirror` 4.25.12 + `@codemirror/lang-json` 6.0.2, read-only | Handles multi-MB request bodies with code folding and Ctrl+F search. About 53 KB gz versus Monaco's workers and CSP friction. Yaak (19.5k★ Tauri API client) uses CodeMirror for exactly this ([yaak package.json](https://github.com/mountain-loop/yaak)). |
| Command palette | shadcn `command` → `cmdk` 1.1.1 | shadcn's Command wraps cmdk ([docs](https://ui.shadcn.com/docs/components/base/command)). Adds Ctrl+K for start/stop proxy, switch account, copy base URL and jump to a request. |
| Toasts | shadcn `toast` (Base UI Toast) | The Base UI variant of shadcn has its own `toast` with `toast.promise` ([docs](https://ui.shadcn.com/docs/components/base/toast)), so no extra dependency is needed. Use `sonner` 2.0.8 (9 KB gz) only if you pick `-b radix`; shadcn's Radix/Aria docs list Sonner as their toast. |
| Theming / dark mode | No package: a 20-line `ThemeProvider` from [shadcn's Vite dark-mode guide](https://ui.shadcn.com/docs/dark-mode/vite) plus Tauri `getCurrentWindow().theme()/onThemeChanged` | `next-themes` targets Next; the Vite guide already ships the tiny provider. Default to "system". |
| Fonts | `@fontsource-variable/geist` 5.3.0 (+ Geist Mono) | Bundled locally, so it works offline and needs no CSP font host. Gives the Vercel/Linear look. Use `tabular-nums` on numbers. |
| Resizable drawer/split | shadcn `resizable` (→ `react-resizable-panels` 4.24.1) | Lets the request log and detail sit side by side on wide windows. Optional; a shadcn `Sheet` is enough to start. |

### Setup commands

```bash
cd ui
npm i -D tailwindcss @tailwindcss/vite
npx shadcn@latest init          # Base UI is the default; Vite template
npx shadcn@latest add button card badge input field select tabs table data-table sheet dialog alert-dialog \
  dropdown-menu tooltip command toast chart skeleton empty kbd switch separator scroll-area sidebar spinner
npm i @tanstack/react-query @tanstack/react-virtual @uiw/react-codemirror @codemirror/lang-json @fontsource-variable/geist
```

(`data-table` brings in `@tanstack/react-table`; `chart` brings in `recharts`; `command` brings in `cmdk`.)

### shadcn components per screen

- **Proxy**
  - Status pill: `badge` with dot, colour plus text. Vercel: "status cues not color-only".
  - Start/stop: `button` with `spinner`; keep the label while loading.
  - Connect-a-client: `card` + `input-group` + copy button + `kbd`.
  - Config form: `field`, `input`, `select`, `switch`.
  - Model list: `table`.
- **Usage**
  - Stat cards: `card`; numbers use `tabular-nums`.
  - Per-model and per-account breakdowns: `chart` (bar/area) + `table`.
  - Request log: `data-table` + virtualizer, with filters (`select`/`combobox`) and an auto-refresh `switch`.
  - Detail: `sheet` (drawer) with `tabs` (Request / Response / Timing) and CodeMirror.
  - Empty states: `empty`.
- **Accounts**
  - List: `item` or `table`.
  - Add token: `dialog` + `field`.
  - Remove: `alert-dialog`. Vercel says "MUST: Confirm destructive actions".
  - Set active: `dropdown-menu`.
- **Navigation:** `sidebar` (collapsible), or `tabs` if three items is all we'll ever have. [INFERENCE] With 3 screens, a slim sidebar or top tabs in the custom titlebar is enough.

### TanStack Query + `invoke` pattern

```ts
// ui/src/queries.ts
import { QueryClient, useQuery } from "@tanstack/react-query";
import { invoke } from "./tauri"; // existing typed wrapper + preview stub
export const qc = new QueryClient({
  defaultOptions: { queries: { networkMode: "always", retry: false, refetchOnWindowFocus: false } },
});
export const useUsage = (live: boolean) =>
  useQuery({ queryKey: ["usage"], queryFn: () => invoke<Usage>("usage"),
             refetchInterval: live ? 2000 : false });
```

- Set `networkMode: "always"`. The TanStack docs say embedded WebViews may not fire `online`/`offline` reliably, and `invoke` never needs the network ([polling guide](https://tanstack.com/query/latest/docs/framework/react/guides/polling)).
- `refetchInterval` can be a function, which lets us pause polling when the auto-refresh switch is off or the proxy is stopped. Note that each observer runs its own timer, so poll from one component per key.
- **Better than polling** for the request log: have Rust `app.emit("request-logged", rowId)` and call `listen("request-logged", () => qc.invalidateQueries({queryKey:["rows"]}))` in JS. Then polling only remains as a fallback. Events need `core:event:default`, which is in `core:default`.
- Use mutations (`useMutation`) for start/stop, save config and add/remove account, then `invalidateQueries`.

### Virtualized log

- Use `useVirtualizer({ count: rows.length, getScrollElement, estimateSize: () => 32, overscan: 10 })` and render only `getVirtualItems()` inside a fixed-height scroll container.
- Keep a real `<table>` and use padding rows to fill the gap. That keeps native table semantics for screen readers.
- Paginate on the Rust side (`rows(offset, limit, filters)`) if the log is persisted. [INFERENCE] Client-side v9 sorting and filtering is fine up to tens of thousands of rows per the v9 benchmarks.

---

## 2. Alternatives rejected

### Component libraries

| Option | Facts | Why not |
|---|---|---|
| **Mantine** 9.8.2 | 193 KB gz full package. CSS modules, no runtime CSS-in-JS ([css modules](https://mantine.dev/styles/css-modules)). Very complete; monthly minors (9.0 Mar 2026 → 9.8 Nov 2026, [releases](https://mantine.dev/changelog/all-releases)). | Strongest runner-up. Rejected because it is a second styling paradigm (not Tailwind), its look is "Mantine-default" unless restyled, and the component code isn't ours. Choose it if the team refuses Tailwind. |
| **Chakra UI v3** 3.38 | Ark UI behaviour, but **Emotion still runs at runtime**; zero-runtime is only a draft v4 RFC ([uithings](https://uithings.com/what-is-chakra-ui), [v3 announce](https://chakra-ui.com/blog/announcing-v3)). 331 KB gz. | Runtime CSS-in-JS costs work per render, the bundle is the largest after antd, and v3 was a breaking rewrite that users complain about (same source). |
| **HeroUI v3** 3.2.8 | React Aria + Tailwind v4, CSS-only animations, 75+ components ([v3 release](https://heroui.com/en/docs/react/releases/v3-0-0), [InfoQ](https://www.infoq.com/news/2026/07/heroui-v3-rewrite/)). 75 KB gz. | A credible option, but it's an installed package with BEM classes rather than owned source. Users report missing v2 features and an unclear roadmap (Reddit/GitHub, quoted in InfoQ). Its rounded, consumer look fits a dense dev tool less well than shadcn [INFERENCE]. |
| **Ant Design** 6.7.2 | v6 moved to CSS variables and React 19 ([v6 notes](https://github.com/ant-design/ant-design/issues/55804)). 461 KB gz, the largest. | Enterprise-admin aesthetic, heavy, and cssinjs runtime. Its great Table isn't needed here because TanStack covers it. |
| **Tremor** (`@tremor/react` 3.18.7) | Vercel bought it in Jan 2025 ([Vercel](https://vercel.com/blog/vercel-acquires-tremor)). The npm package was last released in Jan 2025 and has a React 18 peer; the copy-paste route is the live one. Chart colours use Tailwind names, not `--chart-*` vars ([comparison](https://dev.to/malikabimanyu/best-shadcn-chart-components-and-collections-compared-honestly-jbb)). | It would be a second component system next to shadcn. Borrow ideas from it (tracker / bar-list / category-bar visuals) and build them with Tailwind. |
| **Radix-based shadcn** (`-b radix`, `radix-ui` 1.8.1) | Still fully supported and not deprecated ([changelog](https://ui.shadcn.com/docs/changelog/2026-07-base-ui-default)). | It's fine, but this is a greenfield UI, and shadcn recommends Base UI for new projects. Note that `cmdk` still pulls a few `@radix-ui/*` packages either way. |
| **MUI** | Used by Clash Verge Rev (MUI + Emotion + Monaco + react-virtual) ([package.json](https://github.com/clash-verge-rev/clash-verge-rev)). | Material look plus Emotion runtime. |

### Charts

| Option | Facts | Verdict |
|---|---|---|
| Recharts 3.11 (via shadcn chart) | SVG, composable, themed via CSS vars. 177 KB gz. | **Chosen**, lazy-loaded. Our data is a handful of series per model or account, which SVG handles easily. |
| uPlot 1.6.32 | About 50 KB min / 22 KB gz canvas. 166k points in 25 ms. No animations by design ([README](https://github.com/leeoniya/uPlot)). | Use it only if we add a dense, live tokens/sec timeline over thousands of requests. You theme it by hand by reading CSS vars into the opts. |
| visx (`@visx/xychart` 4.0) | D3 primitives, 50 KB gz for xychart. | Too low-level for 3–4 standard charts. |
| TanStack Charts / shadcn.io charts | 40+ types, but you need an account to see the code ([comparison](https://dev.to/malikabimanyu/best-shadcn-chart-components-and-collections-compared-honestly-jbb)). | Not needed. |

### Body viewer

| Option | Verdict |
|---|---|
| **Monaco** (`@monaco-editor/react` 4.7) | Rejected. It needs web workers set up for Vite ([integrate-esm](https://github.com/microsoft/monaco-editor/blob/main/docs/integrate-esm.md)). Its inline styles fight a strict CSP ([issue #4927](https://github.com/microsoft/monaco-editor/issues/4927)). The default loader pulls from a CDN, so Tauri apps need an offline loader ([Cognia notes](https://docs.cognia.cn/en/docs/subsystems/canvas/editor-core)). It's a whole IDE for a read-only view. |
| **react-json-view-lite** 2.5.0 | 2 KB gz, no dependencies, keyboard-navigable ARIA tree ([README](https://github.com/AnyRoad/react-json-view-lite)). Good as a second "Tree" tab later. It has no search and no virtualization, so it slows on multi-MB bodies. |
| shadcn `code-block` (Shiki) | Nice highlighting ([docs](https://ui.shadcn.com/docs/components/base/code-block)), but it renders the whole document with no fold or search. Not suited to 1M-token-context request bodies [INFERENCE]. |

### Other

- **next-themes**: unnecessary outside Next; shadcn's Vite guide provides the provider.
- **Zustand / Redux**: skip them. Server state lives in TanStack Query and UI state in `useState`/URL. Add a store only if cross-screen client state appears.
- **React Router / TanStack Router**: skip for three tabs; a `useState` tab or hash is enough. Vercel's "URL reflects state" rule matters little in a single-window desktop app [INFERENCE].

---

## 3. Tauri plugins to add

Plugin versions are from the [plugins-workspace](https://github.com/tauri-apps/plugins-workspace) (all require Rust ≥1.90). Add each with `npm run tauri add <name>` or `cargo add`.

| Plugin | JS pkg | Why for MLAI Bridge | Capability permissions |
|---|---|---|---|
| **Tray** (core, not a plugin) | `@tauri-apps/api/tray` | The proxy should keep running when the window closes. The tray menu gives Start/Stop proxy, Copy base URL, Open and Quit; left click shows the window ([docs](https://v2.tauri.app/learn/system-tray/)). Build it in Rust. | Requires `tauri = { features = ["tray-icon"] }`. `core:tray:default` only if JS creates the tray. |
| **single-instance** | (Rust only) | A second launch focuses the existing window instead of trying to bind the port twice ([docs](https://v2.tauri.app/plugin/single-instance/)). **Register it first** among plugins. | None; it has no JS API. |
| **autostart** 2.7.0 | `@tauri-apps/plugin-autostart` | "Start with Windows" toggle, so clients find the proxy up. Pass `Some(vec!["--minimized"])` so it boots to the tray ([docs](https://v2.tauri.app/plugin/autostart/)). | `autostart:default` (= allow-enable/disable/is-enabled) |
| **window-state** 2.5.0 | `@tauri-apps/plugin-window-state` | Restores size and position. Use the Rust side only. | `window-state:default` if JS calls it |
| **clipboard-manager** 2.5.0 | `@tauri-apps/plugin-clipboard-manager` | Copy base URL, API key and the request as JSON. `navigator.clipboard.writeText` usually works in WebView2 too [INFERENCE]; the plugin is the reliable route. | **No default**: add `clipboard-manager:allow-write-text` (write only; never grant read) ([reference](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/clipboard-manager)) |
| **notification** 2.8.1 | `@tauri-apps/plugin-notification` | "Proxy stopped / upstream auth failed / account limit hit" while minimized to the tray. | `notification:default` |
| **updater** 2.14.1 (+ `process` 2.4.0 for relaunch) | `@tauri-apps/plugin-updater`, `@tauri-apps/plugin-process` | Signed self-update from GitHub `latest.json`. On Windows, `installMode: "passive"` is the default and recommended ([docs](https://v2.tauri.app/plugin/updater/)). | `updater:default`, `process:allow-restart` |
| **opener** 2.7.3 | `@tauri-apps/plugin-opener` | Open docs and links in the system browser rather than inside the WebView. | `opener:default` (http/https/mailto/tel + reveal) |

Skipped: `store` (the Rust side already owns config), `log` (`tracing` already exists), `global-shortcut` (no need for a system-wide hotkey), `http`/`fs`/`shell`/`dialog` (unused, and they widen the attack surface).

Example `crates/app/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:allow-start-dragging", "core:window:allow-minimize",
    "core:window:allow-toggle-maximize", "core:window:allow-close",
    "core:window:allow-set-theme",
    "autostart:default", "notification:default", "updater:default",
    "process:allow-restart", "opener:default",
    "clipboard-manager:allow-write-text"
  ]
}
```

---

## 4. Windows 11 native look

### Option A (recommended): keep native decorations, add Mica

- Mica is built in: set `windowEffects: { effects: ["mica"] }` in the window config, or use the builder's `.effects(...)` in `gui.rs`. Valid values are `mica | micaDark | micaLight | tabbed | tabbedDark | tabbedLight` (Windows 11 only) and `acrylic`, which performs badly when dragging or resizing ([config schema](https://schema.tauri.app/config/2)).
- Effects **require `transparent: true`**. Then `html, body { background: transparent }`, and paint panels with semi-opaque surfaces ([window-vibrancy README](https://github.com/tauri-apps/window-vibrancy)).
- Known bug: with decorations on, the effect may not apply until a resize. The workaround is to create with `decorations(false)` and call `set_decorations(true)` after setup ([issue #40](https://github.com/tauri-apps/window-vibrancy/issues/40), upstream tauri#8632).
- Windows 10 users get no Mica. Fall back to an opaque `backgroundColor`.

### Option B: custom titlebar (`decorations: false`)

- Gives a Linear/Raycast look, with tabs in the title row.
- Use `data-tauri-drag-region` plus `core:window:allow-start-dragging/minimize/toggle-maximize/close` ([docs](https://tauri.app/learn/window-customization/)).
- **Pitfall:** you lose Windows 11 Snap Layouts on the maximize button. WebView2 swallows `WM_NCHITTEST`, so no frontend code can fix it. The fix is an invisible native overlay over the button, which [tauri-plugin-decorum](https://github.com/clearlysid/tauri-plugin-decorum) (in maintenance mode) or `tauri-plugin-frame` provide.
- Further traps from the [dev.to write-up](https://dev.to/zbrooklyn/windows-11-snap-layouts-in-a-frameless-tauri-app-the-part-every-guide-gets-wrong-2fjh):
  - Rust button width and height must equal the CSS exactly.
  - A `minWidth` above ~500 px breaks snapping.
- **Verdict:** start with A. Only go to B if the design needs a title-row UI, and then use decorum or frame and test Snap.

---

## 5. Pitfalls

### Tauri v2 permissions and capabilities

- All plugin commands are blocked until they are listed in a capability file. A missing one fails at runtime with a "not allowed" error that the UI may swallow, so surface `invoke` errors in a toast.
- **Capability files in `capabilities/` auto-enable**, but once `app.security.capabilities` is set in `tauri.conf.json`, only the listed ones apply ([docs](https://tauri.app/security/capabilities/)).
- Our crate is `crates/app`, so the directory is `crates/app/capabilities/`. `$schema` points to `../gen/schemas/desktop-schema.json`, generated on first build.
- Our own `#[tauri::command]`s are allowed for all windows by default. Restrict them with `AppManifest::commands` in `build.rs` if more windows or webviews are ever added (same doc).
- `clipboard-manager` has **no** default permission, so `clipboard-manager:default` grants nothing useful. Grant `allow-write-text` explicitly.
- `single-instance` needs no permission, but must be the first plugin registered.
- The window's label must match `"windows": ["main"]`. It does: `gui.rs` uses `"main"`.
- `withGlobalTauri: true` exposes `window.__TAURI__` to any script in the page. Once we import `@tauri-apps/api` ESM, set it to `false` so it isn't needed (decorum's README asks for it `true`; another reason to prefer Option A).

### CSP

- The current `"csp": null` disables CSP entirely. Tauri only enforces a CSP if one is configured. It injects nonces and hashes for bundled assets at build time ([CSP docs](https://v2.tauri.app/security/csp/)).
- Suggested starting policy:

  ```json
  "csp": {
    "default-src": "'self'",
    "connect-src": "ipc: http://ipc.localhost",
    "img-src": "'self' data: blob:",
    "style-src": "'self' 'unsafe-inline'",
    "font-src": "'self'"
  }
  ```

- Notes on that policy:
  - `ipc:` and `http://ipc.localhost` are required for `invoke` and events on Windows (example in the docs).
  - `'unsafe-inline'` for styles is needed by Recharts and Base UI positioning inline styles [INFERENCE: standard for floating-UI libs].
  - Fonts are bundled, so no Google hosts.
  - Do **not** fetch the local proxy (`127.0.0.1:<port>`) from the UI. Go through `invoke`, or else add that origin to `connect-src`.
  - Monaco would also need `worker-src blob:`; another reason to choose CodeMirror.
- Use `devCsp` to keep a looser policy for Vite HMR (`ws://localhost:1420`).

### WebView2

- **Distribution:** the NSIS installer downloads the WebView2 bootstrapper if the runtime is missing (default `downloadBootstrapper`). Offline machines need `embedBootstrapper` or `offlineInstaller` in `bundle.windows.webviewInstallMode` ([installer docs](https://v2.tauri.app/de/distribute/windows-installer), [MS distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)). Win 11 ships the runtime.
- **Build target:** the Tauri Vite guide sets `build.target: 'chrome105'` on Windows ([guide](https://v2.tauri.app/start/frontend/vite/)), but Tailwind v4 requires Chrome 111+ features (`@property`, `color-mix`, cascade layers). Set `target: "chrome111"` (or `es2022`) or Tailwind output may be down-levelled or broken [INFERENCE from the Tailwind compat table]. Evergreen WebView2 is far past 111.
- **White flash on startup in dark mode:**
  - Set the window `backgroundColor` to the dark `--background` value.
  - Put a background on `html` as well as `body`.
  - Optionally create the window `visible(false)` and `show()` after the first paint ([tauri.ninja recipe](https://tauri.ninja/recipe.php?id=front-010)).
  - With `transparent: true`, `noRedirectionBitmap` helps avoid the flash ([schema](https://schema.tauri.app/config/2)).
  - tauri-ui ships "hidden until first paint" for the same reason ([README](https://github.com/agmmnn/tauri-ui)).
- **Theme sync:**
  - Tauri `theme` affects the native titlebar colour; `setTheme` needs `core:window:allow-set-theme`, which is not in `core:default`.
  - `prefers-color-scheme` and `theme()` can disagree when the theme is forced, so drive CSS from one `data-theme`/`.dark` attribute ([recipe](https://tauri.ninja/recipe.php?id=front-010)).
- **Native `<select>` on Windows:** set explicit `background-color` and `color` ([Vercel](https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/AGENTS.md)). Prefer the shadcn Select.
- **HTML5 drag-and-drop** is broken on Windows unless `dragDropEnabled: false` ([schema](https://schema.tauri.app/config/2)). This only matters if we add drag-to-import of token files.
- **Desktop feel:** disable overscroll bounce, keep `user-select: none` on chrome but allow selecting values, open external links via `opener`, and disable the context menu in release builds. tauri-ui bundles all of these as defaults ([README](https://github.com/agmmnn/tauri-ui)).

### Other pitfalls

- **Polling cost:** every `useQuery` observer with `refetchInterval` runs its own timer. Also, reading `isFetching` re-renders on every tick ([polling guide](https://tanstack.com/query/latest/docs/framework/react/guides/polling)). Poll in one place and prefer Rust-emitted events.
- **Bundle size:** this is a local app with no network load, so size mainly affects startup parse time [INFERENCE]. Lazy-load Usage (Recharts) and the detail drawer (CodeMirror). Expected total is roughly 150–250 KB gz first-load JS [INFERENCE from the per-package figures above, tree-shaken].
- **TanStack Table v9** requires an explicit `features` option and has a new `useTable` API. Copying v8 snippets from blogs will not compile ([migration](https://tanstack.com/table/latest/docs/framework/react/guide/migrating)).
- **Base UI vs Radix in shadcn:** `asChild` became `render`. Most blog snippets still use Radix syntax ([changelog](https://ui.shadcn.com/docs/changelog/2026-07-base-ui-default)).

---

## 6. Accessibility checklist (from Vercel guidelines + library choices)

Base UI, cmdk and react-json-view-lite all implement WAI-ARIA patterns. What we must still do (from [Vercel Web Interface Guidelines](https://vercel.com/design/guidelines)):

- Visible `:focus-visible` rings; never `outline: none` without a replacement.
- Icon-only buttons (copy, close, titlebar controls) get an `aria-label`.
- The status pill uses text plus an icon, not colour only.
- Toasts use polite `aria-live`; Base UI Toast and Sonner do this.
- `tabular-nums` on token and credit columns.
- Locale-aware numbers via `Intl.NumberFormat`.
- Honour `prefers-reduced-motion`.
- Chart palettes are colour-blind safe, and every chart also has a table view.
- Confirm "Remove account".
- Hit targets ≥24 px. Dense tables are fine at 32 px rows.

---

## 7. Design references

- **Linear, Mar 2026 refresh** ([post](https://linear.app/now/behind-the-latest-design-refresh)):
  - Dim the sidebar so content dominates.
  - Use compact tabs and fewer, smaller icons.
  - Use softer, fewer borders ("structure should be felt not seen").
  - Use warmer neutral greys instead of blue-tinted ones.
  - Themes are generated in LCH from base, accent and contrast ([2024 redesign](https://linear.app/now/how-we-redesigned-the-linear-ui)).
  - Applied here: use Tailwind v4's OKLCH tokens and a neutral base, and keep one accent for the "running" state.
- **Vercel guidelines** ([AGENTS.md](https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/AGENTS.md)):
  - Layered shadows; semi-transparent borders.
  - Nested radii, with a child never rounder than its parent.
  - Design empty, sparse, dense and error states.
  - Loading buttons keep their label.
- **Reference Tauri apps:**
  - [Yaak](https://github.com/mountain-loop/yaak): Tauri 2 API client, 19.5k★. Tailwind, TanStack Query/Router/Virtual and CodeMirror; the closest analogue to our request inspector.
  - [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev): Tauri proxy GUI. MUI, Monaco, react-virtual and the updater plugin; a proxy with tray, autostart and logs UX.
  - [tauri-ui](https://github.com/agmmnn/tauri-ui): shadcn + Tauri scaffolder, 2.3k★. Its desktop defaults are worth copying.
