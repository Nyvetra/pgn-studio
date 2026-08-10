// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect } from "react";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { WorkflowProvider } from "../../state/WorkflowContext";
import { useWorkflow } from "../../state/useWorkflow";
import { JobRunProvider } from "../execution/JobRunProvider";
import type { ValidationReportDto, CommandPreviewDto, JobSpec } from "../../ipc/client";
import { ReviewScreen } from "./ReviewScreen";

const compileJobPreview = vi.fn();
const startJob = vi.fn();
const cancelJob = vi.fn();
const getJob = vi.fn();

vi.mock("../../ipc/client", async () => {
  const actual = await vi.importActual<typeof import("../../ipc/client")>("../../ipc/client");
  return {
    ...actual,
    compileJobPreview: (...args: unknown[]) => compileJobPreview(...args),
    startJob: (...args: unknown[]) => startJob(...args),
    cancelJob: (...args: unknown[]) => cancelJob(...args),
    getJob: (...args: unknown[]) => getJob(...args),
  };
});

vi.mock("../../ipc/events", () => ({
  onJobState: vi.fn().mockResolvedValue(vi.fn()),
  onJobStage: vi.fn().mockResolvedValue(vi.fn()),
  onJobLog: vi.fn().mockResolvedValue(vi.fn()),
  onJobMetrics: vi.fn().mockResolvedValue(vi.fn()),
  onJobArtifact: vi.fn().mockResolvedValue(vi.fn()),
  onJobCompleted: vi.fn().mockResolvedValue(vi.fn()),
}));

const READY_REPORT: ValidationReportDto = {
  status: "ready",
  errors: [],
  warnings: [],
  advisories: [],
  estimatedInputBytes: 4096,
  freeDiskBytes: 10_000_000,
};

const INVALID_REPORT: ValidationReportDto = {
  status: "invalid",
  errors: [
    {
      code: "OUTPUT_EXISTS",
      title: "Output already exists",
      message: '"C:\\out\\clean.pgn" already exists.',
      remediation: "Choose a different base filename or conflict policy.",
      logPath: null,
      technicalId: "tech-1",
    },
  ],
  warnings: [{ code: "INSUFFICIENT_DISK_SPACE", message: "Disk space is low." }],
  advisories: ["Two inputs are the same file on disk."],
  estimatedInputBytes: 4096,
  freeDiskBytes: null,
};

function previewFor(spec: JobSpec): CommandPreviewDto {
  return {
    displayCommand: "pgn-extract -s --summary -oout.pgn a.pgn",
    argv: ["-s", "--summary", "-oout.pgn", "a.pgn"],
    criteriaFiles: [],
    plannedArtifacts: [
      { kind: "uniqueGames", finalPath: `${spec.output.directory}\\${spec.output.baseName}.pgn`, temporaryPath: null },
    ],
  };
}

