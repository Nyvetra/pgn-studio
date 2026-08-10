// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Phase 0 diagnostic screen.
 *
 * This is deliberately not a real feature screen. Its only job is to prove
 * the Tauri IPC boundary works end to end: the React frontend calls the
 * `get_app_info` Rust command through `src/ipc/client.ts` and renders the
 * response (or a visible error if the call fails). Phase 2 replaces this
 * with the real five-step workflow described in architecture.md §13.
 */
import { useEffect, useState } from "react";
import { getAppInfo } from "../ipc/client";
import type { AppInfo } from "../ipc/generated-types";
import { AppProviders } from "./providers";
import "../styles/global.css";

type LoadState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
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
        Phase 0 scaffold &mdash; IPC boundary check
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
          <dt>App name</dt>
          <dd>{state.info.name}</dd>
          <dt>App version</dt>
          <dd>{state.info.version}</dd>
          <dt>Tauri version</dt>
          <dd>{state.info.tauriVersion}</dd>
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
