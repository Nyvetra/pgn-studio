// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Ordered source-file list (architecture.md §13.2): reorder controls, file
 * size, warnings, and a remove action per row. Reordering uses Move Up/Down
 * buttons rather than drag handles — §13.2 offers either ("drag handles
 * *or* Move Up/Down controls"), and buttons are unambiguously keyboard- and
 * screen-reader-accessible without a custom drag implementation.
 */
import { formatBytes } from "../../state/formatters";
import type { DraftInput } from "../../types/workflow";
import { Button } from "../../components/Button";
import "./SourceList.css";

export interface SourceListProps {
  inputs: DraftInput[];
  onMove: (id: string, direction: "up" | "down") => void;
  onRemove: (id: string) => void;
}

export function SourceList({ inputs, onMove, onRemove }: SourceListProps) {
  if (inputs.length === 0) {
    return (
      <p className="source-list__empty">
        No files added yet. Use &ldquo;Add Files&rdquo; above to choose one or more .pgn files.
      </p>
    );
  }

  return (
    <ol className="source-list" aria-label="Source files, in processing order">
      {inputs.map((input, index) => (
        <li className="source-row" key={input.id}>
          <span className="source-row__priority" aria-hidden="true">
            {index + 1}
          </span>
          <div className="source-row__details">
            <p className="source-row__name" title={input.path}>
              {input.displayName}
            </p>
            <p className="source-row__meta">
              {!input.inspected ? "Checking…" : formatBytes(input.sizeBytes)}
              {input.inspected && input.extensionOk === false && (
                <span className="source-row__badge source-row__badge--warning">Not a .pgn file</span>
              )}
              {input.inspected && input.isReadable === false && (
                <span className="source-row__badge source-row__badge--danger">Unreadable</span>
              )}
            </p>
            {input.warnings.length > 0 && (
              <ul className="source-row__warnings">
                {input.warnings.map((warning) => (
                  <li key={warning}>
                    <span aria-hidden="true">⚠</span> {warning}
                  </li>
                ))}
              </ul>
            )}
          </div>
          <div className="source-row__actions">
            <Button
              variant="ghost"
              onClick={() => onMove(input.id, "up")}
              disabled={index === 0}
              aria-label={`Move ${input.displayName} up`}
            >
              ↑
            </Button>
            <Button
              variant="ghost"
              onClick={() => onMove(input.id, "down")}
              disabled={index === inputs.length - 1}
              aria-label={`Move ${input.displayName} down`}
            >
              ↓
            </Button>
            <Button
              variant="ghost"
              onClick={() => onRemove(input.id)}
              aria-label={`Remove ${input.displayName} from this job`}
            >
              Remove
            </Button>
          </div>
        </li>
      ))}
    </ol>
  );
}
