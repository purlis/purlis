// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { PinToggle, SwitcherRow } from "./Switcher";
import {
  AllWorkspaces,
  ChipSlot,
  mostUrgentOf,
  NOTICE_KINDS,
  NoticeDot,
  PinnedStrip,
  ProjectCrumb,
  RootChip,
  TitleBarSlash,
  urgencyOf,
  WorkspaceChip,
} from "./TitleBarWorkspaces";

afterEach(cleanup);

/**
 * **The title bar's workspace family** (#2916): what its wrapper marks for a theme to key, and
 * what its stylesheet draws, held as written (jsdom lays nothing out; the stories measure).
 */
describe("a workspace's chip", () => {
  it("is a button named by its words, its count its description", () => {
    render(<WorkspaceChip name="smart-ide" waiting={2} />);
    // Named by what it draws (WCAG 2.5.3); the count is its description.
    const chip = screen.getByRole("button", { name: "smart-ide" });
    expect(chip).toHaveAccessibleDescription("2 waiting");
    expect(chip).toHaveAttribute("data-scope", "titlebar");
    expect(chip).toHaveAttribute("data-part", "workspace");
    expect(chip).toHaveAttribute("data-variant", "pinned");
    expect(chip.querySelector('[data-part="count"]')).toHaveTextContent("2");
  });

  it("marks the current one, for a screen reader and for a theme", () => {
    render(<WorkspaceChip name="smart-ide" current />);
    const chip = screen.getByRole("button", { name: "smart-ide" });
    expect(chip).toHaveAttribute("aria-current", "true");
    expect(chip).toHaveAttribute("data-selected");
  });

  it("draws no count where nothing waits, and says + on the dashed one", () => {
    render(<WorkspaceChip name="docs" variant="unpinned" current />);
    const chip = screen.getByRole("button", { name: /docs/ });
    expect(chip.querySelector('[data-part="count"]')).toBeNull();
    expect(chip).toHaveTextContent("docs+");
    expect(chip).toHaveAttribute("data-variant", "unpinned");
  });

  it("names a dormant pin by its words and says it is gone in its description (#3651)", () => {
    render(<WorkspaceChip name="delta" variant="dormant" notices={{ count: 1, kind: "info" }} />);
    const chip = screen.getByRole("button", { name: "delta" });
    expect(chip).toHaveAttribute("data-variant", "dormant");
    expect(chip).not.toHaveAttribute("aria-current");
    expect(chip).toHaveAccessibleDescription("gone, kept dormant, 1 notice");
  });

  it("marks the pointer on it", async () => {
    render(<WorkspaceChip name="alpha" />);
    const chip = screen.getByRole("button", { name: "alpha" });
    await userEvent.hover(chip);
    expect(chip).toHaveAttribute("data-hovered");
  });
});

describe("a chip's notice dot (#3648)", () => {
  it("is a button named for what it shows, its glyph a shape no one hears", async () => {
    let pressed = 0;
    render(<NoticeDot workspace="gamma" count={1} kind="info" onPress={() => (pressed += 1)} />);
    const dot = screen.getByRole("button", { name: "Show the notice on gamma" });
    expect(dot).toHaveAttribute("data-scope", "titlebar");
    expect(dot).toHaveAttribute("data-part", "dot");
    expect(dot).toHaveAttribute("data-kind", "info");
    // A shape, not a colour alone (WCAG 1.4.1): a disc for news, the icon family's.
    expect(dot.querySelector("svg.lucide-circle")).not.toBeNull();
    // A stop the strip's arrows walk, never a Tab stop of its own (charter-app#189).
    expect(dot).toHaveAttribute("tabindex", "-1");
    await userEvent.click(dot);
    expect(pressed).toBe(1);
  });

  it("is reached by its own attributes, beside the chip and not in it, as a theme's rule is", () => {
    // The engine writes `titlebar.dot.*` around the chip (`TITLEBAR.around`, #3648's review):
    // `:where([data-scope="titlebar"][data-part="dot"])`, which must be the dot drawn.
    render(
      <ChipSlot>
        <NoticeDot workspace="gamma" count={1} kind="info" onPress={() => undefined} />
        <WorkspaceChip name="gamma" />
      </ChipSlot>,
    );
    const matched = document.querySelectorAll(':where([data-scope="titlebar"][data-part="dot"])');
    expect([...matched]).toEqual([
      screen.getByRole("button", { name: "Show the notice on gamma" }),
    ]);
    expect(matched[0].closest('[data-part="workspace"]')).toBeNull();
  });

  it("says how many it shows", () => {
    render(<NoticeDot workspace="gamma" count={3} kind="info" onPress={() => undefined} />);
    expect(screen.getByRole("button", { name: "Show the 3 notices on gamma" })).toBeInTheDocument();
  });

  it("leaves the chip's name its words, and adds what it shows to its description", () => {
    render(
      <ChipSlot>
        <WorkspaceChip name="gamma" waiting={2} notices={{ count: 1, kind: "info" }} />
      </ChipSlot>,
    );
    const chip = screen.getByRole("button", { name: "gamma" });
    expect(chip).toHaveAccessibleDescription("2 waiting, 1 notice");
  });
});

