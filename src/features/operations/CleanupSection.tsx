// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Comments/variations/NAG cleanup (architecture.md §13.3). Kept as one
 * section with the three headline toggles up front and the less commonly
 * needed ones tucked behind a native `<details>` disclosure — progressive
 * disclosure (architecture.md §4.6) without hiding real functionality.
 */
import { useState } from "react";
import type { CleanupOptions } from "../../ipc/client";
import { Checkbox } from "../../components/Checkbox";
import { TextField } from "../../components/TextField";
import { Button } from "../../components/Button";
import { isValidTagIdentifier } from "../../state/filterMapping";

export interface CleanupSectionProps {
  cleanup: CleanupOptions;
  onChange: (patch: Partial<CleanupOptions>) => void;
}

export function CleanupSection({ cleanup, onChange }: CleanupSectionProps) {
  const [tagDraft, setTagDraft] = useState("");
  const [tagError, setTagError] = useState<string | undefined>(undefined);

  function addTag() {
    const value = tagDraft.trim();
    if (!value) return;
    if (!isValidTagIdentifier(value)) {
      setTagError("Tag names must start with a letter and contain only letters and digits.");
      return;
    }
    if (cleanup.removeTags.includes(value)) {
      setTagError(`"${value}" is already in the list.`);
      return;
    }
    onChange({ removeTags: [...cleanup.removeTags, value] });
    setTagDraft("");
    setTagError(undefined);
  }

  return (
    <section aria-labelledby="operations-cleanup-heading">
      <h3 id="operations-cleanup-heading">Comments, variations &amp; NAGs</h3>
      <p className="workflow-screen__section-help">
        These remove annotation text from the games themselves. This cannot target only specific
        kinds of annotation (such as clock times or engine evaluations) — removing comments removes
        all comment text, since that is all the engine can tell apart.
      </p>
      <Checkbox
        label="Remove comments"
        checked={cleanup.removeComments}
        onCheckedChange={(removeComments) => onChange({ removeComments })}
      />
      <Checkbox
        label="Remove variations"
        help="Removes side-line moves, keeping only the main line."
        checked={cleanup.removeVariations}
        onCheckedChange={(removeVariations) => onChange({ removeVariations })}
      />
      <Checkbox
        label="Remove NAGs (move annotations like !, ?, !!)"
        checked={cleanup.removeNags}
        onCheckedChange={(removeNags) => onChange({ removeNags })}
      />

      <details className="operations-advanced-disclosure">
        <summary>More cleanup options</summary>
        <Checkbox
          label="Remove move numbers"
          checked={cleanup.removeMoveNumbers}
          onCheckedChange={(removeMoveNumbers) => onChange({ removeMoveNumbers })}
        />
        <Checkbox
          label="Remove result markers from the movetext"
          help='Removes the trailing "1-0"/"0-1"/"1/2-1/2" from the move text itself. The Result header tag is unaffected.'
          checked={cleanup.removeResults}
          onCheckedChange={(removeResults) => onChange({ removeResults })}
        />

        <div className="operations-tag-editor">
          <TextField
            label="Remove specific header tags"
            help='e.g. "Annotator" or "SourceDate". Add one at a time.'
            value={tagDraft}
            onValueChange={(value) => {
              setTagDraft(value);
              setTagError(undefined);
            }}
            error={tagError}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                addTag();
              }
            }}
          />
          <Button type="button" variant="secondary" onClick={addTag}>
            Add
          </Button>
        </div>
        {cleanup.removeTags.length > 0 && (
          <ul className="operations-tag-list" aria-label="Tags that will be removed">
            {cleanup.removeTags.map((tag) => (
              <li key={tag}>
                {tag}
                <button
                  type="button"
                  className="operations-tag-list__remove"
                  onClick={() => onChange({ removeTags: cleanup.removeTags.filter((t) => t !== tag) })}
                  aria-label={`Stop removing the ${tag} tag`}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
      </details>
    </section>
  );
}
