// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import type { HarnessIconRow } from "../../bindings";
import { OWN_GLYPHS } from "../../theme/harnessGlyphs";
import { CLASSIC_THEME, drawIn, type Theme } from "../../theme/theme";
import { HarnessIcon, monogramOf } from "./HarnessIcon";

/**
 * **A harness's icon** (#2922): the glyph of the icon set, the person's own SVG, or the
 * monogram, never a blank; decorative beside a name and named alone.
 */

const SPARK: HarnessIconRow = { glyph: "spark", svg: null, monogram: "C" };

const DUSK: Theme = {
  ...CLASSIC_THEME,
  name: "Dusk",
  glyphs: { spark: { paths: ["M1 1h22"] } },
};

afterEach(() => {
  act(() => drawIn(CLASSIC_THEME));
  cleanup();
});

const drawn = (container: HTMLElement) => container.querySelector(".harness-icon");
const paths = (container: HTMLElement) =>
  [...container.querySelectorAll("path")].map((path) => path.getAttribute("d"));

describe("a harness icon", () => {
  it("draws the window's own glyph where the theme draws none", () => {
    const { container } = render(<HarnessIcon icon={SPARK} kind="claude" />);
    expect(drawn(container)?.getAttribute("data-drawn")).toBe("glyph");
    expect(paths(container)).toEqual(OWN_GLYPHS.spark.paths);
    const svg = container.querySelector("svg");
    expect(svg?.getAttribute("stroke")).toBe("currentColor");
    expect(svg?.getAttribute("fill")).toBe("none");
  });

  it("draws the theme's glyph where its icon set has one (an icon pack redraws it)", () => {
    act(() => drawIn(DUSK));
    const { container } = render(<HarnessIcon icon={SPARK} kind="claude" />);
    expect(paths(container)).toEqual(["M1 1h22"]);
  });

  it("draws the monogram for a profile with no icon, and for a glyph it has none of", () => {
    for (const icon of [
      { glyph: null, svg: null, monogram: "OC" },
      { glyph: "comet", svg: null, monogram: "OC" },
    ]) {
      const { container } = render(<HarnessIcon icon={icon} kind="opencode" />);
      expect(drawn(container)?.getAttribute("data-drawn")).toBe("monogram");
      expect(drawn(container)?.textContent).toBe("OC");
      cleanup();
    }
  });

  it("is never blank, even drawn from an answer that carries no icon", () => {
    const { container } = render(<HarnessIcon kind="codex" />);
    expect(drawn(container)?.textContent).toBe("CX");
    expect([monogramOf("claude"), monogramOf("opencode"), monogramOf("shelly")]).toEqual([
      "C",
      "OC",
      "S",
    ]);
    expect(monogramOf("-x")).toBe("X");
    expect(monogramOf("")).toBe("?");
  });

  it("is hidden beside a name and is an image of its name alone", () => {
    const { container } = render(<HarnessIcon icon={SPARK} kind="claude" />);
    expect(drawn(container)?.getAttribute("aria-hidden")).toBe("true");
    cleanup();
    render(<HarnessIcon icon={SPARK} kind="claude" label="Claude Code" />);
    expect(screen.getByRole("img", { name: "Claude Code" })).toBeInTheDocument();
  });

  it("draws a person's SVG from its shapes, and nothing the shapes do not hold", () => {
    const icon: HarnessIconRow = {
      glyph: null,
      monogram: "C",
      svg: {
        view_box: "0 0 32 32",
        shapes: [
          { tag: "circle", attributes: { cx: "16", cy: "16", r: "8", fill: "none" } },
          { tag: "path", attributes: { d: "M1 1h4", onclick: "alert(1)", "stroke-width": "2" } },
          { tag: "path", attributes: { d: "javascript:alert(1)", fill: "url(#x)" } },
          { tag: "script", attributes: { href: "x" } },
          { tag: "image", attributes: { href: "https://example.invalid/x.png" } },
        ],
      },
    };
    const { container } = render(<HarnessIcon icon={icon} kind="claude" />);
    expect(drawn(container)?.getAttribute("data-drawn")).toBe("svg");
    const svg = container.querySelector("svg");
    expect(svg?.getAttribute("viewBox")).toBe("0 0 32 32");
    expect(container.querySelector("circle")?.getAttribute("fill")).toBe("none");
    const [plain, refused] = container.querySelectorAll("path");
    expect(plain.getAttribute("d")).toBe("M1 1h4");
    expect(plain.getAttribute("stroke-width")).toBe("2");
    expect(plain.getAttribute("onclick")).toBeNull();
    expect(refused.getAttribute("d")).toBeNull();
    expect(refused.getAttribute("fill")).toBeNull();
    expect(container.querySelector("script, image")).toBeNull();
    expect(container.innerHTML).not.toMatch(/alert|example|url\(/);
  });

  it("draws a person's SVG in the text's colour alone", () => {
    const icon: HarnessIconRow = {
      glyph: null,
      monogram: "C",
      svg: {
        view_box: "0 0 24 24",
        shapes: [{ tag: "path", attributes: { d: "M1 1h4", fill: "gold", stroke: "teal" } }],
      },
    };
    const { container } = render(<HarnessIcon icon={icon} kind="claude" />);
    const path = container.querySelector("path");
    expect(path?.getAttribute("fill")).toBe("currentColor");
    expect(path?.getAttribute("stroke")).toBe("currentColor");
  });

  it("draws the monogram for an SVG past the budget", () => {
    const icon: HarnessIconRow = {
      glyph: null,
      monogram: "C",
      svg: {
        view_box: "0 0 24 24",
        shapes: Array.from({ length: 64 }, () => ({
          tag: "path",
          attributes: { d: "M0 0", transform: "rotate(1) ".repeat(120) },
        })),
      },
    };
    const { container } = render(<HarnessIcon icon={icon} kind="claude" />);
    expect(drawn(container)?.getAttribute("data-drawn")).toBe("monogram");
    expect(container.querySelector("path")).toBeNull();
  });

  it("draws the monogram for an SVG it cannot draw", () => {
    const icon: HarnessIconRow = {
      glyph: null,
      monogram: "CX",
      svg: { view_box: "0 0 javascript", shapes: [{ tag: "path", attributes: { d: "M0 0" } }] },
    };
    const { container } = render(<HarnessIcon icon={icon} kind="codex" />);
    expect(drawn(container)?.textContent).toBe("CX");
  });
});
