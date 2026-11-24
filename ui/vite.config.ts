import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  base: "./",
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(import.meta.dirname, "src") } },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  // Tailwind v4 needs Chrome 111+ features; Evergreen WebView2 is far past that.
  build: { outDir: "dist", emptyOutDir: true, target: "chrome111" },
});
