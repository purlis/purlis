// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it, vi } from "vitest";
import { BROWSER_TITLE, browserView } from "./browsers";
import {
  INSPECTOR_TITLE,
  askToInspect,
  inspectorView,
  isInspector,
  onInspect,
  openInspector,
} from "./inspector";
import { contentsOf, noTabs, openTab, openView } from "./tabs";

describe("a browser's DevTools, in a view tab (#2028)", () => {
  it("open beside the browser they inspect, in its tab, and take the keyboard", () => {
    const browser = openView(noTabs(), browserView("B1"), BROWSER_TITLE, "alpha");
    const tab = browser.inFront ?? -1;

    const opened = openInspector(browser, "B1", "alpha");

    expect(contentsOf(opened, tab).map(({ content }) => content)).toEqual([
      { kind: "view", view: browserView("B1"), workspace: "alpha" },
      { kind: "view", view: inspectorView("B1"), workspace: "alpha" },
    ]);
    const devtools = contentsOf(opened, tab).find(
      ({ content }) => content.kind === "view" && isInspector(content.view),
    );
    expect(opened.byId[tab].focused).toBe(devtools?.pane);
    expect(opened.inFront).toBe(tab);
  });

  it("are one pane however often the person inspects", () => {
    const browser = openView(noTabs(), browserView("B1"), BROWSER_TITLE, "alpha");
    const once = openInspector(browser, "B1", "alpha");
    const twice = openInspector(once, "B1", "alpha");

    expect(contentsOf(twice, twice.inFront ?? -1)).toHaveLength(2);
  });

  it("open in a tab of their own where no tab shows the browser", () => {
    const chat = openTab(noTabs(), 3, "3", "steward");

    const opened = openInspector(chat, "B1", "alpha");

    const tab = opened.byId[opened.inFront ?? -1];
    expect(tab.name).toBe(INSPECTOR_TITLE);
    expect(contentsOf(opened, tab.id).map(({ content }) => content)).toEqual([
      { kind: "view", view: inspectorView("B1"), workspace: "alpha" },
    ]);
  });

  it("are a view of their browser alone", () => {
    expect(isInspector(inspectorView("B1"))).toBe(true);
    expect(isInspector(browserView("B1"))).toBe(false);
    expect(isInspector({ from: null, view: "devtools", key: "" })).toBe(false);
  });

  it("are asked for by a browser view of the project the window shows them in", () => {
    const here = vi.fn();
    const there = vi.fn();
    const stop = onInspect("plane-a", here);
    const stopThere = onInspect("plane-b", there);

    askToInspect("plane-a", "B1");
    stop();
    askToInspect("plane-a", "B2");
    stopThere();

    expect(here).toHaveBeenCalledExactlyOnceWith("B1");
    expect(there).not.toHaveBeenCalled();
  });
});
