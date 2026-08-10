// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The real five-step workflow (architecture.md §13): Files -> Operations
 * -> Filters -> Review -> Run & Results. Phase 2a's diagnostic screen
 * (proving the IPC boundary worked end to end) is gone — that job is now
 * done implicitly by every screen that actually calls a real command.
 */
import { WorkflowProvider } from "../state/WorkflowContext";
import { JobRunProvider } from "../features/execution/JobRunProvider";
import { LiveAnnouncerProvider } from "../components/LiveAnnouncer";
import { AppProviders } from "./providers";
import { AppShell } from "./AppShell";
import "../styles/global.css";

export function App() {
  return (
    <AppProviders>
      <WorkflowProvider>
        <JobRunProvider>
          <LiveAnnouncerProvider>
            <AppShell />
          </LiveAnnouncerProvider>
        </JobRunProvider>
      </WorkflowProvider>
    </AppProviders>
  );
}

export default App;
