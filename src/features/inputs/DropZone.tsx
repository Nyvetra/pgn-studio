// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Drop zone + Add Files + Add Folder (architecture.md §13.2). "Add Files"
 * is a single native dialog call (`select_input_files`); "Add Folder" picks
 * a folder the same way (`select_input_directory`) and then scans it via
 * the real `scan_input_directory` command, letting the user review the
 * matched files - including any truncation warning - before they are added
 * (`AddFolderPanel.tsx`, which also owns the non-recursive-by-default
 * decision). Native OS drag-and-drop is a best-effort enhancement
 * (`useTauriFileDrop.ts`).
 */
import { useId, useState } from "react";
import { selectInputFiles } from "../../ipc/client";
import { Button } from "../../components/Button";
import { AddFolderPanel } from "./AddFolderPanel";
import { useTauriFileDrop } from "./useTauriFileDrop";
import "./DropZone.css";

export interface DropZoneProps {
  onFilesChosen: (paths: string[]) => void;
}

export function DropZone({ onFilesChosen }: DropZoneProps) {
  const [isDragOver, setIsDragOver] = useState(false);
  const labelId = useId();

  useTauriFileDrop(onFilesChosen);

  async function handleAddFiles() {
    const result = await selectInputFiles();
    if (result.status === "ok" && result.data.length > 0) {
      onFilesChosen(result.data);
    }
  }

  return (
    <div className="drop-zone-wrap">
      {/* Purely a visual drop target — the drag handlers only toggle a
          hover style; every actual interaction (keyboard or pointer) goes
          through the controls below, so no ARIA interactive role is needed
          here. */}
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
          <AddFolderPanel onFilesChosen={onFilesChosen} />
        </div>
      </div>
    </div>
  );
}
