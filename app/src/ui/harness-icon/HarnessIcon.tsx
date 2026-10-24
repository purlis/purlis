// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createElement, type ReactNode } from "react";
import type { HarnessIconRow, SvgIcon } from "../../bindings";
import { useHarnessGlyphs } from "../../theme/harnessGlyphs";
import { isPath } from "../../theme/icons";

/**
 * **A harness's icon** (#2922, spec #2897, ruling #2866): what a profile is drawn with wherever a
 * harness is named — an option card in Start a chat, a chat's tab, the rail, Settings. One
 * component, so each of them draws the same mark for the same profile.
 *
 * It draws, in order, the first it can:
 *
 * 1. **A glyph** of the icon set (`spark`, `braces`, `terminal`): the theme in force's, or the
 *    window's own where the theme draws none (`harnessGlyphs.ts`). Stroked in `currentColor`,
 *    so it takes the colour of the text it sits in: muted at rest, accent on a chosen card.
 * 2. **The person's own SVG**, as the core read it: checked shapes, never markup
 *    (`purlis_core::safesvg`). Each shape is drawn with this component's own element, and only
 *    the attributes it knows are passed on, each checked again here.
 * 3. **The monogram** of the harness kind (`C`, `OC`, `CX`), in the text's colour. Never a blank.
 *
 * **Decorative beside a name, named alone.** With no `label` it is hidden from assistive
 * technology, because the name beside it says the same thing; with one it is an image of that
 * name, for a place it stands without words.
 *
 * Sized at 14 px by `harness-icon.css` (R29), and never sized by its caller.
 */
export function HarnessIcon({
  icon,
  kind,
  label,
}: {
  /** What the core says the profile is drawn with. Absent (an older answer), the monogram. */
  icon?: HarnessIconRow | null;
  /** The profile's harness kind: `claude`, `opencode`, `codex`. */
  kind: string;
  /** Its name, where it stands without one beside it. */
  label?: string;
}) {
  const glyphs = useHarnessGlyphs();
  const glyph = icon?.glyph == null ? undefined : glyphs(icon.glyph);
  const svg = glyph === undefined && icon?.svg != null ? ownSvg(icon.svg) : undefined;
  const drawn: { as: "glyph" | "svg" | "monogram"; mark: ReactNode } = glyph
    ? {
        as: "glyph",
        mark: (
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
            focusable="false"
            aria-hidden="true"
          >
            {glyph.paths.map((d, at) => (isPath(d) ? <path key={at} d={d} /> : null))}
          </svg>
        ),
      }
    : svg
      ? { as: "svg", mark: svg }
      : {
          as: "monogram",
          mark: <span aria-hidden="true">{icon?.monogram ?? monogramOf(kind)}</span>,
        };
  return (
    <span
      className="harness-icon"
      data-drawn={drawn.as}
      data-kind={kind}
      {...(label === undefined ? { "aria-hidden": true } : { role: "img", "aria-label": label })}
    >
      {drawn.mark}
    </span>
  );
}

/** The monogram of `kind` where the core sent none: its first letter or digit, upper case. */
export function monogramOf(kind: string): string {
  return /[a-z0-9]/i.exec(kind)?.[0].toUpperCase() ?? "?";
}

/** The shapes a person's SVG may draw, by element. */
const SHAPES: ReadonlySet<string> = new Set([
  "path",
  "circle",
  "ellipse",
  "rect",
  "line",
  "polyline",
  "polygon",
]);

/** Every attribute a shape may carry, by its SVG name, as React names it. */
const ATTRIBUTES: Readonly<Record<string, string>> = {
  d: "d",
  points: "points",
  cx: "cx",
  cy: "cy",
  r: "r",
  rx: "rx",
  ry: "ry",
  x: "x",
  y: "y",
  width: "width",
  height: "height",
  x1: "x1",
  y1: "y1",
  x2: "x2",
  y2: "y2",
  fill: "fill",
  stroke: "stroke",
  opacity: "opacity",
  "fill-opacity": "fillOpacity",
  "stroke-opacity": "strokeOpacity",
  "stroke-width": "strokeWidth",
  "stroke-miterlimit": "strokeMiterlimit",
  "fill-rule": "fillRule",
  "clip-rule": "clipRule",
  "stroke-linecap": "strokeLinecap",
  "stroke-linejoin": "strokeLinejoin",
  transform: "transform",
};

/** A value that could name anything outside the icon, which no attribute here may hold. */
const REFERS = /url\s*\(|script|[<>"'`&\\]/i;

/** Four numbers. */
const VIEW_BOX = /^\s*-?[\d.e+-]+(?:[\s,]+-?[\d.e+-]+){3}\s*$/i;

/** **A person's SVG, drawn from its checked shapes**: nothing the core did not keep, and nothing
 *  this list does not know, whatever the answer holds. None where nothing is left to draw. */
function ownSvg(svg: SvgIcon): ReactNode | undefined {
  if (!VIEW_BOX.test(svg.view_box)) return undefined;
  const shapes = svg.shapes.flatMap((shape, at) => {
    if (!SHAPES.has(shape.tag)) return [];
    const props: Record<string, string | number> = { key: at };
    for (const [name, value] of Object.entries(shape.attributes)) {
      const prop = Object.prototype.hasOwnProperty.call(ATTRIBUTES, name)
        ? ATTRIBUTES[name]
        : undefined;
      if (prop === undefined || typeof value !== "string" || REFERS.test(value)) continue;
      if (name === "d" && !isPath(value)) continue;
      props[prop] = value;
    }
    return [createElement(shape.tag, props)];
  });
  if (shapes.length === 0) return undefined;
  return (
    <svg viewBox={svg.view_box} fill="currentColor" focusable="false" aria-hidden="true">
      {shapes}
    </svg>
  );
}
