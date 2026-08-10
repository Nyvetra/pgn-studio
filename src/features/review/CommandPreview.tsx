// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The optional, collapsed "advanced" view (architecture.md §13.5): the
 * generated `pgn-extract` command and criteria-file contents, for
 * inspection only. `displayCommand`/`argv` come straight from
 * `compile_job_preview` — nothing here re-derives or re-renders the
 * command, and nothing in this codebase ever executes this text (design-02
 * §1.6's never-executed guarantee lives entirely on the Rust side).
 */
import type { CommandPreviewDto, PublicError } from "../../ipc/client";
import { Banner } from "../../components/Banner";
import "./CommandPreview.css";

export interface CommandPreviewProps {
  open: boolean;
  onToggle: (open: boolean) => void;
  loading: boolean;
  preview: CommandPreviewDto | null;
  error: PublicError | null;
}

export function CommandPreview({ open, onToggle, loading, preview, error }: CommandPreviewProps) {
  return (
    <details
      className="command-preview"
      open={open}
      onToggle={(event) => onToggle(event.currentTarget.open)}
    >
      <summary>Advanced: view the generated command</summary>
      <div className="command-preview__body">
        <Banner tone="info">
          This is shown for inspection only. It is never run as a shell command — PGN Studio always
          launches the engine directly with this exact list of arguments, with no shell involved.
        </Banner>

        {loading && <p role="status">Generating preview…</p>}
        {error && (
          <Banner tone="danger" role="alert">
            Could not generate a preview: {error.message}
          </Banner>
        )}

        {preview && (
          <>
            <h4>Command</h4>
            <pre className="command-preview__code">{preview.displayCommand}</pre>

            <h4>Arguments</h4>
            <ol className="command-preview__argv">
              {preview.argv.map((arg, index) => (
                // Positional argv tokens have no other stable identity to key on.
                <li key={index}>
                  <code>{arg}</code>
                </li>
              ))}
            </ol>

            {preview.criteriaFiles.length > 0 && (
              <>
                <h4>Generated criteria files</h4>
                {preview.criteriaFiles.map((file) => (
                  <div key={file.relativePath}>
                    <p className="command-preview__file-name">{file.relativePath}</p>
                    <pre className="command-preview__code">{file.content}</pre>
                  </div>
                ))}
              </>
            )}
          </>
        )}
      </div>
    </details>
  );
}
