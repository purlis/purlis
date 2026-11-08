// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { commands, type WindowErrorSeen } from "./bindings";
import { watchWindowErrors } from "./windowErrors";

/**
 * **An error the window did not catch is handed to the core** (`windowErrors.ts`, #5690): what
 * of it is sent, and whether, is the core's to say, so the window hands in every one it sees once.
 */

let handed: WindowErrorSeen[];
let stop: () => void;

beforeEach(() => {
  handed = [];
  vi.spyOn(commands, "reportWindowError").mockImplementation((error) => {
    handed.push(error);
    return Promise.resolve({ status: "ok", data: null });
  });
  stop = watchWindowErrors(window);
});

afterEach(() => {
  stop();
  vi.restoreAllMocks();
});

/** Throws `error` in the window, uncaught, the way the browser reports one. */
function throwInTheWindow(error: Error, filename = "tauri://localhost/assets/index-3f2a9c.js") {
  const event = new ErrorEvent("error", {
    error,
    message: error.message,
    filename,
    lineno: 1,
    colno: 4242,
    cancelable: true,
  });
  // Seen by the test alone: Vitest fails a run on an error event nobody prevented.
  window.addEventListener("error", (seen) => seen.preventDefault(), { once: true });
  window.dispatchEvent(event);
}

/** Rejects a promise in the window that nobody handles, with `reason`. */
function rejectUnhandled(reason: unknown) {
  const event = new Event("unhandledrejection");
  Object.assign(event, { reason, promise: Promise.resolve() });
  window.dispatchEvent(event);
}

describe("an error the window did not catch", () => {
  it("thrown, is handed to the core with its name, message and where it was thrown", () => {
    throwInTheWindow(new TypeError("cannot read workspace of undefined"));

    expect(handed).toEqual([
      {
        name: "TypeError",
        message: "cannot read workspace of undefined",
        source: "tauri://localhost/assets/index-3f2a9c.js",
        line: 1,
        column: 4242,
        rejection: false,
      },
    ]);
  });

  it("from a promise nobody handled, is handed to the core as a rejection", () => {
    rejectUnhandled(new Error("no answer"));
    rejectUnhandled("a reason that is no error");

    expect(handed).toEqual([
      {
        name: "Error",
        message: "no answer",
        source: null,
        line: null,
        column: null,
        rejection: true,
      },
      {
        name: "Rejection",
        message: "a reason that is no error",
        source: null,
        line: null,
        column: null,
        rejection: true,
      },
    ]);
  });

  it("thrown again in the same place is handed to the core only once", () => {
    throwInTheWindow(new TypeError("cannot read workspace of undefined"));
    throwInTheWindow(new TypeError("cannot read workspace of undefined"));

    expect(handed).toHaveLength(1);
  });

  it("of the browser's own resize loop is no error of purlis's and is not handed in", () => {
    throwInTheWindow(new Error("ResizeObserver loop completed with undelivered notifications."));

    expect(handed).toEqual([]);
  });

  it("is not handed in once the watch stops", () => {
    stop();

    throwInTheWindow(new TypeError("after the watch"));

    expect(handed).toEqual([]);
  });
});
