// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Type augmentation for `jest-axe`'s `toHaveNoViolations()` matcher, which
 * `src/test/setup.ts` registers on Vitest's `expect` at runtime via
 * `expect.extend(toHaveNoViolations)`. `jest-axe`'s own `.d.ts` only
 * augments Jest's global `expect`/`@jest/expect` module — this project
 * uses Vitest, whose own `Assertion`/`AsymmetricMatchersContaining`
 * interfaces need the same augmentation, following Vitest's documented
 * pattern for adopting a Jest-ecosystem matcher package
 * (https://vitest.dev/guide/extending-matchers).
 */
import type { AxeResults } from "axe-core";

interface CustomMatchers<R = unknown> {
  toHaveNoViolations(): R;
}

declare module "vitest" {
  // Both interfaces below have no members of their own by design — this is
  // TypeScript's required *declaration-merging* shape for augmenting an
  // imported interface (a `type` alias cannot be merged this way), and is
  // exactly the pattern Vitest's own docs prescribe for this. Disabling the
  // "empty interface" rule here rather than broadening it project-wide.
  // eslint-disable-next-line @typescript-eslint/no-empty-object-type
  interface Assertion<T = unknown> extends CustomMatchers<T> {}
  // eslint-disable-next-line @typescript-eslint/no-empty-object-type
  interface AsymmetricMatchersContaining extends CustomMatchers {}
}

// Referenced only for documentation purposes above; keeps this file a
// module (rather than a global script) without an unused-import lint
// warning.
export type { AxeResults };
