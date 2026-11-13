// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { coveredByAnOverlay, drawnAndMore, movedBy, pinOrderAfterDrop } from "./artifacts";

describe("an artifact group's order (#5953)", () => {
  const ids = ["a", "b", "c", "d"];

  it("moves a pin one place up or down, and says nothing at either end", () => {
    expect(movedBy(ids, "b", -1)).toEqual(["b", "a", "c", "d"]);
    expect(movedBy(ids, "b", 1)).toEqual(["a", "c", "b", "d"]);
    expect(movedBy(ids, "a", -1)).toBeUndefined();
    expect(movedBy(ids, "d", 1)).toBeUndefined();
    expect(movedBy(ids, "zz", 1)).toBeUndefined();
  });

  it("puts a dropped pin where it landed, and says nothing for a drop on itself", () => {
    expect(pinOrderAfterDrop(ids, "a", "c")).toEqual(["b", "c", "a", "d"]);
    expect(pinOrderAfterDrop(ids, "d", "a")).toEqual(["d", "a", "b", "c"]);
    expect(pinOrderAfterDrop(ids, "a", "a")).toBeUndefined();
    expect(pinOrderAfterDrop(ids, "a", "zz")).toBeUndefined();
  });

  it("draws at most eight and keeps every one past them for show-more, none dropped", () => {
    const eleven = Array.from({ length: 11 }, (_, n) => `p${n}`);
    const { drawn, more } = drawnAndMore(eleven);
    expect(drawn).toEqual(eleven.slice(0, 8));
    expect(more).toEqual(eleven.slice(8));
    expect([...drawn, ...more]).toEqual(eleven);
    expect(drawnAndMore(ids)).toEqual({ drawn: ids, more: [] });
  });
});

/** A box at `left`, `top`, `width` by `height`, as `getBoundingClientRect` answers. */
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

/** An element of `markup`, drawn in `at`, put in the document. */
function drawn(markup: string, at: DOMRect): Element {
  const holder = document.createElement("div");
  holder.innerHTML = markup;
  const one = holder.firstElementChild as Element;
  one.getBoundingClientRect = () => at;
  document.body.append(one);
  return one;
}

describe("what hides an artifact's page (#6226)", () => {
  // The page's frame, in the centre, right of a sidebar 0–240 across.
  const frame = box(260, 40, 700, 600);
  afterEach(() => {
    document.body.replaceChildren();
  });

  it("hides it under a modal dialog, even one whose box is clear of the frame", () => {
    drawn('<div data-scope="dialog" data-part="scrim"></div>', box(0, 0, 1000, 700));
    expect(coveredByAnOverlay(frame)).toBe(true);
    document.body.replaceChildren();
    drawn('<div role="dialog" aria-modal="true"></div>', box(10, 10, 200, 100));
    expect(coveredByAnOverlay(frame)).toBe(true);
  });

  it("hides it under an alert dialog, wherever it stands", () => {
    drawn('<div role="alertdialog"></div>', box(10, 10, 200, 100));
    expect(coveredByAnOverlay(frame)).toBe(true);
  });

  it("hides it under a menu opened from its own bar, which hangs over the page", () => {
    drawn(
      '<div data-scope="popover" data-part="root"><div role="menu"></div></div>',
      box(800, 40, 180, 200),
    );
    expect(coveredByAnOverlay(frame)).toBe(true);
  });

  it("leaves it under a chat row's card kept over the sidebar, clear of the frame", () => {
    drawn(
      '<div data-scope="popover" data-part="root"><div role="tooltip"></div></div>',
      box(8, 120, 224, 160),
    );
    expect(coveredByAnOverlay(frame)).toBe(false);
  });

  it("leaves it under the sidebar's menu, which stands in the sidebar", () => {
    drawn(
      '<div data-scope="popover" data-part="root"><div role="menu"></div></div>',
      box(60, 120, 180, 200),
    );
    expect(coveredByAnOverlay(frame)).toBe(false);
  });

  it("leaves it under a tooltip, which comes and goes under the pointer", () => {
    drawn(
      '<div data-scope="tooltip" data-part="root" role="tooltip"></div>',
      box(250, 50, 120, 30),
    );
    expect(coveredByAnOverlay(frame)).toBe(false);
  });

  it("hides it under a card that could not be kept clear, which would otherwise be drawn under the page", () => {
    drawn(
      '<div data-scope="popover" data-part="root"><div role="tooltip"></div></div>',
      box(246, 120, 320, 160),
    );
    expect(coveredByAnOverlay(frame)).toBe(true);
  });

  it("leaves it under an overlay not drawn yet, with no box", () => {
    drawn('<div role="alertdialog"></div>', box(0, 0, 0, 0));
    expect(coveredByAnOverlay(frame)).toBe(false);
  });
});
