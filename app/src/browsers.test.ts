// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  addressChip,
  deviceOf,
  browserView,
  browsersLetGo,
  browsersShown,
  BROWSER_TITLE,
  chatBrowserTitle,
  openBrowserBeside,
} from "./browsers";
import {
  closeFocusedPane,
  closeTab,
  contentsOf,
  noTabs,
  openTab,
  openView,
  splitFocusedPane,
} from "./tabs";

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

describe("a chat's browser (#2024)", () => {
  it("is named after its chat and the name the chat knows it by", () => {
    expect(chatBrowserTitle("steward 3", "b1")).toBe("Browser b1 of steward 3");
    // A pinned browser whose chat's name was not kept is named by its own name alone.
    expect(chatBrowserTitle("", "b1")).toBe("Browser b1");
  });

  it("is never let go of when a tab showing it closes: it ends with its chat", () => {
    const opened = openView(noTabs(), browserView("B7"), BROWSER_TITLE, "alpha");
    const closed = closeTab(opened, opened.inFront ?? -1, filedIn, () => false);

    expect(browsersLetGo(opened, closed, new Set(["B7"]))).toEqual([]);
    expect(browsersLetGo(opened, closed)).toEqual(["B7"]);
  });

  it("opens beside its chat's pane, in the chat's tab, and leaves the keyboard where it was", () => {
    const chat = openTab(noTabs(), 3, "3", "steward");
    const other = openTab(chat, 9, "9", "steward");
    const tab = chat.inFront ?? -1;

    const beside = openBrowserBeside(other, 3, "B7", "alpha");

    expect(contentsOf(beside, tab).map(({ content }) => content)).toEqual([
      { kind: "session", session: 3, chat: "3" },
      { kind: "view", view: browserView("B7"), workspace: "alpha" },
    ]);
    expect(beside.inFront).toBe(other.inFront);
    expect(beside.byId[tab].focused).toBe(chat.byId[tab].focused);
  });

  it("is not opened a second time beside its chat once a tab shows it", () => {
    const chat = openTab(noTabs(), 3, "3", "steward");
    const once = openBrowserBeside(chat, 3, "B7", "alpha");

    expect(openBrowserBeside(once, 3, "B7", "alpha")).toBe(once);
  });

  it("of a chat with no pane opens in nothing: its row still opens it", () => {
    const tabs = openTab(noTabs(), 3, "3", "steward");
    expect(openBrowserBeside(tabs, 4, "B7", "alpha")).toBe(tabs);
  });
});

describe("a browser's address chip (#2024)", () => {
  it("names the page's host and the device it runs on", () => {
    expect(addressChip("http://localhost:5173/login", "this Mac")).toEqual({
      said: "localhost:5173 on this Mac",
      whole: "http://localhost:5173/login",
    });
    expect(addressChip("https://docs.example.com/a?b=c", "this computer")).toEqual({
      said: "docs.example.com on this computer",
      whole: "https://docs.example.com/a?b=c",
    });
  });

  it("names the device alone where there is no page yet, or the page is blank", () => {
    expect(addressChip(null, "this Mac")).toEqual({ said: "On this Mac", whole: null });
    expect(addressChip("about:blank", "this Mac")).toEqual({
      said: "A blank page on this Mac",
      whole: null,
    });
  });
});

describe("the device a browser's chip names (#2050)", () => {
  it("is this machine, or the pool whose runner the browser runs on", () => {
    expect(deviceOf(null, true)).toBe("this Mac");
    expect(deviceOf(null, false)).toBe("this computer");
    expect(deviceOf("builds", true)).toBe("pool builds");
    expect(addressChip("http://localhost:5173/", deviceOf("builds", true)).said).toBe(
      "localhost:5173 on pool builds",
    );
  });
});

describe("a browser's chip before its device is known (#2050)", () => {
  it("names its page alone, never a device it may not be on", () => {
    expect(addressChip("http://localhost:5173/", null)).toEqual({
      said: "localhost:5173",
      whole: "http://localhost:5173/",
    });
    expect(addressChip(null, null).said).toBe("");
    expect(addressChip("about:blank", null).said).toBe("A blank page");
  });
});
