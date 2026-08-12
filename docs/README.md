# docs/

Project documentation, per the repository structure in architecture.md §8.

| File | Content |
|---|---|
| [`user-guide.md`](./user-guide.md) | End-user walkthrough of the five-step workflow (Files → Operations → Filters → Review → Run & Results). |
| [`engine-capabilities.md`](./engine-capabilities.md) | What the pinned `pgn-extract` build actually supports, including several verified surprises that corrected earlier design assumptions. |
| [`duplicate-semantics.md`](./duplicate-semantics.md) | Architecture.md §10.7's duplicate-detection semantics ("keep first copy," input order as retention priority) and the annotated-duplicate warning's exact meaning and limits. |
| [`release-process.md`](./release-process.md) | How a release is built and verified end to end, including an honest accounting of what cannot be verified from a Windows-only development machine. |
| [`acceptance-criteria.md`](./acceptance-criteria.md) | The project's own item-by-item self-assessment against architecture.md §25's MVP acceptance criteria - verified / not verified / not achievable here, with evidence for each. |
| `adr/` | Architecture Decision Records for choices made after `architecture.md` was written (architecture.md §26). Not yet populated. |

All content above reflects the real, tested application as of Phase 6 -
none of it is aspirational or written ahead of the behavior it describes.
