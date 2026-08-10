// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Phase 2a diagnostic screen.
 *
 * Still deliberately not a real feature screen — its only job is to prove
 * the Tauri IPC boundary works end to end with the real, typed command
 * surface: the React frontend calls the `get_app_info` Rust command
 * through `src/ipc/client.ts` (backed by the generated, tauri-specta
 * bindings) and renders the response, or a visible error if the call
 * fails. Phase 2b replaces this with the real five-step workflow described
 * in architecture.md §13 — building that UI, and wiring the rest of the
 * command/event surface `src/ipc/client.ts` and `src/ipc/events.ts` already
 * expose, is explicitly out of scope here.
 */
import { useEffect, useState } from "react";
import { getAppInfo, type AppInfoDto } from "../ipc/client";
import { AppProviders } from "./providers";
import "../styles/global.css";

type LoadState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfoDto }
  | { status: "error"; message: string };

function Diagnostics() {
  const [state, setState] = useState<LoadState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;

    getAppInfo()
      .then((info) => {
        if (!cancelled) {
          setState({ status: "ready", info });
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setState({
            status: "error",
            message: error instanceof Error ? error.message : String(error),
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="diagnostics">
      <h1>PGN Studio</h1>
      <p className="diagnostics__subtitle">
        Phase 2a scaffold &mdash; typed IPC boundary check
      </p>

      {state.status === "loading" && (
        <p role="status">Contacting Rust backend&hellip;</p>
      )}

      {state.status === "error" && (
        <p role="alert" className="diagnostics__error">
          Failed to reach the Rust backend: {state.message}
        </p>
      )}

      {state.status === "ready" && (
        <dl className="diagnostics__info" aria-live="polite">
          <dt>App version</dt>
          <dd>{state.info.appVersion}</dd>
          <dt>OS</dt>
          <dd>{state.info.os}</dd>
          <dt>Architecture</dt>
          <dd>{state.info.arch}</dd>
        </dl>
      )}
    </main>
  );
}

export function App() {
  return (
    <AppProviders>
      <Diagnostics />
    </AppProviders>
  );
}

export default App;