describe("a dot's kind (#3649)", () => {
  it("draws a warning as a triangle, its own part, and says it after the count", () => {
    render(<NoticeDot workspace="gamma" count={2} kind="warning" onPress={() => undefined} />);
    const dot = screen.getByRole("button", {
      name: "Show the 2 notices on gamma, most urgent: warning",
    });
    expect(dot).toHaveAttribute("data-kind", "warning");
    // Its own part, so a theme's news dot never colours it (`titlebar.dot-warning`).
    expect(dot).toHaveAttribute("data-part", "dot-warning");
    expect(dot.querySelector("svg.lucide-triangle")).not.toBeNull();
  });

  it("puts the most urgent kind in the chip's description", () => {
    render(
      <ChipSlot>
        <WorkspaceChip name="gamma" waiting={2} notices={{ count: 3, kind: "warning" }} />
      </ChipSlot>,
    );
    expect(screen.getByRole("button", { name: "gamma" })).toHaveAccessibleDescription(
      "2 waiting, 3 notices, most urgent: warning",
    );
  });

  it("ranks the kinds by urgency, news the least", () => {
    expect(mostUrgentOf([])).toBe("info");
    expect(mostUrgentOf(["info", "warning", "info"])).toBe("warning");
    expect(NOTICE_KINDS.map(urgencyOf)).toEqual([0, 1]);
  });
});

