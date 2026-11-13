// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A card beside its row stands clear of a native page** (#6226): a page's webview is drawn
 * above the window, so a card across it would be drawn under it.
 */
import { afterEach, describe, expect, it } from "vitest";
import { NATIVE_PAGE, keptOffPages } from "./offPage";

function box(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON: () => ({}),
  } as DOMRect;
}

/** `one`, answering `at` for its box. */
function at<T extends Element>(one: T, where: DOMRect): T {
  one.getBoundingClientRect = () => where;
  return one;
}

/**
 * The window: a sidebar 0–240 across with a row in it, and the centre beside it, which holds
 * a page's frame when `page` is drawn.
 */
function window_({ page = true }: { page?: boolean } = {}) {
  const whole = at(document.createElement("div"), box(0, 0, 1000, 700));
  const sidebar = at(document.createElement("nav"), box(0, 0, 240, 700));
  const list = at(document.createElement("ul"), box(0, 40, 240, 660));
  const row = at(document.createElement("li"), box(0, 100, 240, 28));
  const centre = at(document.createElement("main"), box(248, 0, 752, 700));
  list.append(row);
  sidebar.append(list);
  whole.append(sidebar, centre);
  if (page) {
    const frame = at(document.createElement("div"), box(250, 40, 748, 658));
    frame.setAttribute(NATIVE_PAGE, "");
    centre.append(frame);
  }
  document.body.append(whole);
  return { sidebar, row, centre };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("where a card beside its row stands while a page is open (#6226)", () => {
  it("is kept in the row's own region, the widest part around it clear of the page", () => {
    const { sidebar, row } = window_();
    expect(keptOffPages(row, "right")).toBe(sidebar);
  });

  it("stands where it asked when no page is open", () => {
    const { row } = window_({ page: false });
    expect(keptOffPages(row, "right")).toBeNull();
  });

  it("stands where it asked when the page is on its other side", () => {
    const { row } = window_();
    expect(keptOffPages(row, "left")).toBeNull();
  });

  it("stands where it asked when the page is hidden, as a tab behind another is", () => {
    const { centre, row } = window_();
    centre.hidden = true;
    expect(keptOffPages(row, "right")).toBeNull();
  });

  it("leaves a card under or over its trigger alone: only one beside it is moved", () => {
    const { row } = window_();
    expect(keptOffPages(row, "bottom")).toBeNull();
  });

  it("is not moved when nothing around its row is clear of the page", () => {
    const { row } = window_();
    at(row, box(240, 100, 40, 28));
    expect(keptOffPages(row, "right")).toBeNull();
  });
});
