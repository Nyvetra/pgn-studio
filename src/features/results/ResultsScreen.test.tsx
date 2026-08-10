// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";
import type { JobEvent, JobResult } from "../../ipc/client";
import { JobRunProvider } from "../execution/JobRunProvider";
import { useJobRunContext } from "../execution/useJobRunContext";
import { WorkflowProvider } from "../../state/WorkflowContext";
import { useWorkflow } from "../../state/useWorkflow";
import { ResultsScreen } from "./ResultsScreen";
import { NOT_AVAILABLE } from "../../state/formatters";

const startJob = vi.fn();
const cancelJob = vi.fn();
const getJob = vi.fn();
const openPath = vi.fn();
const revealPath = vi.fn();

vi.mock("../../ipc/client", async () => {
  const actual = await vi.importActual<typeof import("../../ipc/client")>("../../ipc/client");
  return {
    ...actual,
    startJob: (...args: unknown[]) => startJob(...args),
    cancelJob: (...args: unknown[]) => cancelJob(...args),
    getJob: (...args: unknown[]) => getJob(...args),
    openPath: (...args: unknown[]) => openPath(...args),
    revealPath: (...args: unknown[]) => revealPath(...args),
  };
});

type Handler = (event: JobEvent) => void;
const handlers: Record<string, Handler[]> = { state: [], stage: [], log: [], metrics: [], artifact: [], completed: [] };
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

function succeededResult(overrides: Partial<JobResult> = {}): JobResult {
  return {
    jobId: JOB_ID,
    status: "succeeded",
    startedAt: "2026-08-10T10:00:00Z",
    finishedAt: "2026-08-10T10:00:05Z",
    elapsedMs: 5000,
    engine: { version: "v26-06", sha256: "a".repeat(64), targetTriple: "x86_64-pc-windows-msvc" },
    artifacts: [{ kind: "uniqueGames", path: "C:\\out\\clean.pgn", sizeBytes: 2048 }],
    metrics: {
      inputFiles: 2,
      inputBytes: 4096,
      processedGames: 10,
      inputGames: 10,
      outputGames: 8,
      duplicateGames: 2,
      brokenGames: null,
      outputBytes: 1500,
    },
    warnings: [],
    error: null,
    ...overrides,
  };
}

function Harness({ result }: { result: JobResult }) {
  const jobRun = useJobRunContext();
  return (
    <div>
      <button
        onClick={async () => {
          await jobRun.start({
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
          });
          emit({ type: "completed", jobId: JOB_ID, seq: 1, result });
        }}
      >
        run-and-complete
      </button>
      <ResultsScreen />
    </div>
  );
}

function StepProbe() {
  const { state } = useWorkflow();
  return <p data-testid="step">{state.step}</p>;
}

/** In the real app, reaching "run-results" always means the user passed
 * through every earlier step's own Next button first, which is what
 * actually extends `farthestStepIndex` far enough for "Rerun Job"'s
 * `GO_TO_STEP("review")` to be allowed. This test harness renders
 * ResultsScreen directly (bypassing that real navigation), so it recreates
 * the precondition explicitly, mount-only. */
