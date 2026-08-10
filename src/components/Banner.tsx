// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Status banner for inline messages (errors, warnings, advisories, success
 * confirmations). Never relies on color alone (architecture.md §13.8): each
 * variant renders its own text label ("Error", "Warning", ...) and a
 * distinct inline glyph, so the meaning survives grayscale/high-contrast
 * rendering and is available to screen readers even before the visible
 * label text is reached.
 */
import type { PropsWithChildren, ReactNode } from "react";
import "./Banner.css";

export type BannerTone = "info" | "warning" | "danger" | "success";

const TONE_LABEL: Record<BannerTone, string> = {
  info: "Note",
  warning: "Warning",
  danger: "Error",
  success: "Success",
};

/** `aria-hidden` decorative glyphs — text-shaped, not relying on any
 * bundled icon font (the CSP forbids remote fonts/assets). */
const TONE_GLYPH: Record<BannerTone, string> = {
  info: "ℹ", // ℹ
  warning: "⚠", // ⚠
  danger: "✕", // ✕
  success: "✓", // ✓
};

export interface BannerProps {
  tone: BannerTone;
  title?: string;
  className?: string;
  /** `"alert"` interrupts screen readers immediately; use for validation
   * failures. `"status"` is polite; use for advisories/success notes. */
  role?: "alert" | "status";
}

export function Banner({ tone, title, role = "status", className, children }: PropsWithChildren<BannerProps>) {
  return (
    <div className={["banner", `banner--${tone}`, className].filter(Boolean).join(" ")} role={role}>
      <span className="banner__glyph" aria-hidden="true">
        {TONE_GLYPH[tone]}
      </span>
      <div className="banner__body">
        <p className="banner__label">{title ?? `${TONE_LABEL[tone]}:`}</p>
        <div className="banner__content">{children as ReactNode}</div>
      </div>
    </div>
  );
}
