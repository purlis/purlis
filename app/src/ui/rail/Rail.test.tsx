// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DEFAULT_THEME } from "../../theme/theme";
import { monogramOf, Rail, RailAction, RailProject, RailSlot } from "./Rail";
import { projectColour, projectTint } from "./projectColour";
import {
  NO_PEEK,
  peekAfter,
  PEEK_AT,
  PEEK_DWELL,
  PEEK_UNTIL,
  type PointerAt,
} from "../../ProjectRail";
import { afterTheSlide, forgetTheSlide, slideEnded, slideStarted } from "../../railSlide";
import { loadRailMode } from "../../railMode";

afterEach(cleanup);

describe("the rail family (#2915)", () => {
  it("writes a project's monogram from the first letters of its name's parts (R77)", () => {
    expect(monogramOf("easydmarc-umbrella")).toBe("EU");
    expect(monogramOf("elastic-press-docs")).toBe("EP");
    expect(monogramOf("charter")).toBe("C");
    expect(monogramOf("my_notes")).toBe("MN");
  });

  it("draws a project as its family's button: named, ringed while current, its badge counted", () => {
    render(
      <Rail name="Projects" tabs={["a", "b"]}>
        <RailProject id="a" role="tab" name="charter" current waiting={3} />
        <RailProject id="b" role="tab" name="volaticloud" current={false} />
        <RailAction aria-label="Open a project…" icon={<span>+</span>} />
      </Rail>,
    );
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((one) => one.getAttribute("aria-label"))).toEqual(["charter", "volaticloud"]);
    expect(tabs[0]).toHaveAttribute("data-scope", "rail");
    expect(tabs[0]).toHaveAttribute("data-part", "button");
    expect(tabs[0]).toHaveAttribute("data-variant", "project");
    expect(tabs[0]).toHaveAttribute("data-selected");
    expect(tabs[1]).not.toHaveAttribute("data-selected");
    expect(tabs[0].querySelector('[data-part="badge"]')).toHaveAttribute("data-count", "3");
    expect(tabs[1].querySelector('[data-part="badge"]')).toBeNull();
    expect(screen.getByRole("tablist", { name: "Projects" })).toHaveAttribute(
      "aria-orientation",
      "vertical",
    );
    expect(screen.getByRole("button", { name: "Open a project…" })).toHaveAttribute(
      "data-variant",
      "action",
    );
  });

  it("dots a project with unsaved work, and says it (charter-app#302)", () => {
    render(
      <Rail name="Projects" tabs={["a"]}>
        <RailProject
          id="a"
          role="tab"
          name="charter"
          current={false}
          waiting={2}
          unsaved={{ blocked: true, says: "The save is blocked" }}
        />
      </Rail>,
    );
    const tab = screen.getByRole("tab", { name: "charter" });
    expect(tab.querySelector('[data-part="unsaved"]')).toHaveAttribute("data-blocked");
    expect(tab).toHaveAccessibleDescription("2 waiting, saving is blocked");
  });

  it("keeps a project's name first in its tooltip, its unsaved work the second line (S1)", async () => {
    render(
      <Rail name="Projects" tabs={["a"]}>
        <RailProject
          id="a"
          role="tab"
          name="charter"
          current={false}
          unsaved={{ blocked: false, says: "3 files to save" }}
        />
      </Rail>,
    );
    screen.getByRole("tab", { name: "charter" }).focus();
    fireEvent.pointerEnter(screen.getByRole("tab", { name: "charter" }));
    const tooltip = await screen.findByRole("tooltip", {}, { timeout: 2000 });
    expect(tooltip.textContent).toBe("charter3 files to save");
  });

  it("takes a hidden rail out of reach (R82)", () => {
    const { rerender } = render(
      <RailSlot hidden={false}>
        <RailAction aria-label="Hide the projects sidebar" icon={<span />} />
      </RailSlot>,
    );
    expect(screen.getByRole("button", { name: "Hide the projects sidebar" })).toBeInTheDocument();
    rerender(
      <RailSlot hidden>
        <RailAction aria-label="Hide the projects sidebar" icon={<span />} />
      </RailSlot>,
    );
    expect(screen.queryByRole("button", { name: "Hide the projects sidebar" })).toBeNull();
  });

  it("gives each project a colour of the theme's own palette, the same one every time", () => {
    const one = projectColour(DEFAULT_THEME, "charter");
    expect(DEFAULT_THEME.tint.hues.map((hue) => hue.name)).toContain(one);
    expect(projectColour(DEFAULT_THEME, "charter")).toBe(one);
    expect(Object.keys(projectTint(DEFAULT_THEME, "charter")).sort()).toEqual([
      "--accent-base",
      "--accent-surface",
    ]);
  });
});

