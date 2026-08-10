// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Shared axe-core configuration for this project's automated accessibility
 * assertions (architecture.md §13.8: "add automated a11y assertions").
 *
 * `jest-axe`/`axe-core` are devDependencies only (never bundled into the
 * shipped app — `package.json`) and run entirely offline: the full ruleset
 * ships inside the `axe-core` package itself, so a test run performs no
 * network access. Nothing else about these tests changes that; they were
 * run and verified in this environment with no network calls involved.
 *
 * Every workflow screen test below renders that screen in isolation (its
 * own `<section>`, not the full `AppShell` with its surrounding `<main>`),
 * so whole-*document* landmark rules that only make sense once per real
 * page — "is there exactly one `<main>`", "is every region inside a
 * landmark" — are disabled here. Every rule that evaluates the component's
 * own markup (color contrast, label/name association, `aria-*` validity,
 * button/link accessible names, form-field association, duplicate ids,
 * ...) stays fully active.
 */
import { axe, type JestAxeConfigureOptions } from "jest-axe";

export const COMPONENT_AXE_OPTIONS: JestAxeConfigureOptions = {
  rules: {
    "landmark-one-main": { enabled: false },
    region: { enabled: false },
    "page-has-heading-one": { enabled: false },
  },
};

export function checkA11y(container: Element) {
  return axe(container, COMPONENT_AXE_OPTIONS);
}
