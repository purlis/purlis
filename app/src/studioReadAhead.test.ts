// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { AT_THE_ROOT, inWorkspace, scopeKey } from "./skillStudio";
import {
  forgetStudioReadAhead,
  readStudioAhead,
  studioOpeningRead,
  studioOpeningReadDrawn,
} from "./studioReadAhead";

let reads: unknown[] = [];

beforeEach(() => {
  reads = [];
  mockIPC(
    (cmd, args) => {
      if (cmd !== "skill_studio") return null;
      reads.push((args as { workspace: unknown }).workspace);
      return { read: reads.length };
    },
    { shouldMockEvents: true },
  );
});

afterEach(() => {
  forgetStudioReadAhead();
  clearMocks();
});

const TAB = "studio-history";
const ALPHA = inWorkspace("alpha");

/** Lets a listener the read ahead asked for be registered, as it is long before a tab draws. */
const listening = () => new Promise((done) => setTimeout(done, 0));

describe("the studio's skills asked for as it is opened (#4351)", () => {
  it("hands the tab it was asked for that read, as often as it asks before drawing it", async () => {
    readStudioAhead("/plane", TAB, ALPHA);

    const first = studioOpeningRead(TAB, scopeKey(ALPHA));
    const again = studioOpeningRead(TAB, scopeKey(ALPHA));

    expect(first).toBeDefined();
    expect(again).toBe(first);
    expect(reads).toEqual(["alpha"]);
  });

  it("hands it to no one once the tab has drawn it", () => {
    readStudioAhead("/plane", TAB, ALPHA);
    const taken = studioOpeningRead(TAB, scopeKey(ALPHA));
    if (taken === undefined) throw new Error("the opening's read was not handed over");

    studioOpeningReadDrawn(taken);

    expect(studioOpeningRead(TAB, scopeKey(ALPHA))).toBeUndefined();
  });

  it("is no other tab's, nor another scope's", () => {
    readStudioAhead("/plane", TAB, ALPHA);

    expect(studioOpeningRead("another-tab", scopeKey(ALPHA))).toBeUndefined();
    expect(studioOpeningRead(TAB, scopeKey(AT_THE_ROOT))).toBeUndefined();
  });

  it("is left for a read of the tab's own once the views' stores change on disk", async () => {
    readStudioAhead("/plane", TAB, ALPHA);
    await listening();

    await emit("plane-changed", { plane: "/plane", answers: null, changes: null });

    expect(studioOpeningRead(TAB, scopeKey(ALPHA))).toBeUndefined();
  });

  it("still hands it over for a change in another project", async () => {
    readStudioAhead("/plane", TAB, ALPHA);
    await listening();

    await emit("plane-changed", { plane: "/elsewhere", answers: null, changes: null });

    expect(studioOpeningRead(TAB, scopeKey(ALPHA))).toBeDefined();
  });

  it("hands over only the last opening's read", () => {
    readStudioAhead("/plane", TAB, ALPHA);
    readStudioAhead("/plane", TAB, AT_THE_ROOT);

    expect(studioOpeningRead(TAB, scopeKey(ALPHA))).toBeUndefined();
    expect(studioOpeningRead(TAB, scopeKey(AT_THE_ROOT))).toBeDefined();
  });
});
