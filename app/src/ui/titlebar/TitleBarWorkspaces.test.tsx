// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { PinnedStrip, TitleBarSlash, WorkspaceChip } from "./TitleBarWorkspaces";

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

  it("marks the pointer on it", async () => {
    render(<WorkspaceChip name="alpha" />);
    const chip = screen.getByRole("button", { name: "alpha" });
    await userEvent.hover(chip);
    expect(chip).toHaveAttribute("data-hovered");
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
    for (const state of ["[data-hovered]", "[data-selected]", '[data-variant="unpinned"]']) {
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
