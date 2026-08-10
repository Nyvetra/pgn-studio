# docs/

Reserved for the project documentation set described in architecture.md §8:

| Planned file | Content | Lands in |
|---|---|---|
| `user-guide.md` | End-user walkthrough of the five-step workflow (Files -> Operations -> Filters -> Review -> Run & Results). | Phase 2+, once that workflow exists. |
| `engine-capabilities.md` | Human-readable record of what the pinned `pgn-extract` build actually supports, generated from/kept in sync with `EngineCapabilities` (architecture.md §10.4). | Phase 1, once the capability self-test exists. |
| `duplicate-semantics.md` | Explains architecture.md §10.7's duplicate-detection semantics ("keep first copy", input order as retention priority, why comments/variations aren't part of duplicate identity) for both contributors and curious users. | Phase 3 (Deduplication and audit). |
| `release-process.md` | How a signed/notarized release is cut end to end (architecture.md §21). | Phase 6 (Persistence and release quality). |
| `adr/` | Architecture Decision Records for choices made after `PGN-Studio-architecture.md` was written (architecture.md §26). | As needed. |

None of these exist yet. Phase 0 intentionally does not stub them out with
placeholder prose, because most of their content depends on Phase 1+
behavior that has not been built (an "engine capabilities" doc with nothing
to describe, or a "release process" doc for a release process that doesn't
exist, would be misleading rather than useful). This README is the
placeholder for the directory itself, per the repository structure in
architecture.md §8.
