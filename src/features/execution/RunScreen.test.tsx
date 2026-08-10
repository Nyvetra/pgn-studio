// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";
import type { JobEvent } from "../../ipc/client";
import { JobRunProvider } from "./JobRunProvider";
import { useJobRunContext } from "./useJobRunContext";
import { RunScreen } from "./RunScreen";
import { LiveAnnouncerProvider } from "../../components/LiveAnnouncer";
import { WorkflowProvider } from "../../state/WorkflowContext";
import { checkA11y } from "../../test/a11y";

const startJob = vi.fn();
const cancelJob = vi.fn();
const getJob = vi.fn();

vi.mock("../../ipc/client", async () => {
  const actual = await vi.importActual<typeof import("../../ipc/client")>("../../ipc/client");
  return {
    ...actual,
    startJob: (...args: unknown[]) => startJob(...args),
    cancelJob: (...args: unknown[]) => cancelJob(...args),
    getJob: (...args: unknown[]) => getJob(...args),
  };
});

type Handler = (event: JobEvent) => void;
const handlers: Record<string, Handler[]> = {
  state: [],
  stage: [],
  log: [],
  metrics: [],
  artifact: [],
  completed: [],
};

function registerHandler(kind: keyof typeof handlers) {
  return vi.fn((handler: Handler) => {
    handlers[kind].push(handler);
    return Promise.resolve(vi.fn());
  });
}

vi.mock("../../ipc/events", () => ({
  onJobState: registerHandler("state"),
  onJobStage: registerHandler("stage"),
  onJobLog: registerHandler("log"),
  onJobMetrics: registerHandler("metrics"),
  onJobArtifact: registerHandler("artifact"),
  onJobCompleted: registerHandler("completed"),
}));

function emit(event: JobEvent) {
  for (const handler of handlers[event.type]) handler(event);
}

const JOB_ID = "11111111-1111-4111-8111-111111111111";

function Harness() {
  const jobRun = useJobRunContext();
  return (
    <div>
      <button
        onClick={() =>
          void jobRun.start({
            schemaVersion: 1,
            id: JOB_ID,
            name: "test",
            inputs: [],
            output: {
              directory: "C:\\out",
              baseName: "clean",
              uniqueGames: true,
              duplicateGames: "none",
              logFile: true,
              manifest: true,
              alwaysCreateAudit: false,
              conflictPolicy: "addNumericSuffix",
              confirmedReplace: false,
            },
            operations: {
              mode: "process",
              duplicates: "none",
              cleanup: {
                removeComments: false,
                removeVariations: false,
                removeNags: false,
                removeMoveNumbers: false,
                removeResults: false,
                removeTags: [],
                rejectBadResults: false,
                fixResultTags: false,
              },
              broken: "discard",
              eco: { enabled: false },
              outputNotation: "san",
              checkFile: null,
            },
            filters: {
              tagRules: [],
              moveBounds: null,
              checkmateOnly: false,
              setupPolicy: "any",
              fenPattern: null,
              textualVariations: [],
              advancedArgs: [],
            },
            runtime: { useExternalDuplicateTable: false, countOutputGames: true },
          })
        }
      >
        start-test-job
      </button>
      <RunScreen />
    </div>
  );
}

function renderHarness() {
  return render(
    <WorkflowProvider>
      <LiveAnnouncerProvider>
        <JobRunProvider>
          <Harness />
        </JobRunProvider>
      </LiveAnnouncerProvider>
    </WorkflowProvider>,
  );
}

beforeEach(() => {
  handlers.state = [];
  handlers.stage = [];
  handlers.log = [];
  handlers.metrics = [];
  handlers.artifact = [];
  handlers.completed = [];
  startJob.mockReset().mockResolvedValue({
    status: "ok",
    data: { jobId: JOB_ID, startedAt: "2026-08-10T10:00:00Z" },
  });
  cancelJob.mockReset().mockResolvedValue({ status: "ok", data: null });
  getJob.mockReset().mockResolvedValue({ status: "error", error: { code: "INVALID_JOB_SPEC" } });
});

