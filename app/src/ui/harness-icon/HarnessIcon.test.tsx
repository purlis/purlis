// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import type { HarnessIconRow, SvgIcon } from "../../bindings";
import { OWN_GLYPHS } from "../../theme/harnessGlyphs";
import { CLASSIC_THEME, drawIn, type Theme } from "../../theme/theme";
import { HarnessIcon, MOST_OUTPUT, monogramOf, ownIconsOf } from "./HarnessIcon";

/**
 * **A harness's icon** (#2922): the glyph of the icon set, the person's own SVG, or the
 * monogram, never a blank; decorative beside a name and named alone.
 */

const SPARK: HarnessIconRow = { glyph: "spark", own: null, monogram: "C" };

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
      { glyph: null, own: null, monogram: "OC" },
      { glyph: "comet", own: null, monogram: "OC" },
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
    const own: SvgIcon = {
      view_box: "0 0 32 32",
      shapes: [
        { tag: "circle", attributes: { cx: "16", cy: "16", r: "8", fill: "none" } },
        { tag: "path", attributes: { d: "M1 1h4", onclick: "alert(1)", "stroke-width": "2" } },
        { tag: "path", attributes: { d: "javascript:alert(1)", fill: "url(#x)" } },
        { tag: "script", attributes: { href: "x" } },
        { tag: "image", attributes: { href: "https://example.invalid/x.png" } },
      ],
    };
    const icon: HarnessIconRow = { glyph: null, own: "mine.svg", monogram: "C" };
    const { container } = render(<HarnessIcon icon={icon} svg={own} kind="claude" />);
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
    const own: SvgIcon = {
      view_box: "0 0 24 24",
      shapes: [{ tag: "path", attributes: { d: "M1 1h4", fill: "gold", stroke: "teal" } }],
    };
    const icon: HarnessIconRow = { glyph: null, own: "mine.svg", monogram: "C" };
    const { container } = render(<HarnessIcon icon={icon} svg={own} kind="claude" />);
    const path = container.querySelector("path");
    expect(path?.getAttribute("fill")).toBe("currentColor");
    expect(path?.getAttribute("stroke")).toBe("currentColor");
  });

  it("draws the monogram for an SVG past the budget", () => {
    const own: SvgIcon = {
      view_box: "0 0 24 24",
      shapes: Array.from({ length: 64 }, () => ({
        tag: "path",
        attributes: { d: "M0 0", transform: "rotate(1) ".repeat(120) },
      })),
    };
    const icon: HarnessIconRow = { glyph: null, own: "mine.svg", monogram: "C" };
    const { container } = render(<HarnessIcon icon={icon} svg={own} kind="claude" />);
    expect(drawn(container)?.getAttribute("data-drawn")).toBe("monogram");
    expect(container.querySelector("path")).toBeNull();
  });

  it("draws the monogram for an SVG it cannot draw", () => {
    const own: SvgIcon = {
      view_box: "0 0 javascript",
      shapes: [{ tag: "path", attributes: { d: "M0 0" } }],
    };
    const icon: HarnessIconRow = { glyph: null, own: "mine.svg", monogram: "CX" };
    const { container } = render(<HarnessIcon icon={icon} svg={own} kind="codex" />);
    expect(drawn(container)?.textContent).toBe("CX");
  });

  it("is drawn from the row's own SVG only where the row names it", () => {
    const own: SvgIcon = {
      view_box: "0 0 24 24",
      shapes: [{ tag: "path", attributes: { d: "M1 1" } }],
    };
    const { container } = render(<HarnessIcon icon={SPARK} svg={own} kind="claude" />);
    expect(drawn(container)?.getAttribute("data-drawn")).toBe("glyph");
    cleanup();
    const plain: HarnessIconRow = { glyph: null, own: null, monogram: "C" };
    render(<HarnessIcon icon={plain} svg={own} kind="claude" />);
    expect(document.querySelector(".harness-icon")?.getAttribute("data-drawn")).toBe("monogram");
  });
});

/** A list that draws many rows' own SVGs holds every drawing of them to one budget (#2922). */
describe("a list's own icons", () => {
  /** An SVG of about `size` once counted. */
  const sized = (size: number): SvgIcon => ({
    view_box: "0 0 24 24",
    shapes: [{ tag: "path", attributes: { d: `M0 0${"h1".repeat(size / 2)}` } }],
  });
  const naming = (own: string | null) => ({ icon: { glyph: null, own, monogram: "C" } });

  it("hands each row the SVG it names while the list's budget holds", () => {
    const svgs = { "a.svg": sized(MOST_OUTPUT - 100) };
    const rows = Array.from({ length: 300 }, () => naming("a.svg"));
    const owns = ownIconsOf(rows, svgs);
    const handed = owns.filter((svg) => svg !== undefined).length;
    expect(handed).toBe(4);
    expect(owns.slice(0, 4).every((svg) => svg === svgs["a.svg"])).toBe(true);
    expect(owns.slice(4).every((svg) => svg === undefined)).toBe(true);
  });

  it("hands nothing to a row naming none, a key the answer lacks, or an SVG past its own budget", () => {
    const svgs = { "big.svg": sized(MOST_OUTPUT + 10), "ok.svg": sized(10) };
    expect(
      ownIconsOf([naming(null), naming("gone.svg"), naming("big.svg"), naming("ok.svg"), {}], svgs),
    ).toEqual([undefined, undefined, undefined, svgs["ok.svg"], undefined]);
    expect(ownIconsOf([naming("ok.svg")], null)).toEqual([undefined]);
  });
});
