// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Semantic `<button>` wrapper (architecture.md §13.8: "semantic buttons and
 * labels"). Always a real `<button type="button">` by default (never a
 * `div`/`span` with a click handler) so it is keyboard-activatable (Enter
 * and Space) and announced correctly by assistive technology for free.
 */
import { forwardRef, type ButtonHTMLAttributes } from "react";
import "./Button.css";

export type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  busy?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", busy = false, className, disabled, children, type = "button", ...rest },
  ref,
) {
  const classes = ["btn", `btn--${variant}`, className].filter(Boolean).join(" ");
  return (
    <button
      ref={ref}
      type={type}
      className={classes}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      {...rest}
    >
      {children}
    </button>
  );
});
