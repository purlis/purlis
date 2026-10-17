// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { isPath, toneOf, type IconSymbol } from "./theme/icons";

/**
 * **One icon of the icon theme** (FM-3, #1106), as React elements this component builds: an
 * `<svg>` with the symbol's view box and a `<path>` per outline, each carrying its token as
 * `data-tone` for `App.css` to colour. Nothing else from the theme reaches the page — no SVG
 * text, no attribute it named, no style — so a contributed icon theme cannot carry a script,
 * a link or a fetch (`theme/icons.ts` says why the data has no room for one).
 *
 * Decorative, like every icon beside a name: hidden from assistive technology, so a row is
 * named by its words.
 */
export function FileIcon({ symbol }: { symbol: IconSymbol }) {
  return (
    <svg
      className="node-icon file-icon"
      viewBox={symbol.viewBox}
      aria-hidden="true"
      focusable="false"
      data-icon={symbol.name}
    >
      {symbol.shapes.map((shape, at) => (
        <path
          key={at}
          d={shape.d}
          data-tone={toneOf(shape.tone)}
          fillRule={shape.evenOdd ? "evenodd" : undefined}
        />
      ))}
    </svg>
  );
}

/**
 * **One icon of a theme's icon set** (#2337): a view's tab on the rails, drawn the way the
 * window's own Lucide marks are: on a 24 grid, stroked 2 wide with round caps and joins in
 * `currentColor`, which the tab's rules set for its state. Only the path data is the theme's,
 * checked against the path grammar by the core's theme engine and again here, as a file icon's
 * is, so a path outside it is not drawn; every attribute is this component's, so a theme can
 * say where the lines go and nothing else.
 */
export function RailIcon({ view, paths }: { view: string; paths: readonly string[] }) {
  return (
    <svg
      className="rail-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      data-icon={view}
    >
      {paths.map((d, at) => (isPath(d) ? <path key={at} d={d} /> : null))}
    </svg>
  );
}
