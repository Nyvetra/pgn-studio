# state/

Cross-feature application state (job progress, recent jobs, settings cache,
navigation/workflow-step state) as described in architecture.md §13 and
§14.2.

Empty in Phase 0: there is no job or settings state yet. Phase 2 introduces
the five-step workflow (`architecture.md` §13.1) and will need state here to
track the in-progress `JobSpec` draft and event-correlated job status.
