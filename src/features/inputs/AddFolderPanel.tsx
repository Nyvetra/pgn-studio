// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Add Folder" (architecture.md §13.2): pick a folder, scan it for
 * candidate `.pgn` files (backed by the real `scan_input_directory`
 * command), and let the user review the result - including any truncation
 * warning - before the files are actually added to the source list.
 *
 * **Recursion default (product decision, documented in full in
 * `src-tauri/src/filesystem/folder_scan.rs`'s module doc comment):** a scan
 * starts **non-recursive** every time a new folder is picked - only that
 * folder's own files are matched. The user must explicitly check "Include
 * subfolders" to recurse; doing so (or toggling the extension override)
 * re-runs the scan immediately with the new options, so the effect of each
 * control is always visible before anything is added.
 */
import { useEffect, useState } from "react";
import { scanInputDirectory, selectInputDirectory } from "../../ipc/client";
import type { DirectoryScanDto } from "../../ipc/client";
import { Banner } from "../../components/Banner";
import { Button } from "../../components/Button";
import { Checkbox } from "../../components/Checkbox";
import { formatBytes } from "../../state/formatters";
import "./AddFolderPanel.css";

export interface AddFolderPanelProps {
  onFilesChosen: (paths: string[]) => void;
}

/** A scan can match up to `filesystem::folder_scan::MAX_MATCHED_FILES`
 * (10,000) files; rendering every one as a DOM row would make the review
 * list itself the slow part. The full list (not just this preview) is
 * still what gets added on confirm. */
const PREVIEW_LIMIT = 50;

type ScanState =
  | { phase: "idle" }
  | { phase: "scanning" }
  | { phase: "reviewing"; result: DirectoryScanDto }
  | { phase: "error"; message: string };

function summarize(result: DirectoryScanDto, recursive: boolean): string {
  const count = result.files.length;
  const noun = count === 1 ? "file" : "files";
  if (!recursive) {
    return `Found ${count} ${noun} in this folder.`;
  }
  const folderNoun = result.directoriesScanned === 1 ? "folder" : "folders";
  return `Found ${count} ${noun}, including subfolders (${result.directoriesScanned} ${folderNoun} scanned).`;
}

export function AddFolderPanel({ onFilesChosen }: AddFolderPanelProps) {
  const [pickedDirectory, setPickedDirectory] = useState<string | null>(null);
  const [recursive, setRecursive] = useState(false);
  const [includeAllExtensions, setIncludeAllExtensions] = useState(false);
  const [scan, setScan] = useState<ScanState>({ phase: "idle" });

  // The "scanning" transition is set from the event handlers below (button
  // click, checkbox change), never synchronously inside this effect body -
  // this effect only performs the async scan and reports its *result*,
  // matching `useInputInspectionEffect`'s same shape (dispatch only inside
  // the `.then()` callback, nothing synchronous in the effect body itself).
  useEffect(() => {
    if (pickedDirectory === null) return;
    let cancelled = false;
    void scanInputDirectory(pickedDirectory, { recursive, includeAllExtensions }).then((result) => {
      if (cancelled) return;
      if (result.status === "error") {
        setScan({ phase: "error", message: result.error.message });
        return;
      }
      setScan({ phase: "reviewing", result: result.data });
    });
    return () => {
      cancelled = true;
    };
  }, [pickedDirectory, recursive, includeAllExtensions]);

  async function handleAddFolder() {
    const result = await selectInputDirectory();
    if (result.status === "error") {
      setScan({ phase: "error", message: result.error.message });
      return;
    }
    if (!result.data) return; // user cancelled the OS folder picker
    // A fresh pick always starts from the safe, non-recursive default -
    // options do not silently carry over from a previous folder.
    setRecursive(false);
    setIncludeAllExtensions(false);
    setScan({ phase: "scanning" });
    setPickedDirectory(result.data);
  }

  function handleRecursiveChange(next: boolean) {
    setRecursive(next);
    setScan({ phase: "scanning" });
  }

  function handleIncludeAllExtensionsChange(next: boolean) {
    setIncludeAllExtensions(next);
    setScan({ phase: "scanning" });
  }

  function handleDismiss() {
    setPickedDirectory(null);
    setScan({ phase: "idle" });
    setRecursive(false);
    setIncludeAllExtensions(false);
  }

  function handleConfirmAdd() {
    if (scan.phase !== "reviewing") return;
    onFilesChosen(scan.result.files.map((file) => file.path));
    handleDismiss();
  }

  const showPanel = scan.phase !== "idle";

  return (
    <div className="add-folder-panel">
      <Button variant="secondary" onClick={() => void handleAddFolder()}>
        Add Folder
      </Button>

      {showPanel && (
        <Banner
          tone={scan.phase === "error" ? "danger" : "info"}
          role={scan.phase === "error" ? "alert" : "status"}
          className="add-folder-panel__review"
        >
          {pickedDirectory && (
            <p className="add-folder-panel__directory">
              Folder: <code>{pickedDirectory}</code>
            </p>
          )}

          <div className="add-folder-panel__options">
            <Checkbox
              label="Include subfolders"
              help="Off by default so a large folder tree can't silently pull in thousands of files. Rescans immediately when changed."
              checked={recursive}
              onCheckedChange={handleRecursiveChange}
              disabled={scan.phase === "scanning"}
            />
            <Checkbox
              label="Include files without a .pgn extension"
              help="Advanced. Normally only .pgn files (any letter case) are matched."
              checked={includeAllExtensions}
              onCheckedChange={handleIncludeAllExtensionsChange}
              disabled={scan.phase === "scanning"}
            />
          </div>

          {scan.phase === "scanning" && <p>Scanning…</p>}

          {scan.phase === "error" && <p>{scan.message}</p>}

          {scan.phase === "reviewing" && (
            <>
              <p className="add-folder-panel__summary">{summarize(scan.result, recursive)}</p>

              {scan.result.truncated &&
                scan.result.truncationNotes.map((note) => (
                  <p key={note} className="add-folder-panel__truncation">
                    <span aria-hidden="true">⚠</span> {note}
                  </p>
                ))}

              {scan.result.files.length > 0 ? (
                <ul className="add-folder-panel__preview">
                  {scan.result.files.slice(0, PREVIEW_LIMIT).map((file) => (
                    <li key={file.path}>
                      <span className="add-folder-panel__preview-name">{file.displayName}</span>
                      <span className="add-folder-panel__preview-meta">
                        {file.isReadable ? formatBytes(file.sizeBytes) : "unreadable"}
                      </span>
                    </li>
                  ))}
                  {scan.result.files.length > PREVIEW_LIMIT && (
                    <li className="add-folder-panel__preview-more">
                      …and {scan.result.files.length - PREVIEW_LIMIT} more
                    </li>
                  )}
                </ul>
              ) : (
                <p>No matching files were found.</p>
              )}
            </>
          )}

          <div className="add-folder-panel__actions">
            {scan.phase === "reviewing" && scan.result.files.length > 0 && (
              <Button variant="primary" onClick={handleConfirmAdd}>
                Add {scan.result.files.length} {scan.result.files.length === 1 ? "File" : "Files"}
              </Button>
            )}
            <Button variant="ghost" onClick={handleDismiss}>
              {scan.phase === "error" ? "Dismiss" : "Cancel"}
            </Button>
          </div>
        </Banner>
      )}
    </div>
  );
}