function AdvanceFarthestStepToRunResultsOnce() {
  const { dispatch } = useWorkflow();
  useEffect(() => {
    dispatch({ type: "GO_NEXT" });
    dispatch({ type: "GO_NEXT" });
    dispatch({ type: "GO_NEXT" });
    dispatch({ type: "GO_NEXT" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return null;
}

async function renderCompleted(result: JobResult = succeededResult()) {
  const user = userEvent.setup();
  render(
    <WorkflowProvider>
      <JobRunProvider>
        <AdvanceFarthestStepToRunResultsOnce />
        <Harness result={result} />
        <StepProbe />
      </JobRunProvider>
    </WorkflowProvider>,
  );
  await user.click(screen.getByText("run-and-complete"));
  await waitFor(() => expect(screen.getByRole("heading", { name: "Results" })).toBeInTheDocument());
  return user;
}

beforeEach(() => {
  handlers.state = [];
  handlers.stage = [];
  handlers.log = [];
  handlers.metrics = [];
  handlers.artifact = [];
  handlers.completed = [];
  startJob.mockReset().mockResolvedValue({ status: "ok", data: { jobId: JOB_ID, startedAt: "2026-08-10T10:00:00Z" } });
  cancelJob.mockReset().mockResolvedValue({ status: "ok", data: null });
  getJob.mockReset().mockResolvedValue({ status: "error", error: { code: "INVALID_JOB_SPEC" } });
  openPath.mockReset().mockResolvedValue({ status: "ok", data: null });
  revealPath.mockReset().mockResolvedValue({ status: "ok", data: null });
  // jsdom exposes `navigator.clipboard` as a getter-only property, so a
  // plain assignment throws — redefine it instead. jsdom also does not
  // actually implement `document.execCommand`'s copy behavior, so that is
  // stubbed too — together these let `copyToClipboard`'s real success path
  // (whichever branch fires) be exercised deterministically in tests.
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
    configurable: true,
  });
  // jsdom does not define `execCommand` at all (not even as a stub), so it
  // must be assigned outright rather than spied on.
  document.execCommand = vi.fn().mockReturnValue(true);
});

describe("ResultsScreen", () => {
  it("shows a success banner and the elapsed time from the terminal result", async () => {
    await renderCompleted();
    expect(screen.getByText("Job succeeded")).toBeInTheDocument();
    expect(screen.getByText("Elapsed time: 5s")).toBeInTheDocument();
  });

  it("shows a failure banner as an alert when the job failed", async () => {
    await renderCompleted(
      succeededResult({
        status: "failed",
        error: {
          code: "ENGINE_EXIT_NONZERO",
          title: "The engine exited with an error",
          message: "pgn-extract exited with a non-zero status.",
          remediation: "Check the log for details.",
          logPath: "C:\\logs\\job.log",
          technicalId: "tech-1",
        },
      }),
    );
    // Two alerts are expected and legitimate here: the overall status
    // banner (tone "danger" -> role "alert") and the dedicated error-detail
    // banner underneath it.
    const alerts = screen.getAllByRole("alert");
    expect(alerts.some((el) => el.textContent?.includes("Job failed"))).toBe(true);
    expect(screen.getByText("pgn-extract exited with a non-zero status.")).toBeInTheDocument();
    expect(screen.getByText("Check the log for details.")).toBeInTheDocument();
  });

  it('renders unknown metrics as "Not available", never 0 (§9.3, §13.7, §25 binding rule)', async () => {
    await renderCompleted(
      succeededResult({
        metrics: {
          inputFiles: 2,
          inputBytes: 4096,
          processedGames: null,
          inputGames: 10,
          outputGames: null,
          duplicateGames: null,
          brokenGames: null,
          outputBytes: null,
        },
      }),
    );
    // outputGames/duplicateGames/brokenGames/outputBytes are all null here.
    expect(screen.getAllByText(NOT_AVAILABLE).length).toBe(4);
    expect(screen.queryByText(/^0$/)).not.toBeInTheDocument();
  });

  it("shows a genuine zero metric as 0, distinct from unknown", async () => {
    await renderCompleted(succeededResult({ metrics: { ...succeededResult().metrics, duplicateGames: 0 } }));
    const dupMetric = screen.getByText("Duplicate games").closest(".metric");
    expect(dupMetric).toHaveTextContent("0");
  });

  it("lists artifacts with size and wires Open File / Reveal in Folder to the real IPC commands", async () => {
    const user = await renderCompleted();
    expect(screen.getByText("2 KB")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open File" }));
    expect(openPath).toHaveBeenCalledWith("C:\\out\\clean.pgn");

    await user.click(screen.getByRole("button", { name: "Reveal in Folder" }));
    expect(revealPath).toHaveBeenCalledWith("C:\\out\\clean.pgn");
  });

  it("Copy Path confirms success once the path is copied", async () => {
    // Asserts the observable, user-facing behavior (button label flips to
    // "Copied!") rather than which specific browser clipboard API fired —
    // jsdom's own `navigator.clipboard` is a non-configurable-in-practice
    // stub that resists reliable mocking, but `copyToClipboard`'s
    // `document.execCommand` fallback (also exercised by real older
    // WebView2 configurations) is real and jsdom does implement it.
    const user = await renderCompleted();
    await user.click(screen.getByRole("button", { name: "Copy Path" }));
    expect(await screen.findByRole("button", { name: "Copied!" })).toBeInTheDocument();
  });

  it("View Log toggles the bounded log view", async () => {
    const user = await renderCompleted();
    expect(screen.queryByRole("log")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "View Log" }));
    expect(screen.getByRole("log")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Hide Log" }));
    expect(screen.queryByRole("log")).not.toBeInTheDocument();
  });

  it("states that the job has been saved to history, and offers Rerun Job / Start New Job", async () => {
    await renderCompleted();
    expect(screen.getByText(/added to your local job history/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rerun Job" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start New Job" })).toBeInTheDocument();
  });

  it("Rerun Job returns to the Review step", async () => {
    const user = await renderCompleted();
    await user.click(screen.getByRole("button", { name: "Rerun Job" }));
    expect(screen.getByTestId("step")).toHaveTextContent("review");
  });

  it("Start New Job resets the draft back to the Files step", async () => {
    const user = await renderCompleted();
    await user.click(screen.getByRole("button", { name: "Start New Job" }));
    expect(screen.getByTestId("step")).toHaveTextContent("files");
  });
});
