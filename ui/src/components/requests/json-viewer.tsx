import { useEffect, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { json } from "@codemirror/lang-json";
import { oneDark } from "@codemirror/theme-one-dark";
import { useTheme } from "@/components/theme-provider";

function useIsDark() {
  const { theme } = useTheme();
  const [dark, setDark] = useState(() => document.documentElement.classList.contains("dark"));
  useEffect(() => {
    const read = () => setDark(document.documentElement.classList.contains("dark"));
    read();
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    mql.addEventListener("change", read);
    const obs = new MutationObserver(read);
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => {
      mql.removeEventListener("change", read);
      obs.disconnect();
    };
  }, [theme]);
  return dark;
}

/** Read-only JSON body with line numbers, folding and Ctrl+F search. Lazy-loaded. */
export default function JsonViewer({ value, height }: { value: string; height?: string }) {
  const dark = useIsDark();
  return (
    <CodeMirror
      value={value}
      height={height}
      readOnly
      theme={dark ? oneDark : "light"}
      extensions={[json()]}
      basicSetup={{
        lineNumbers: true,
        foldGutter: true,
        highlightActiveLine: false,
        highlightActiveLineGutter: false,
        searchKeymap: true,
        bracketMatching: true,
        closeBrackets: false,
        autocompletion: false,
      }}
      className="overflow-hidden rounded-md border text-xs [&_.cm-editor]:outline-none [&_.cm-gutters]:bg-muted/40"
    />
  );
}
