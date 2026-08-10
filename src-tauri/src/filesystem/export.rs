// SPDX-License-Identifier: GPL-3.0-or-later
//! Exporting a Rust-owned document (the job manifest, architecture.md
//! §13.7/§15.3 - "Save Job") to a user-chosen destination file. Distinct
//! from [`super::publish`] (engine-artifact publication with
//! conflict-policy resolution across `<base>.pgn`/`.duplicates.pgn`/etc.
//! naming) and [`super::workspace`] (the fixed, job-owned
//! `manifest.json` path): this module writes **one** small file to an
//! **arbitrary user-chosen path**, picked via the native save dialog, that
//! lives outside any job workspace. Architecture.md §11.1's "no backend
//! command may open a source PGN with write access" and §11.5's "silent
//! overwrite is prohibited" still apply to this destination even though it
//! is not a PGN artifact.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::domain::PublicError;
use crate::errors;

use super::identity;

/// Validates a user-chosen export destination the same way every other
/// output path in this codebase is validated (architecture.md §11.2),
/// reusing `filesystem::identity`'s existing primitives rather than
/// inventing new path-safety logic:
///
/// - the parent folder must exist, be a real directory, and be writable
///   ([`identity::probe_writable`] - the same probe `validate_job`'s own
///   destination-directory step uses);
/// - the file name must not be a reserved Windows device name
///   ([`identity::is_reserved_windows_device_name`]);
/// - the destination must not be file-identity-aliased to any path in
///   `protected_paths` ([`identity::is_aliased`] - the same two-layer
///   existing-or-not-yet-existing check `validate_job`'s aliasing step
///   uses for input/output collisions), so "Save Job" can never silently
///   clobber one of the job's own source or artifact files.
///
/// Overwriting an unrelated, pre-existing file at the destination is
/// intentionally **not** rejected here: the native save dialog itself
/// already asks the user to confirm replacing an existing file before ever
/// returning a path (standard OS "Save As" behavior) - the same kind of
/// explicit user confirmation architecture.md §11.5 requires before
/// replacing an output. There is no second confirmation to add on top of
/// what the OS dialog already gathered.
pub fn validate_export_destination(
    destination: &Path,
    protected_paths: &[PathBuf],
) -> Result<(), PublicError> {
    if !destination.is_absolute() {
        return Err(errors::invalid_job_spec(
            "destination",
            "must be an absolute path",
        ));
    }
    let file_name = destination
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if file_name.is_empty() {
        return Err(errors::invalid_job_spec("destination", "must name a file"));
    }
    if identity::is_reserved_windows_device_name(file_name) {
        return Err(errors::invalid_job_spec(
            "destination",
            &format!("\"{file_name}\" is a reserved device name"),
        ));
    }

    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    match std::fs::metadata(parent) {
        Ok(meta) if !meta.is_dir() => {
            return Err(errors::output_not_writable_not_a_directory(parent));
        }
        Ok(_) => {
            if let Err(e) = identity::probe_writable(parent) {
                return Err(errors::output_not_writable_io(parent, &e));
            }
        }
        Err(e) => return Err(errors::output_not_writable_io(parent, &e)),
    }

    for protected in protected_paths {
        if !protected.exists() {
            continue; // nothing to alias against if it no longer exists
        }
        if identity::is_aliased(protected, destination).unwrap_or(false) {
            return Err(errors::export_destination_collision(destination, protected));
        }
    }

    Ok(())
}

/// Writes `bytes` to `destination` durably and, so far as the platform
/// allows, atomically: write + `sync_all` a same-directory temp file, then
/// rename onto the final name (architecture.md §11.4's "using the
/// destination directory for temporary outputs improves the likelihood
/// that rename is atomic"). Cleans up the temp file if the rename fails.
///
/// Unlike `publish::publish_all`, this performs a **replacing** rename
/// (`std::fs::rename`) rather than a no-replace one:
/// [`validate_export_destination`]'s caller only reaches this after the
/// native save dialog already gathered the user's explicit confirmation to
/// replace any pre-existing file at that exact path (see that function's
/// doc comment) - the same trust `filesystem::workspace::
/// write_final_manifest` places in its own (internal, always-fresh)
/// manifest path.
pub fn write_export_file_atomically(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let temp_name = format!(
        ".pgnstudio-export-tmp-{}",
        &Uuid::new_v4().simple().to_string()[..8]
    );
    let temp_path = parent.join(temp_name);
    {
        let mut file = std::fs::File::create(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    match std::fs::rename(&temp_path, destination) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ErrorCode;

    #[test]
    fn validate_export_destination_accepts_a_writable_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let destination = tmp.path().join("job.pgnstudio-job.json");
        assert!(validate_export_destination(&destination, &[]).is_ok());
    }

    #[test]
    fn validate_export_destination_rejects_a_relative_path() {
        let err = validate_export_destination(Path::new("relative\\job.json"), &[]).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidJobSpec);
    }

    #[test]
    fn validate_export_destination_rejects_a_reserved_device_name() {
        let tmp = tempfile::tempdir().unwrap();
        let destination = tmp.path().join("NUL.json");
        let err = validate_export_destination(&destination, &[]).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidJobSpec);
    }

    #[test]
    fn validate_export_destination_rejects_a_missing_parent_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let destination = tmp.path().join("does-not-exist").join("job.json");
        let err = validate_export_destination(&destination, &[]).unwrap_err();
        assert_eq!(err.code(), ErrorCode::OutputNotWritable);
    }

    #[test]
    fn validate_export_destination_rejects_a_collision_with_a_protected_path() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("master.pgn");
        std::fs::write(&source, b"[Event \"x\"]\n").unwrap();
        // Saving directly on top of a tracked input/artifact must be
        // rejected even though nothing about the *parent folder* is wrong.
        let err = validate_export_destination(&source, std::slice::from_ref(&source)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InputOutputCollision);
    }

    #[test]
    fn validate_export_destination_allows_an_unrelated_existing_file() {
        // Overwriting an unrelated pre-existing file is allowed here - the
        // native save dialog is the confirmation gate (see this module's
        // doc comment), not a second check in Rust.
        let tmp = tempfile::tempdir().unwrap();
        let destination = tmp.path().join("job.json");
        std::fs::write(&destination, b"old").unwrap();
        assert!(validate_export_destination(&destination, &[]).is_ok());
    }

    #[test]
    fn write_export_file_atomically_writes_the_given_bytes_and_leaves_no_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let destination = tmp.path().join("job.json");
        write_export_file_atomically(&destination, b"{\"hello\":true}").unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"{\"hello\":true}");
        let leftover_temp_files = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains("pgnstudio-export-tmp")
            })
            .count();
        assert_eq!(leftover_temp_files, 0);
    }

    #[test]
    fn write_export_file_atomically_replaces_an_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let destination = tmp.path().join("job.json");
        std::fs::write(&destination, b"OLD CONTENT").unwrap();
        write_export_file_atomically(&destination, b"NEW CONTENT").unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"NEW CONTENT");
    }
}