function Seed({ ready }: { ready: boolean }) {
  const { dispatch, state } = useWorkflow();
  useEffect(() => {
    dispatch({ type: "ADD_INPUTS", paths: ["C:\\in\\a.pgn"] });
    dispatch({ type: "SET_OUTPUT_DIRECTORY", directory: "C:\\out" });
    dispatch({ type: "SET_BASE_NAME", baseName: "clean" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    dispatch({
      type: "SET_VALIDATION_RESULT",
      report: ready ? READY_REPORT : INVALID_REPORT,
      specRevision: state.specRevision,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.specRevision]);
  return null;
}

function renderScreen(ready: boolean) {
  return render(
    <WorkflowProvider>
      <JobRunProvider>
        <Seed ready={ready} />
        <ReviewScreen />
      </JobRunProvider>
    </WorkflowProvider>,
  );
}

beforeEach(() => {
  compileJobPreview.mockReset().mockImplementation(async (spec: JobSpec) => ({
    status: "ok",
    data: previewFor(spec),
  }));
  startJob.mockReset().mockResolvedValue({
    status: "ok",
    data: { jobId: "job-1", startedAt: "2026-08-10T10:00:00Z" },
  });
  cancelJob.mockReset().mockResolvedValue({ status: "ok", data: null });
  getJob.mockReset().mockResolvedValue({ status: "error", error: { code: "INVALID_JOB_SPEC" } });
});

describe("ReviewScreen", () => {
  it("keeps Run disabled while validation has not returned ready", async () => {
    renderScreen(false);
    await waitFor(() => expect(screen.getByRole("button", { name: "Run Job" })).toBeDisabled());
  });

  it("enables Run once validation returns ready", async () => {
    renderScreen(true);
    await waitFor(() => expect(screen.getByRole("button", { name: "Run Job" })).toBeEnabled());
  });

  it('never describes duplicate handling as "keep best copy" (§10.7)', () => {
    renderScreen(true);
    expect(screen.queryByText(/best copy/i)).not.toBeInTheDocument();
  });

  it("shows the active preset and its version on a fresh draft (Merge Safely, by construction)", () => {
    renderScreen(true);
    expect(
      screen.getByText(/Started from preset: Merge Safely \(version 1\)\./),
    ).toBeInTheDocument();
  });

  it('shows "Custom configuration" once a manual edit no longer matches any preset (architecture.md §12.1: presets are versioned and inspectable, not silently invalidated)', async () => {
    function SeedCustomEdit() {
      const { dispatch } = useWorkflow();
      useEffect(() => {
        // `removeMoveNumbers: true` alone (with everything else left at
        // the fresh-draft default) is not part of ANY of the six presets'
        // own `effect` - unlike e.g. `duplicates: "suppressKeepFirst"`
        // alone, which would coincidentally match "New Games Against
        // Master"'s own effect exactly and defeat the point of this test.
        dispatch({ type: "SET_CLEANUP", patch: { removeMoveNumbers: true } });
      }, [dispatch]);
      return null;
    }
    render(
      <WorkflowProvider>
        <JobRunProvider>
          <Seed ready />
          <SeedCustomEdit />
          <ReviewScreen />
        </JobRunProvider>
      </WorkflowProvider>,
    );
    await waitFor(() =>
      expect(
        screen.getByText(/Custom configuration — not an unmodified built-in preset\./),
      ).toBeInTheDocument(),
    );
  });

  it("shows validation errors, warnings, and advisories from the backend", async () => {
    renderScreen(false);
    const warningsSection = await screen.findByRole("heading", { name: "Warnings" });
    const section = warningsSection.closest("section") as HTMLElement;
    expect(within(section).getByText(/Output already exists/)).toBeInTheDocument();
    expect(within(section).getByText("Disk space is low.")).toBeInTheDocument();
    expect(within(section).getByText("Two inputs are the same file on disk.")).toBeInTheDocument();
  });

  it("lists the planned destination artifacts from compile_job_preview", async () => {
    renderScreen(true);
    expect(await screen.findByText(/clean\.pgn/)).toBeInTheDocument();
    await waitFor(() => expect(compileJobPreview).toHaveBeenCalled());
  });

  it("the advanced view is collapsed by default and labelled for inspection only", () => {
    renderScreen(true);
    expect(screen.getByText("Advanced: view the generated command")).toBeInTheDocument();
    expect(screen.queryByText(/pgn-extract -s --summary/)).not.toBeInTheDocument();
  });

  it("expanding the advanced view shows the generated argv, and states it is never executed", async () => {
    const user = userEvent.setup();
    renderScreen(true);
    await user.click(screen.getByText("Advanced: view the generated command"));
    expect(await screen.findByText(/pgn-extract -s --summary/)).toBeInTheDocument();
    expect(screen.getByText(/never run as a shell command/)).toBeInTheDocument();
  });

  it("clicking Run starts the job and navigates to the Run & Results step", async () => {
    const user = userEvent.setup();
    function Probe() {
      const { state } = useWorkflow();
      return <p data-testid="step">{state.step}</p>;
    }
    // In the real app, AppShell only mounts ReviewScreen once state.step is
    // already "review" (reached via the three earlier screens' own Next
    // buttons) — recreate that precondition explicitly, mount-only, rather
    // than asserting on cross-screen navigation composing correctly, which
    // is App-level, not this screen's own concern.
    function AdvanceToReviewOnce() {
      const { dispatch } = useWorkflow();
      useEffect(() => {
        dispatch({ type: "GO_NEXT" }); // files -> operations
        dispatch({ type: "GO_NEXT" }); // operations -> filters
        dispatch({ type: "GO_NEXT" }); // filters -> review
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, []);
      return null;
    }
    render(
      <WorkflowProvider>
        <JobRunProvider>
          <Seed ready />
          <AdvanceToReviewOnce />
          <ReviewScreen />
          <Probe />
        </JobRunProvider>
      </WorkflowProvider>,
    );
    expect(screen.getByTestId("step")).toHaveTextContent("review");
    await waitFor(() => expect(screen.getByRole("button", { name: "Run Job" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Run Job" }));
    expect(screen.getByTestId("step")).toHaveTextContent("run-results");
    await waitFor(() => expect(startJob).toHaveBeenCalledTimes(1));
  });

  it("replaceAfterConfirmation asks for confirmation before running, and sends confirmedReplace: true", async () => {
    const user = userEvent.setup();
    function SeedReplacePolicy() {
      const { dispatch } = useWorkflow();
      useEffect(() => {
        dispatch({ type: "SET_CONFLICT_POLICY", policy: "replaceAfterConfirmation" });
      }, [dispatch]);
      return null;
    }
    render(
      <WorkflowProvider>
        <JobRunProvider>
          <Seed ready />
          <SeedReplacePolicy />
          <ReviewScreen />
        </JobRunProvider>
      </WorkflowProvider>,
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "Run Job" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Run Job" }));

    const dialog = await screen.findByRole("dialog", { name: "Replace existing files?" });
    expect(dialog).toBeInTheDocument();
    expect(startJob).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Replace and run" }));
    await waitFor(() => expect(startJob).toHaveBeenCalledTimes(1));
    const sentSpec = startJob.mock.calls[0][0] as JobSpec;
    expect(sentSpec.output.confirmedReplace).toBe(true);
  });

  it("cancelling the replace confirmation does not start the job", async () => {
    const user = userEvent.setup();
    function SeedReplacePolicy() {
      const { dispatch } = useWorkflow();
      useEffect(() => {
        dispatch({ type: "SET_CONFLICT_POLICY", policy: "replaceAfterConfirmation" });
      }, [dispatch]);
      return null;
    }
    render(
      <WorkflowProvider>
        <JobRunProvider>
          <Seed ready />
          <SeedReplacePolicy />
          <ReviewScreen />
        </JobRunProvider>
      </WorkflowProvider>,
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "Run Job" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Run Job" }));
    await screen.findByRole("dialog", { name: "Replace existing files?" });
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(startJob).not.toHaveBeenCalled();
  });
});
