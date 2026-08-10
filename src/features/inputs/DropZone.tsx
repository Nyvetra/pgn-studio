// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Drop zone + Add Files + Add Folder (architecture.md §13.2). "Add Files"
 * and "Add Folder" are both backed by real, mocked-in-tests IPC commands
 * (`select_input_files`/`select_input_directory`); native OS drag-and-drop
 * is a best-effort enhancement (`useTauriFileDrop.ts`).
 *
 * Known backend gap (reported, not worked around): there is no IPC command
 * to *enumerate* a folder's `.pgn` files, and the frontend has no direct
 * filesystem-read capability (architecture.md §16.3). `select_input_directory`
 * therefore can only return the chosen folder's path, not its contents —
 * this component is honest about that limitation rather than pretending to
 * scan the folder.
 */
import { useId, useState } from "react";
import { selectInputDirectory, selectInputFiles } from "../../ipc/client";
import { Button } from "../../components/Button";
import { Banner } from "../../components/Banner";
import { useTauriFileDrop } from "./useTauriFileDrop";
import "./DropZone.css";

export interface DropZoneProps {
  onFilesChosen: (paths: string[]) => void;
}

export function DropZone({ onFilesChosen }: DropZoneProps) {
  const [isDragOver, setIsDragOver] = useState(false);
  const [pickedFolder, setPickedFolder] = useState<string | null>(null);
  const labelId = useId();

  useTauriFileDrop(onFilesChosen);

  async function handleAddFiles() {
    const result = await selectInputFiles();
    if (result.status === "ok" && result.data.length > 0) {
      onFilesChosen(result.data);
    }
  }

  async function handleAddFolder() {
    const result = await selectInputDirectory();
    if (result.status === "ok" && result.data) {
      setPickedFolder(result.data);
    }
  }

  return (
    <div className="drop-zone-wrap">
      {/* Purely a visual drop target — the drag handlers only toggle a
          hover style; every actual interaction (keyboard or pointer) goes
          through the two labelled buttons below, so no ARIA interactive
          role is needed here. */}
      <div
        className={["drop-zone", isDragOver ? "drop-zone--active" : ""].filter(Boolean).join(" ")}
        aria-labelledby={labelId}
        onDragOver={(event) => {
          event.preventDefault();
          setIsDragOver(true);
        }}
        onDragLeave={() => setIsDragOver(false)}
        onDrop={(event) => {
          event.preventDefault();
          setIsDragOver(false);
        }}
      >
        <p className="drop-zone__instructions" id={labelId}>
          Drag PGN files here, or use the buttons below
        </p>
        <div className="drop-zone__actions">
          <Button variant="primary" onClick={() => void handleAddFiles()}>
            Add Files
          </Button>
          <Button variant="secondary" onClick={() => void handleAddFolder()}>
            Add Folder
          </Button>
        </div>
      </div>
      {pickedFolder && (
        <Banner tone="info" role="status">
          <p>
            Selected folder: <code>{pickedFolder}</code>
          </p>
          <p>
            PGN Studio can&rsquo;t scan a folder&rsquo;s contents automatically yet — use &ldquo;Add
            Files&rdquo; above and choose the .pgn files inside it directly.
          </p>
          <Button variant="ghost" onClick={() => setPickedFolder(null)}>
            Dismiss
          </Button>
        </Banner>
      )}
    </div>
  );
}
