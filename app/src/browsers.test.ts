// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { browserView, browsersLetGo, browsersShown, BROWSER_TITLE } from "./browsers";
import { closeFocusedPane, contentsOf, noTabs, openView, splitFocusedPane } from "./tabs";

const filedIn = () => "alpha";

describe("a browser's view tab (#2012)", () => {
  it("splits like a chat: a chat opens beside the browser, in the same tab", () => {
    const opened = openView(noTabs(), browserView("B1"), BROWSER_TITLE, "alpha");
    const tab = opened.inFront ?? -1;

    const split = splitFocusedPane(opened, "row", 7, "steward 7");

    const shown = contentsOf(split, tab).map(({ content }) => content);
    expect(shown).toEqual([
      { kind: "view", view: browserView("B1"), workspace: "alpha" },
      { kind: "session", session: 7, chat: "steward 7" },
    ]);
    expect(browsersShown(split)).toEqual(new Set(["B1"]));
  });

  it("is one tab however often it is opened", () => {
    const once = openView(noTabs(), browserView("B1"), BROWSER_TITLE, "alpha");
    const twice = openView(once, browserView("B1"), BROWSER_TITLE, "alpha");
    expect(twice.order).toHaveLength(1);
  });

  it("lets its browser go when the last tab showing it closes, and not before", () => {
    const one = openView(noTabs(), browserView("B1"), BROWSER_TITLE, "alpha");
    const two = openView(one, browserView("B2"), BROWSER_TITLE, "alpha");
    const split = splitFocusedPane(two, "row", 7, "steward 7");

    const chatClosed = closeFocusedPane(split, filedIn);
    expect(browsersLetGo(split, chatClosed)).toEqual([]);

    const browserClosed = closeFocusedPane(chatClosed, filedIn);
    expect(browsersLetGo(chatClosed, browserClosed)).toEqual(["B2"]);
    expect(browsersShown(browserClosed)).toEqual(new Set(["B1"]));
  });
});
