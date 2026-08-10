// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The shape every generated command wrapper resolves to — see the
 * `typedError` runtime helper at the bottom of `generated-types.ts`. Every
 * IPC call site must branch on `.status`; only a genuine JS runtime/
 * serialization failure (never a `PublicError`) rejects the promise.
 */
import type { PublicError } from "./client";

export type IpcResult<T> = { status: "ok"; data: T } | { status: "error"; error: PublicError };

export class IpcError extends Error {
  public readonly publicError: PublicError;
  constructor(publicError: PublicError) {
    super(publicError.message);
    this.name = "IpcError";
    this.publicError = publicError;
  }
}

/** Convenience for call sites that would rather `try`/`catch` than branch on
 * `.status` inline (e.g. one-shot effects that already have a catch block). */
export function unwrap<T>(result: IpcResult<T>): T {
  if (result.status === "error") {
    throw new IpcError(result.error);
  }
  return result.data;
}
