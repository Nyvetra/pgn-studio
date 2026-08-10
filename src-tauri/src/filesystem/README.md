# filesystem/

Filesystem safety port (architecture.md §7.1, §11): source-immutability
enforcement, path validation and canonicalization, input/output-collision
and symlink-aliasing detection, atomic output publication (temp file in the
destination directory, verify, then rename), and conflict-policy resolution
(`Fail` / `AddNumericSuffix` / `ReplaceAfterConfirmation`).

All direct filesystem operations must live behind this port so the rest of
the app (and definitely the frontend) never touches paths directly.

Empty in Phase 0.