describe("the peek (R86, R87, B2)", () => {
  const at = (x: number, time: number, more: Partial<PointerAt> = {}): PointerAt => ({
    x,
    under: true,
    time,
    pressed: false,
    fresh: false,
    ...more,
  });
  /** The peek after each position in turn. */
  const after = (...moves: PointerAt[]) => moves.reduce(peekAfter, NO_PEEK);

  it("comes in once the pointer has rested at the left edge below the title bar", () => {
    expect(after(at(0, 0)).peek).toBe("closed");
    expect(after(at(0, 0), at(2, PEEK_DWELL - 1)).peek).toBe("closed");
    expect(after(at(0, 0), at(2, PEEK_DWELL)).peek).toBe("open");
    expect(after(at(PEEK_AT, 0), at(PEEK_AT, PEEK_DWELL)).peek).toBe("closed");
    expect(after(at(0, 0, { under: false }), at(0, PEEK_DWELL, { under: false })).peek).toBe(
      "closed",
    );
  });

  it("never comes in for a pointer thrown through the edge to the activity bar there", () => {
    // At the edge, and away to the bar a moment later: the click is the bar's.
    expect(after(at(0, 0), at(20, 40), at(20, PEEK_DWELL + 40)).peek).toBe("closed");
    // Back at the edge starts the wait again.
    expect(after(at(0, 0), at(20, 40), at(1, 100), at(1, PEEK_DWELL + 40)).peek).toBe("closed");
  });

  it("never comes in while a button is held, or on the move that brings the pointer in", () => {
    expect(after(at(0, 0, { pressed: true }), at(0, PEEK_DWELL, { pressed: true })).peek).toBe(
      "closed",
    );
    expect(after(at(0, 0, { fresh: true }), at(0, PEEK_DWELL - 1)).peek).toBe("closed");
  });

  it("stays while the pointer is near, goes past 70px, and comes back while going", () => {
    const open = after(at(0, 0), at(0, PEEK_DWELL));
    expect(peekAfter(open, at(PEEK_UNTIL, 1000)).peek).toBe("open");
    const leaving = peekAfter(open, at(PEEK_UNTIL + 1, 1000));
    expect(leaving.peek).toBe("leaving");
    expect(peekAfter(peekAfter(leaving, at(2, 1100)), at(2, 1100 + PEEK_DWELL)).peek).toBe("open");
  });
});

describe("the rail's slide (B1)", () => {
  it("holds what fits itself to the work until the slide is over, and runs it once", () => {
    forgetTheSlide();
    let fits = 0;
    const refit = () => {
      fits += 1;
    };
    afterTheSlide(refit);
    expect(fits).toBe(1);
    slideStarted();
    // A resize observer's sixteen frames of a 260 ms slide.
    for (let frame = 0; frame < 16; frame++) afterTheSlide(refit);
    expect(fits).toBe(1);
    slideEnded();
    expect(fits).toBe(2);
    slideEnded();
    expect(fits).toBe(2);
  });
});

describe("the rail's mode (R83)", () => {
  it("is shown unless the layout file says the rail hides itself", () => {
    expect(loadRailMode({ version: 2, regions: [] })).toEqual({ value: "shown", said: [] });
    expect(loadRailMode({ version: 2, regions: [], rail: "auto" })).toEqual({
      value: "auto",
      said: [],
    });
  });

  it("says what it could not read, and shows the rail", () => {
    const { value, said } = loadRailMode({ rail: 3 });
    expect(value).toBe("shown");
    expect(said).toEqual(['"rail" 3 is not "shown" or "auto", so the rail is shown']);
  });
});
