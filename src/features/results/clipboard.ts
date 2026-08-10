// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Copy Path" (architecture.md §13.7) via the standard Web Clipboard API.
 * No dedicated Tauri clipboard plugin is installed in this project
 * (`@tauri-apps/plugin-clipboard-manager` is not a dependency), so this
 * uses `navigator.clipboard` directly, with a `document.execCommand`
 * fallback for any context where the async Clipboard API is unavailable.
 * Not independently verified against a live packaged Tauri window in this
 * environment — see the Phase 2b report.
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // fall through to the legacy fallback below
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  try {
    textarea.focus();
    textarea.select();
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    // Must run even if execCommand throws, or the hidden textarea leaks.
    document.body.removeChild(textarea);
  }
}
