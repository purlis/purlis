// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { listLinksOf, listsLinks } from "./paneLinksOpen";

/**
 * **Where the window reaches a pane's *Links in this pane…*** (#6332, #6488): a pane says how
 * to open its list while it draws a chat, and the window's row opens it.
 */
describe("a pane's Links in this pane… as the window reaches it", () => {
  it("opens the list of the pane drawing the chat, and says it did", () => {
    const opened: string[] = [];
    const done = listsLinks("/p", 4, () => opened.push("pane A"));

    expect(listLinksOf("/p", 4)).toBe(true);
    expect(opened).toEqual(["pane A"]);
    done();
  });

  it("says it opened none where no pane draws the chat", () => {
    expect(listLinksOf("/p", 5)).toBe(false);
    const done = listsLinks("/p", 5, () => {});
    done();
    expect(listLinksOf("/p", 5)).toBe(false);
  });

  it("opens the newest pane's list where two panes draw one chat, and the other's once it goes", () => {
    const opened: string[] = [];
    const first = listsLinks("/p", 6, () => opened.push("pane A"));
    const second = listsLinks("/p", 6, () => opened.push("pane B"));

    expect(listLinksOf("/p", 6)).toBe(true);
    // The second pane goes: the first still draws the chat, and its list still opens.
    second();
    expect(listLinksOf("/p", 6)).toBe(true);
    first();
    expect(listLinksOf("/p", 6)).toBe(false);
    expect(opened).toEqual(["pane B", "pane A"]);
  });

  it("keeps the newest pane's list where an older pane goes first", () => {
    const opened: string[] = [];
    const first = listsLinks("/p", 7, () => opened.push("pane A"));
    const second = listsLinks("/p", 7, () => opened.push("pane B"));

    first();
    expect(listLinksOf("/p", 7)).toBe(true);
    second();
    expect(listLinksOf("/p", 7)).toBe(false);
    expect(opened).toEqual(["pane B"]);
  });

  it("tells chats of two projects apart", () => {
    const opened: string[] = [];
    const done = listsLinks("/p", 8, () => opened.push("here"));

    expect(listLinksOf("/q", 8)).toBe(false);
    expect(opened).toEqual([]);
    done();
  });
});
