// SPDX-License-Identifier: GPL-3.0-or-later
/** Artifact list with size, Open File, Reveal in Folder, and Copy Path
 * (architecture.md §13.7). */
import { useState } from "react";
import type { OutputArtifact } from "../../ipc/client";
import { openPath, revealPath } from "../../ipc/client";
import { ARTIFACT_KIND_LABELS, formatBytes } from "../../state/formatters";
import { Button } from "../../components/Button";
import { Banner } from "../../components/Banner";
import { copyToClipboard } from "./clipboard";
import "./ArtifactList.css";

export interface ArtifactListProps {
  artifacts: OutputArtifact[];
}

export function ArtifactList({ artifacts }: ArtifactListProps) {
  const [copiedPath, setCopiedPath] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  if (artifacts.length === 0) {
    return <p className="workflow-screen__section-help">No output files were published.</p>;
  }

  async function handleOpen(path: string) {
    setActionError(null);
    const result = await openPath(path);
    if (result.status === "error") setActionError(result.error.message);
  }

  async function handleReveal(path: string) {
    setActionError(null);
    const result = await revealPath(path);
    if (result.status === "error") setActionError(result.error.message);
  }

  async function handleCopy(path: string) {
    const ok = await copyToClipboard(path);
    setCopiedPath(ok ? path : null);
    if (!ok) setActionError("Could not copy the path to the clipboard.");
  }

  return (
    <div>
      {actionError && (
        <Banner tone="danger" role="alert">
          {actionError}
        </Banner>
      )}
      <ul className="artifact-list">
        {artifacts.map((artifact) => (
          <li key={artifact.path} className="artifact-list__row">
            <div className="artifact-list__info">
              <p className="artifact-list__kind">{ARTIFACT_KIND_LABELS[artifact.kind]}</p>
              <p className="artifact-list__path">{artifact.path}</p>
              <p className="artifact-list__size">{formatBytes(artifact.sizeBytes)}</p>
            </div>
            <div className="artifact-list__actions">
              <Button variant="secondary" onClick={() => void handleOpen(artifact.path)}>
                Open File
              </Button>
              <Button variant="secondary" onClick={() => void handleReveal(artifact.path)}>
                Reveal in Folder
              </Button>
              <Button variant="ghost" onClick={() => void handleCopy(artifact.path)}>
                {copiedPath === artifact.path ? "Copied!" : "Copy Path"}
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