describe("RunScreen", () => {
  it("has no automated a11y violations once running (architecture.md §13.8)", async () => {
    const user = userEvent.setup();
    const { container } = renderHarness();
    await user.click(screen.getByText("start-test-job"));
    await waitFor(() => expect(screen.getByRole("progressbar")).toBeInTheDocument());
    emit({ type: "stage", jobId: JOB_ID, seq: 1, stage: "processing", message: "Processing" });
    emit({ type: "log", jobId: JOB_ID, seq: 2, level: "info", line: "Games: 42" });
    await screen.findByText("Processing games");
    expect(await checkA11y(container)).toHaveNoViolations();
  });

  it("shows a waiting message before the job is accepted", () => {
    renderHarness();
    expect(screen.getByText(/Waiting for the job to start/)).toBeInTheDocument();
  });

  it("states plainly that original files are unchanged", async () => {
    const user = userEvent.setup();
    renderHarness();
    await user.click(screen.getByText("start-test-job"));
    await waitFor(() => expect(screen.getByRole("progressbar")).toBeInTheDocument());
    expect(screen.getByText(/original source files are never modified/)).toBeInTheDocument();
  });

  it("shows an indeterminate progress indicator that never carries a numeric percentage (§4.7 binding rule)", async () => {
    const user = userEvent.setup();
    renderHarness();
    await user.click(screen.getByText("start-test-job"));
    const bar = await screen.findByRole("progressbar");
    expect(bar).not.toHaveAttribute("aria-valuenow");
    expect(bar).not.toHaveAttribute("aria-valuemin");
    expect(bar).not.toHaveAttribute("aria-valuemax");
  });

  it("shows the current stage and the last log line as stage/log events arrive", async () => {
    const user = userEvent.setup();
    renderHarness();
    await user.click(screen.getByText("start-test-job"));
    await waitFor(() => expect(screen.getByRole("progressbar")).toBeInTheDocument());

    emit({ type: "stage", jobId: JOB_ID, seq: 1, stage: "processing", message: "Processing" });
    emit({ type: "log", jobId: JOB_ID, seq: 2, level: "info", line: "Games: 42" });

    expect(await screen.findByText("Processing games")).toBeInTheDocument();
    // "Games: 42" legitimately appears twice: once as the "last log line"
    // summary, once in the full log body below it.
    expect(screen.getAllByText("Games: 42").length).toBe(2);
  });

  it("shows elapsed time ticking (formatted, never raw milliseconds)", async () => {
    const user = userEvent.setup();
    renderHarness();
    await user.click(screen.getByText("start-test-job"));
    await waitFor(() => expect(screen.getByRole("progressbar")).toBeInTheDocument());
    expect(screen.getByText("Elapsed time")).toBeInTheDocument();
  });

  it("lists output artifacts as job://artifact events arrive", async () => {
    const user = userEvent.setup();
    renderHarness();
    await user.click(screen.getByText("start-test-job"));
    await waitFor(() => expect(screen.getByRole("progressbar")).toBeInTheDocument());
    expect(screen.getByText("None yet.")).toBeInTheDocument();

    emit({
      type: "artifact",
      jobId: JOB_ID,
      seq: 1,
      artifact: { kind: "uniqueGames", path: "C:\\out\\clean.pgn", sizeBytes: 100 },
    });
    expect(await screen.findByText(/clean\.pgn/)).toBeInTheDocument();
  });

  it("Cancel calls cancel_job with the active job id and disables itself while cancelling", async () => {
    const user = userEvent.setup();
    renderHarness();
    await user.click(screen.getByText("start-test-job"));
    await waitFor(() => expect(screen.getByRole("progressbar")).toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancelJob).toHaveBeenCalledWith(JOB_ID);

    emit({ type: "state", jobId: JOB_ID, seq: 1, state: "cancelling" });
    expect(await screen.findByRole("button", { name: "Cancelling…" })).toBeDisabled();
  });

  it("shows a clear error and a way back if start_job itself fails", async () => {
    startJob.mockResolvedValueOnce({
      status: "error",
      error: {
        code: "JOB_ALREADY_RUNNING",
        title: "A job is already running",
        message: "Only one job can run at a time.",
        remediation: "Wait for it to finish, or cancel it.",
        logPath: null,
        technicalId: "x",
      },
    });
    const user = userEvent.setup();
    renderHarness();
    await user.click(screen.getByText("start-test-job"));
    expect(await screen.findByText("Only one job can run at a time.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Back to Review" })).toBeInTheDocument();
  });
});
