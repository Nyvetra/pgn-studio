# engine/

The `pgn-extract` engine adapter (architecture.md §7.1, §10). This is the
*only* place allowed to know about the sidecar's actual CLI flags and
process lifecycle - the rest of the app talks to it through
`EngineCapabilities` and `CompiledEngineCommand`.

Planned layout (architecture.md §8), none of which exists yet:

| File | Purpose |
|---|---|
| `mod.rs` | Module root, re-exports the public adapter surface. |
| `capability.rs` | `EngineCapabilities` self-test (architecture.md §10.4) - detects what the pinned binary actually supports at startup instead of trusting a hardcoded flag list. |
| `command_compiler.rs` | Pure `JobSpec + EngineCapabilities -> CompiledEngineCommand` function (architecture.md §10.5) with `display_command` for UI preview only - never executed. |
| `pgn_extract.rs` | Process spawning: argument array only, never a shell (architecture.md §10.3, §16.2). |
| `output_parser.rs` | Parses stdout/stderr/exit status into `ProcessingMetrics` / `JobWarning` (architecture.md §10.9), never fabricating a metric that could not be measured. |

Empty in Phase 0, which only pins the upstream revision
(`engine-src/upstream.lock`) and bundles static resources
(`src-tauri/resources/pgn-extract/`). Phase 1 ("Engine adapter proof")
implements `capability.rs` and a minimal `command_compiler.rs` /
`pgn_extract.rs` for a two-file merge.
