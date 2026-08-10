# jobs/

Job lifecycle management and event emission (architecture.md §9.1, §10.9,
§10.10): the single-job-at-a-time runner, per-job working directory
creation (architecture.md §11.3), progress/log/metric event emission over
`job://*` channels, and cancellation (transition to `Cancelling`, grace
period, force-terminate, cleanup).

Empty in Phase 0.
