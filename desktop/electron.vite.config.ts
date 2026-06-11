import { resolve } from "node:path";

import react from "@vitejs/plugin-react";
import { defineConfig, externalizeDepsPlugin } from "electron-vite";

// Renderer = KÖKTEKİ mevcut React uygulaması (src/ + index.html). Kod
// çatallanmaz: aynı kaynak Tauri'de `pnpm tauri dev`, Electron'da `pnpm dev`
// (desktop/) ile çalışır — host farkını src/lib/host.ts adaptörü kapatır.
export default defineConfig({
  main: {
    plugins: [externalizeDepsPlugin()],
  },
  preload: {
    plugins: [externalizeDepsPlugin()],
  },
  renderer: {
    root: resolve(__dirname, ".."),
    plugins: [react()],
    build: {
      outDir: resolve(__dirname, "out", "renderer"),
      rollupOptions: {
        input: resolve(__dirname, "..", "index.html"),
      },
    },
  },
});
