// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Best-effort native OS drag-and-drop for the Files screen's drop zone
 * (architecture.md §13.2 "drop zone").
 *
 * Plain HTML5 `DataTransfer`/`File` drag-and-drop does not expose real
 * filesystem paths inside a WebView2/WKWebView webview (browsers
 * deliberately withhold them) — Tauri instead emits its own window-level
 * `tauri://drag-drop` event carrying real paths, via
 * `Window.onDragDropEvent` (`@tauri-apps/api/window`), which this hook
 * subscribes to. That subscription only rides the same generic
 * `core:event:default` permission the app already uses for job events
 * (`capabilities/default.json`'s own comment: "core:default... already
 * include event listen/emit"), so no capability change is required.
 *
 * Kept defensive end to end: this only activates inside a real Tauri
 * webview, and any failure to subscribe is swallowed. "Add Files"/"Add
 * Folder" (`DropZone.tsx`, backed by `select_input_files`/
 * `select_input_directory`) remain the fully-verified, guaranteed-working
 * path regardless of whether this enhancement engages — this hook was not
 * exercised against a real packaged app in this environment (no way to
 * launch one here), only type-checked and defensively coded.
 */
import { useEffect, useRef } from "react";

const PGN_EXTENSION = /\.pgn$/i;

/** Mirrors `@tauri-apps/api/core`'s own `isTauri()` check
 * (`return !!(globalThis || window).isTauri`) inline, rather than
 * importing it, so this hook does not depend on the shared
 * `@tauri-apps/api/core` mock every component test already installs
 * (`src/test/setup.ts`) having an opinion about `isTauri`. */
function isRunningInsideTauri(): boolean {
  return typeof window !== "undefined" && Boolean((window as unknown as { isTauri?: boolean }).isTauri);
}

export function useTauriFileDrop(onPaths: (paths: string[]) => void): void {
  const onPathsRef = useRef(onPaths);
  useEffect(() => {
    onPathsRef.current = onPaths;
  });

  useEffect(() => {
    if (!isRunningInsideTauri()) return;

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) =>
        getCurrentWindow().onDragDropEvent((event) => {
          if (event.payload.type !== "drop") return;
          const pgnPaths = event.payload.paths.filter((path) => PGN_EXTENSION.test(path));
          if (pgnPaths.length > 0) onPathsRef.current(pgnPaths);
        }),
      )
      .then((stop) => {
        if (cancelled) {
          stop();
        } else {
          unlisten = stop;
        }
      })
      .catch(() => {
        // Native drag-and-drop is an enhancement only; the click-to-browse
        // buttons cover the required §13.2 functionality on their own.
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}
