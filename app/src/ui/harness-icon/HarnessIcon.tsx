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
 *    the attributes it knows are passed on, each checked again here, within the same budget.
 *    It is drawn in the text's colour alone, whatever colours the file named, so it never
 *    passes for a coloured status mark.
 * 3. **The monogram** of the harness kind (`C`, `OC`, `CX`), in the text's colour. Never a blank.
 *
 * **Decorative beside a name, named alone.** With no `label` it is hidden from assistive
 * technology, because the name beside it says the same thing; with one it is an image of that
 * name, for a place it stands without words.
 *
 * Sized at 14 px by `harness-icon.css` (R29), and never sized by its caller.
 *
 * **Never on a consent or ask surface.** A profile's icon is the person's to choose, and a chat
 * can write the file that names it, so it is drawn only where a profile is listed beside its
 * name: never on an Allow, an Accept, a grant or a question purlis asks.
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

/** The built-in kinds' monograms, as the core writes them (`harness_icon::monogram`). */
const MONOGRAMS: Readonly<Record<string, string>> = { claude: "C", opencode: "OC", codex: "CX" };

/** The monogram of `kind` where the core sent none, as the core makes it: the built-ins' own,
 *  and any other kind's first letter or digit, upper case. */
export function monogramOf(kind: string): string {
  if (Object.prototype.hasOwnProperty.call(MONOGRAMS, kind)) return MONOGRAMS[kind];
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

/** The most a person's icon may come to, every tag, attribute name and value counted: the core's
 *  own budget (`purlis_core::safesvg::MOST_OUTPUT`), held again here. */
export const MOST_OUTPUT = 64 * 1024;

/** A fill or stroke as an icon draws it: `none`, or the text's colour whatever the answer says. */
const paint = (value: string) => (value === "none" ? "none" : "currentColor");

/** Four numbers. */
const VIEW_BOX = /^\s*-?[\d.e+-]+(?:[\s,]+-?[\d.e+-]+){3}\s*$/i;

/** **A person's SVG, drawn from its checked shapes**: nothing the core did not keep, and nothing
 *  this list does not know, whatever the answer holds. None where nothing is left to draw. */
function ownSvg(svg: SvgIcon): ReactNode | undefined {
  if (!VIEW_BOX.test(svg.view_box)) return undefined;
  let spent = 0;
  for (const shape of svg.shapes) {
    spent += shape.tag.length;
    for (const [name, value] of Object.entries(shape.attributes))
      spent += name.length + (typeof value === "string" ? value.length : 0);
    if (spent > MOST_OUTPUT) return undefined;
  }
  const shapes = svg.shapes.flatMap((shape, at) => {
    if (!SHAPES.has(shape.tag)) return [];
    const props: Record<string, string | number> = { key: at };
    for (const [name, value] of Object.entries(shape.attributes)) {
      const prop = Object.prototype.hasOwnProperty.call(ATTRIBUTES, name)
        ? ATTRIBUTES[name]
        : undefined;
      if (prop === undefined || typeof value !== "string" || REFERS.test(value)) continue;
      if (name === "d" && !isPath(value)) continue;
      props[prop] = name === "fill" || name === "stroke" ? paint(value) : value;
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