describe("the plane root's chip (#3654)", () => {
  it("is a button named for the root, the workspace chip's root variant, drawing a house", () => {
    render(<RootChip name="Plane root" current={false} onPress={() => undefined} />);
    const chip = screen.getByRole("button", { name: "Plane root" });
    expect(chip).toHaveAttribute("data-scope", "titlebar");
    expect(chip).toHaveAttribute("data-part", "workspace");
    expect(chip).toHaveAttribute("data-variant", "root");
    expect(chip).not.toHaveAttribute("aria-current");
    expect(chip.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    // Nothing to let go: no name, no + and no count of a pin's.
    expect(chip.querySelector('[data-part="plus"], [data-part="count"]')).toBeNull();
  });

  it("takes the chosen look while the plane root is in front, and goes there on a press", async () => {
    let pressed = 0;
    render(<RootChip name="Plane root" current onPress={() => (pressed += 1)} />);
    const chip = screen.getByRole("button", { name: "Plane root" });
    expect(chip).toHaveAttribute("aria-current", "true");
    expect(chip).toHaveAttribute("data-selected");
    await userEvent.click(chip);
    expect(pressed).toBe(1);
  });
});

describe("the strip", () => {
  it("is one toolbar named for what it holds, after a slash no one hears", () => {
    render(
      <>
        <TitleBarSlash />
        <PinnedStrip name="Pinned workspaces">
          <WorkspaceChip name="alpha" />
          <WorkspaceChip name="beta" />
        </PinnedStrip>
      </>,
    );
    const strip = screen.getByRole("toolbar", { name: "Pinned workspaces" });
    expect(strip.querySelectorAll('[data-part="workspace"]')).toHaveLength(2);
    expect(document.querySelector('[data-part="slash"]')).toHaveAttribute("aria-hidden", "true");
  });
});

describe("its stylesheet (R101, R103, R153, R167)", () => {
  const css = readFileSync(join(process.cwd(), "src/ui/titlebar/titlebar.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );
  const rule = (selector: string) => {
    const at = css.indexOf(`${selector} {`);
    expect(at, selector).toBeGreaterThanOrEqual(0);
    return css.slice(at, css.indexOf("}", at));
  };
  const chip = '[data-part="workspace"][data-scope="titlebar"]';

  it("draws a chip 28px tall with a 1px edge at rest, so no state changes its size", () => {
    const base = rule(chip);
    expect(base).toContain("block-size: max(28px");
    expect(base).toContain("border: 1px solid transparent");
    // A state changes colours only.
    for (const state of [
      "[data-hovered]",
      "[data-selected]",
      '[data-variant="unpinned"]',
      '[data-variant="root"][data-hovered]',
      '[data-variant="root"][data-selected]',
    ]) {
      const body = css
        .split("}")
        .filter((one) => one.includes(`${chip}${state}`))
        .join("}");
      expect(body, state).not.toMatch(/padding|block-size|inline-size|border-width|font-size/);
    }
  });

  it("draws the chosen chip on the accent's wash, never the hovered one's fill (#3345)", () => {
    // R101 as amended: under the pointer the card's fill, chosen the wash, chosen under the
    // pointer the stronger wash, each with the primary words.
    const fill = (selector: string) => /background-color: ([^;]+);/.exec(rule(selector))?.[1];
    expect(fill(`${chip}[data-hovered]`)).toBe("var(--surface-overlay)");
    expect(fill(`${chip}[data-selected]`)).toBe("var(--accent-wash)");
    expect(fill(`${chip}[data-selected][data-hovered]`)).toBe("var(--accent-wash-hover)");
    expect(rule(`${chip}[data-selected]`)).toContain("color: var(--text-primary)");
  });

  it("scrolls the strip sideways with its scrollbar hidden, at most 62% of the bar", () => {
    const strip = rule('[data-part="strip"][data-scope="titlebar"]');
    expect(strip).toContain("overflow-x: auto");
    expect(strip).toContain("scrollbar-width: none");
    expect(strip).toContain("max-inline-size: 62vw");
  });

  it("draws the slash in the faint tier, as a mark", () => {
    expect(rule('[data-part="slash"][data-scope="titlebar"]')).toContain("var(--text-faint)");
  });
});

describe("the crumb, the ⌄, a switcher's row and its pin (#3656)", () => {
  // The engine writes each apart from every chip (`parts::APART`): its own attributes, and the
  // state its own element is marked in, e.g. `titlebar.crumb.selected.fill` as
  // `:where([data-scope="titlebar"][data-part="crumb"][data-selected])`.
  const reached = (part: string, state = "") => {
    const selector = `:where([data-scope="titlebar"][data-part="${part}"]${state})`;
    return [...document.querySelectorAll(selector)];
  };

  it("reaches each by its own attributes, never inside a chip", () => {
    render(
      <>
        <ProjectCrumb name="purlis" mark={null} open onPress={() => undefined} />
        <AllWorkspaces keys="⌥⌘P" open={false} stop onPress={() => undefined} />
        <SwitcherRow id="r" name="alpha" active onPress={() => undefined} />
        <PinToggle name="alpha" pinned onChange={() => undefined} />
      </>,
    );
    const crumb = screen.getByRole("button", { name: /purlis/ });
    const all = screen.getByRole("button", { name: "All workspaces" });
    const row = screen.getByRole("option", { name: "alpha" });
    const pin = screen.getByRole("button", { name: "Unpin alpha" });
    expect(reached("crumb")).toEqual([crumb]);
    expect(reached("all")).toEqual([all]);
    expect(reached("row")).toEqual([row]);
    expect(reached("pin")).toEqual([pin]);
    for (const one of [crumb, all, row, pin]) {
      expect(one.closest('[data-part="workspace"]')).toBeNull();
    }
    // Open, pinned, and the row Enter would go to: each its selected state.
    expect(reached("crumb", "[data-selected]")).toEqual([crumb]);
    expect(reached("row", "[data-selected]")).toEqual([row]);
    expect(reached("pin", "[data-selected]")).toEqual([pin]);
    expect(reached("all", "[data-selected]")).toEqual([]);
  });

  it("marks the pointer on each, as its hover keys reach it", async () => {
    render(
      <>
        <ProjectCrumb name="purlis" mark={null} open={false} onPress={() => undefined} />
        <SwitcherRow id="r" name="alpha" active={false} onPress={() => undefined} />
      </>,
    );
    const hovered = "[data-hovered]:not([data-disabled])";
    const crumb = screen.getByRole("button", { name: /purlis/ });
    await userEvent.hover(crumb);
    expect(reached("crumb", hovered)).toEqual([crumb]);
    const row = screen.getByRole("option", { name: "alpha" });
    await userEvent.hover(row);
    expect(reached("row", hovered)).toEqual([row]);
    expect(reached("row", "[data-selected]")).toEqual([]);
  });
});
