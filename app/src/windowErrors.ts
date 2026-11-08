// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands, type WindowErrorSeen } from "./bindings";

/**
 * **An error the window did not catch becomes a Crash report, the panic's way** (#5690, spec
 * #5683).
 *
 * Every error thrown and not caught, and every promise rejected with nobody to handle it, is
 * handed to the core (`report_window_error`) as the window saw it. Whether it may go, what of it
 * goes and where are the core's to say: without the person's yes to Crash reports nothing
 * leaves, and what does is scrubbed as a panic is and listed in Settings › You › Reports.
 *
 * Each error is handed in once per watch: the core counts a repeat across launches, and a
 * window that throws on every frame should not ask it sixty times a second. A failed hand-in is
 * let go, never reported in turn.
 */

/** Errors of the browser's own, never of purlis's code. */
const NOT_OURS = [/^ResizeObserver loop/];

/** Hands each error `target` does not catch to the core, until the answer is called. */
export function watchWindowErrors(target: Window): () => void {
  const handed = new Set<string>();
  const handIn = (error: WindowErrorSeen) => {
    if (NOT_OURS.some((pattern) => pattern.test(error.message))) return;
    const key = JSON.stringify(error);
    if (handed.has(key)) return;
    handed.add(key);
    void commands.reportWindowError(error).catch(() => undefined);
  };
  const thrown = (event: ErrorEvent) => {
    const error: unknown = event.error;
    handIn({
      name: error instanceof Error ? error.name : "Error",
      message: error instanceof Error ? error.message : event.message,
      source: event.filename || null,
      line: event.lineno || null,
      column: event.colno || null,
      rejection: false,
    });
  };
  const rejected = (event: PromiseRejectionEvent) => {
    const reason: unknown = event.reason;
    handIn({
      name: reason instanceof Error ? reason.name : "Rejection",
      message: reason instanceof Error ? reason.message : String(reason),
      source: null,
      line: null,
      column: null,
      rejection: true,
    });
  };
  target.addEventListener("error", thrown);
  target.addEventListener("unhandledrejection", rejected);
  return () => {
    target.removeEventListener("error", thrown);
    target.removeEventListener("unhandledrejection", rejected);
  };
}
