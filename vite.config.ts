// SPDX-License-Identifier: GPL-3.0-or-later
/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  // Vitest configuration (Vite's `test` field, read only by the Vitest CLI).
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,

    // Vitest's default is 5000ms, which suits fast unit tests. A good part
    // of this suite is not that: `src/app/App.test.tsx` renders the real
    // component tree and drives it with userEvent (a keystroke at a time,
    // each triggering a React re-render), and every screen has an axe
    // accessibility audit.
    //
    // 5000ms proved too tight on CI's slower hardware. On GitHub's
    // macos-15-intel runner, `moving from Files to Operations via Next`
    // exceeded 5000ms and failed the Frontend job (main CI for f21e23d),
    // while an a11y test took 2744ms there. The same two tests measure
    // ~596ms and ~600ms on the development machine, so that runner is
    // roughly 5-8x slower for this kind of work. At that factor the
    // slowest test here (~726ms, OperationsScreen's a11y audit) would land
    // near 5.8s - so this was never specific to the one test that happened
    // to fail first.
    //
    // 20s leaves the slowest measured test ~27x headroom, covering the
    // observed slowdown several times over. It masks nothing: no test here
    // waits on anything unbounded, a genuinely stuck test still fails, and
    // a raised ceiling costs zero time on the passing path.
    testTimeout: 20_000,
  },
}));
