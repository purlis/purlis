import { describe, expect, it } from "vitest";
import {
  DEFAULT_ARRANGEMENT,
  panelToggled,
  picked,
  shownIn,
  viewMoved,
  type Arrangement,
} from "./regions";
import { isNarrow, lastUsed, narrowed, narrowPicked } from "./narrowPanel";

/** The halves drawn, by side, as the window draws them. */
const drawnHalves = (arrangement: Arrangement) =>
  shownIn(arrangement.filter((one) => one.id.startsWith("panel-"))).map((one) => one.side);

const away = (arrangement: Arrangement, id: string) =>
  arrangement.map((one) => (one.id === id ? { ...one, collapsed: true } : one));

describe("a narrow window (P-7)", () => {
  it("is one below about 900 px wide", () => {
    expect(isNarrow(1280)).toBe(false);
    expect(isNarrow(900)).toBe(false);
    expect(isNarrow(899)).toBe(true);
    expect(isNarrow(640)).toBe(true);
  });

  it("is counted at the default window text size, so a bigger text narrows it as zoom does", () => {
    // The window is never under 1024 px (ADR 0054), and every size in it is in rem: at the
    // narrowest window, text of 16 px lays the halves out as 896 px at the default 14 would.
    expect(isNarrow(1024, 14)).toBe(false);
    expect(isNarrow(1024, 15)).toBe(false);
    expect(isNarrow(1024, 16)).toBe(true);
    expect(isNarrow(1280, 18)).toBe(false);
    expect(isNarrow(1100, 18)).toBe(true);
    expect(isNarrow(880, 12)).toBe(false);
  });

  it("draws one half of the Panel at a time, the one used last", () => {
    expect(drawnHalves(narrowed(DEFAULT_ARRANGEMENT, "left"))).toEqual(["left"]);
    expect(drawnHalves(narrowed(DEFAULT_ARRANGEMENT, "right"))).toEqual(["right"]);
  });

  it("draws the other half, whole, when the one used last is away", () => {
    expect(drawnHalves(narrowed(away(DEFAULT_ARRANGEMENT, "panel-left"), "left"))).toEqual([
      "right",
    ]);
  });

  it("draws nothing of the Panel when both halves are away, and leaves the sidebars alone", () => {
    const hidden = panelToggled(DEFAULT_ARRANGEMENT);
    expect(drawnHalves(narrowed(hidden, "left"))).toEqual([]);
    const sidebars = (one: Arrangement) => one.filter((it) => !it.id.startsWith("panel-"));
    expect(sidebars(narrowed(DEFAULT_ARRANGEMENT, "right"))).toEqual(sidebars(DEFAULT_ARRANGEMENT));
  });

  it("keeps what is stored: a half it does not draw is not put away", () => {
    // The arrangement handed back is what is drawn; the one kept is unchanged, so a window
    // widened again draws both halves as they were left.
    const kept = structuredClone(DEFAULT_ARRANGEMENT);
    narrowed(kept, "left");
    expect(kept).toEqual(DEFAULT_ARRANGEMENT);
  });
});

describe("the half used last", () => {
  it("is the half that was just opened, or switched to another view", () => {
    const leftAway = away(DEFAULT_ARRANGEMENT, "panel-left");
    expect(lastUsed(leftAway, DEFAULT_ARRANGEMENT, "right")).toBe("left");
    const switched = picked(DEFAULT_ARRANGEMENT, "todos");
    expect(lastUsed(DEFAULT_ARRANGEMENT, switched, "left")).toBe("right");
  });

  it("stays as it was when a change opens both, as ⌘J does, or touches no half", () => {
    const hidden = panelToggled(DEFAULT_ARRANGEMENT);
    expect(lastUsed(hidden, panelToggled(hidden), "right")).toBe("right");
    expect(lastUsed(hidden, panelToggled(hidden), "left")).toBe("left");
    const sidebar = picked(DEFAULT_ARRANGEMENT, "explorer");
    expect(lastUsed(DEFAULT_ARRANGEMENT, sidebar, "right")).toBe("right");
    // Putting a half away is not using it.
    expect(lastUsed(DEFAULT_ARRANGEMENT, away(DEFAULT_ARRANGEMENT, "panel-left"), "right")).toBe(
      "right",
    );
  });
});

describe("a Lower icon pressed in a narrow window", () => {
  it("of the half not drawn: draws that half, on the view pressed, and puts nothing away", () => {
    // Memory is the right half's open view; the left half is the one drawn.
    expect(narrowPicked(DEFAULT_ARRANGEMENT, "memory", "left")).toEqual({
      last: "right",
      forward: false,
    });
    // Another view of that half: drawn, and switched to, which the window's own pick does.
    expect(narrowPicked(DEFAULT_ARRANGEMENT, "todos", "left")).toEqual({
      last: "right",
      forward: true,
    });
  });

  it("of a half that is away: brings it back and draws it", () => {
    const rightAway = away(DEFAULT_ARRANGEMENT, "panel-right");
    expect(narrowPicked(rightAway, "memory", "left")).toEqual({ last: "right", forward: true });
  });

  it("of the half drawn: the window's own pick, so its open view puts it away", () => {
    expect(narrowPicked(DEFAULT_ARRANGEMENT, "repos", "left")).toEqual({
      last: "left",
      forward: true,
    });
  });

  it("of a sidebar's view: the window's own pick, and the half used last stays", () => {
    expect(narrowPicked(DEFAULT_ARRANGEMENT, "inbox", "left")).toEqual({
      last: "left",
      forward: true,
    });
  });

  it("of a view moved into a half (#1740): the half it is in now, not the sidebar it left", () => {
    // Explorer moved into the left half, and open there; the right half is the one drawn.
    const moved = viewMoved(DEFAULT_ARRANGEMENT, "explorer", "panel-left");
    expect(narrowPicked(moved, "explorer", "right")).toEqual({ last: "left", forward: false });
    // Changes moved up to the left sidebar is a sidebar's view now: the window's own pick.
    const up = viewMoved(DEFAULT_ARRANGEMENT, "changes", "navigation");
    expect(narrowPicked(up, "changes", "right")).toEqual({ last: "right", forward: true });
  });
});
