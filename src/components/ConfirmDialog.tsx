// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A native `<dialog>`-based confirmation modal (architecture.md §11.5:
 * "replacing an existing output requires explicit user confirmation...
 * silent overwrite is prohibited"). Using the browser's own `<dialog>`
 * element gives modal focus containment, `::backdrop`, and Escape-to-close
 * for free, which is more reliable than a hand-rolled focus trap.
 *
 * The default-focused control is Cancel, not Confirm — for a destructive
 * confirmation (replacing files), the safe action should be what an
 * accidental Enter keypress activates.
 */
import { useEffect, useId, useRef } from "react";
import type { ReactNode } from "react";
import { Button } from "./Button";
import "./ConfirmDialog.css";

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  danger = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const descriptionId = useId();

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      // `showModal`/`close` are unimplemented in some `jsdom` versions
      // (real targets — WebView2/WKWebView, both Chromium/WebKit-based —
      // support them fully); fall back to the plain `open` attribute so
      // component tests still exercise real markup and focus behavior.
      if (typeof dialog.showModal === "function") {
        dialog.showModal();
      } else {
        dialog.setAttribute("open", "");
      }
      cancelRef.current?.focus();
    } else if (!open && dialog.open) {
      if (typeof dialog.close === "function") {
        dialog.close();
      } else {
        dialog.removeAttribute("open");
      }
    }
  }, [open]);

  return (
    <dialog
      ref={dialogRef}
      className="confirm-dialog"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      onCancel={(event) => {
        // The native Escape-to-close gesture; treat it the same as Cancel.
        event.preventDefault();
        onCancel();
      }}
      onClose={onCancel}
    >
      <h2 className="confirm-dialog__title" id={titleId}>
        {title}
      </h2>
      <div className="confirm-dialog__description" id={descriptionId}>
        {description}
      </div>
      <div className="confirm-dialog__actions">
        <Button ref={cancelRef} variant="secondary" onClick={onCancel}>
          {cancelLabel}
        </Button>
        <Button variant={danger ? "danger" : "primary"} onClick={onConfirm}>
          {confirmLabel}
        </Button>
      </div>
    </dialog>
  );
}
