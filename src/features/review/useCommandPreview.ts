// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Fetches `compile_job_preview` for the Review screen (architecture.md
 * §13.5). Its response serves two parts of that screen: the always-visible
 * "destination artifacts" list (`plannedArtifacts`) and the optional,
 * collapsed advanced view (`displayCommand`/`argv`/`criteriaFiles`) — both
 * come from one call, so this hook fetches unconditionally while mounted
 * rather than gating on the disclosure being open.
 */
import { useEffect, useState } from "react";
import { compileJobPreview, type CommandPreviewDto, type PublicError } from "../../ipc/client";
import { buildJobSpec } from "../../state/jobSpecBuilder";
import type { WorkflowState } from "../../state/workflowReducer";

export interface CommandPreviewState {
  preview: CommandPreviewDto | null;
  loading: boolean;
  error: PublicError | null;
}

export function useCommandPreview(state: WorkflowState): CommandPreviewState {
  // `loading` only ever flips to `false` once a response lands (from the
  // async `.then` continuation, never synchronously at the top of the
  // effect — react-hooks/set-state-in-effect flags the latter as a
  // cascading-render anti-pattern). One consequence: refetching after an
  // edit does not flash back to a loading state, it just keeps the
  // previous preview visible until the new one replaces it — an accepted
  // trade-off for a call this cheap and local.
  const [result, setResult] = useState<CommandPreviewState>({
    preview: null,
    loading: true,
    error: null,
  });

  useEffect(() => {
    let cancelled = false;
    const spec = buildJobSpec(state);
    void compileJobPreview(spec).then((response) => {
      if (cancelled) return;
      if (response.status === "ok") {
        setResult({ preview: response.data, loading: false, error: null });
      } else {
        setResult({ preview: null, loading: false, error: response.error });
      }
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.specRevision]);

  return result;
}
