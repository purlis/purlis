// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { forgetRailMode, RAIL_MODE, railDrawn, shownAnywayAfter, useRailMode } from "./railMode";
import { railCrowds, ROOM_HELD, RAIL_WIDTH, useSidesOfTheFront } from "./railRoom";
import { forgetTextSizes, setTextSize } from "./textSize";

/** The window `width` px wide, as a resize says it. */
function windowIs(width: number) {
  act(() => {
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: width,
    });
    window.dispatchEvent(new Event("resize"));
  });
}

/** The rail as the window draws it, and the person's two presses: what the title bar's show and
 *  the rail's own hide do (`App.tsx`, `ProjectRail.tsx`). */
function Window({ sides }: { sides: number }) {
  useSidesOfTheFront(sides);
  const [drawn, chose] = useRailMode();
  return (
    <>
      <output aria-label="rail">{drawn}</output>
      <button onClick={() => chose("shown")}>Show</button>
      <button onClick={() => chose("auto")}>Hide</button>
    </>
  );
}
const drawn = () => screen.getByRole("status", { name: "rail" }).textContent;
const press = (name: "Show" | "Hide") =>
  act(() => void screen.getByRole("button", { name }).click());

describe("the room the project rail leaves the work (#3077)", () => {
  it("never leaves the work less than the narrowest window, or the default one with both sidebars", () => {
    // ADR 0054's narrowest window, with any sidebars: the rail's 52px would come out of it.
    for (const sides of [0, 1, 2]) expect(railCrowds(ROOM_HELD.narrowest, sides)).toBe(true);
    // The narrowest window and the rail beside it: room for both, with a sidebar or none.
    expect(railCrowds(ROOM_HELD.narrowest + RAIL_WIDTH, 1)).toBe(false);
    expect(railCrowds(ROOM_HELD.narrowest + RAIL_WIDTH - 1, 0)).toBe(true);
    // The default window with both sidebars open is what the tables are held to (DS-9).
    expect(railCrowds(ROOM_HELD.bothSides, 2)).toBe(true);
    expect(railCrowds(ROOM_HELD.bothSides, 1)).toBe(false);
    expect(railCrowds(ROOM_HELD.bothSides + RAIL_WIDTH, 2)).toBe(false);
  });

  it("counts the window at the default text size: a bigger text crowds it as a narrower window would", () => {
    expect(railCrowds(1200, 1, 14)).toBe(false);
    expect(railCrowds(1200, 1, 18)).toBe(true);
    // The rail's own 52px are pixels, whatever the text.
    expect(railCrowds((ROOM_HELD.narrowest * 16) / 14 + RAIL_WIDTH, 1, 16)).toBe(false);
  });

  it("is drawn over the person's mode, which it never overrides", () => {
    expect(railDrawn("shown", false, false)).toBe("shown");
    expect(railDrawn("shown", true, false)).toBe("auto");
    expect(railDrawn("shown", true, true)).toBe("shown");
    // Hidden by the person, it stays hidden in a window with all the room it wants.
    expect(railDrawn("auto", false, true)).toBe("auto");
    expect(shownAnywayAfter(false, "shown", true)).toBe(true);
    expect(shownAnywayAfter(false, "shown", false)).toBe(false);
    expect(shownAnywayAfter(true, "shown", false)).toBe(true);
    expect(shownAnywayAfter(true, "auto", true)).toBe(false);
  });
});

describe("the project rail in a narrow window (#3077)", () => {
  const written = vi.fn();
  let stop = () => {};
  beforeEach(() => {
    forgetRailMode();
    forgetTextSizes();
    written.mockReset();
    stop = RAIL_MODE.on(written);
  });
  afterEach(() => {
    stop();
    cleanup();
    windowIs(1440);
    forgetRailMode();
    forgetTextSizes();
  });

  it("hides itself where it would crowd the work and comes back as the window widens, writing nothing", () => {
    windowIs(1440);
    render(<Window sides={2} />);
    expect(drawn()).toBe("shown");
    windowIs(1280);
    expect(drawn()).toBe("auto");
    windowIs(1024);
    expect(drawn()).toBe("auto");
    windowIs(1440);
    expect(drawn()).toBe("shown");
    // The window's hide is never the person's: the layout file is not told.
    expect(written).not.toHaveBeenCalled();
    expect(RAIL_MODE.value()).toBe("shown");
    expect(RAIL_MODE.written()).toBeUndefined();
  });

  it("comes back when a sidebar is put away, and goes again as it comes out", () => {
    windowIs(1280);
    const { rerender } = render(<Window sides={2} />);
    expect(drawn()).toBe("auto");
    rerender(<Window sides={1} />);
    expect(drawn()).toBe("shown");
    rerender(<Window sides={2} />);
    expect(drawn()).toBe("auto");
    expect(written).not.toHaveBeenCalled();
  });

  it("hides itself as the window's text grows", () => {
    windowIs(1200);
    render(<Window sides={1} />);
    expect(drawn()).toBe("shown");
    act(() => setTextSize("window", 18));
    expect(drawn()).toBe("auto");
    act(() => setTextSize("window", 14));
    expect(drawn()).toBe("shown");
  });

  it("stays shown in a narrow window once the person shows it there, however often it narrows", () => {
    windowIs(1024);
    render(<Window sides={2} />);
    expect(drawn()).toBe("auto");
    press("Show");
    expect(drawn()).toBe("shown");
    windowIs(1440);
    windowIs(1024);
    expect(drawn()).toBe("shown");
    // Shown is the default, which the file leaves out: nothing to write.
    expect(written).not.toHaveBeenCalled();
    // Until the person hides it: then the window's hide is back for the next narrow window.
    press("Hide");
    expect(drawn()).toBe("auto");
    expect(written).toHaveBeenLastCalledWith("auto");
    windowIs(1440);
    press("Show");
    windowIs(1024);
    expect(drawn()).toBe("auto");
  });

  it("stays hidden in a wide window once the person hides it", () => {
    windowIs(1440);
    render(<Window sides={2} />);
    press("Hide");
    expect(drawn()).toBe("auto");
    windowIs(1024);
    windowIs(1920);
    expect(drawn()).toBe("auto");
    expect(written).toHaveBeenCalledTimes(1);
    expect(written).toHaveBeenLastCalledWith("auto");
  });
});
