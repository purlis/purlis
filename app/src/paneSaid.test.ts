// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { paneSaidOf } from "./paneSaid";
import { focusPane, noTabs, openTab, panesOf, press, splitFocusedPane, type Tabs } from "./tabs";

/**
 * **What a reader is told when a press lands in another pane** (#4767, ruled on #4699): one
 * polite line where the keyboard goes to a pane it was not asked into, and nothing where it
 * follows the press into the pane it was asked into.
 */

const pressing = { filedIn: () => "ops", frontOf: () => undefined };
const nameOf = (session: number) => `steward ${session}`;

/** 1 on the left, 2 on the right; the left pane has the focus. */
function twoPanes(): Tabs {
  const split = splitFocusedPane(openTab(noTabs(), 1), "row", 2);
  const left = panesOf(split, split.inFront as number).find((one) => one.session === 1);
  return focusPane(split, left?.pane as number);
}

describe("the line about a pane the keyboard did not ask for", () => {
  it("says a chat pressed for is already in another pane, and that the keyboard went there", () => {
    const before = twoPanes();
    const next = press(before, { session: 2, chat: "2" }, pressing).tabs;

    expect(paneSaidOf(before, next, 2, nameOf)).toBe(
      "steward 2 is already in the right pane; moved there.",
    );
  });

  it("says nothing where the press is shown in the focused pane, which the keyboard follows", () => {
    const before = twoPanes();
    const next = press(before, { session: 7, chat: "7" }, pressing).tabs;

    expect(paneSaidOf(before, next, 7, nameOf)).toBeUndefined();
  });

  it("says nothing where the chat was in the focused pane already", () => {
    const before = twoPanes();
    const next = press(before, { session: 1, chat: "1" }, pressing).tabs;

    expect(paneSaidOf(before, next, 1, nameOf)).toBeUndefined();
  });

  it("says nothing where another tab came forward, whose strip says it", () => {
    const before = openTab(twoPanes(), 9);
    const next = press(before, { session: 2, chat: "2" }, pressing).tabs;

    expect(paneSaidOf(before, next, 2, nameOf)).toBeUndefined();
  });
});
