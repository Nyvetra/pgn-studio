// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Shared "why is this option disabled" text for capability-gated controls
 * (task spec: "Only offer options the backend reports as supported via
 * get_engine_capabilities; disable the rest with an explanation").
 *
 * `capabilities === null` (not yet fetched) is treated as "unknown, so
 * disabled" rather than optimistically enabled — the same conservative
 * posture `filesystem::validate` itself uses for `unicode_paths` ("the safe
 * default... can only make validate_job more conservative, never silently
 * accept [something] the running binary might not actually support").
 */
export function capabilityDisabledReason(
  capabilitiesLoaded: boolean,
  supported: boolean,
): string | undefined {
  if (!capabilitiesLoaded) return "Checking what this engine build supports…";
  if (!supported) return "Not supported by this engine build.";
  return undefined;
}
