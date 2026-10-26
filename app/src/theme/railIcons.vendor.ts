// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How Purlis's icon set was made from Lucide** (#2337): the Lucide icon each rail view is
 * drawn with in the Purlis theme, and the conversion of its elements to the path data a theme's
 * icon set holds (`purlis_core::theme::Icons`). Lucide is ISC and already the window's icon
 * family, so its glyphs come from the `lucide-react` the window depends on.
 *
 * Only `railIcons.vendor.test.ts` imports this, to hold
 * `crates/purlis-core/src/theme/built-in/purlis/icons/icons.json` to the Lucide it came from;
 * the window never does.
 */

/** Each rail view and the Lucide icon Purlis draws it with: the foundation's icon of the same
 *  place (`docs/design/foundation/README.md`), matched by meaning and shape. */
export const LUCIDE: Readonly<Record<string, string>> = {
  chats: "message-square",
  explorer: "network",
  search: "search",
  // #3266: the Skills view, the Skill studio's own sparkles.
  skills: "sparkles",
  repos: "folder",
  changes: "workflow",
  inbox: "bell",
  personas: "users",
  vaults: "key",
  memory: "archive",
  todos: "file-check",
  sessions: "rotate-cw-clock",
};

/** One element of a Lucide icon, as `lucide-react` holds it: a tag and its attributes. */
export type LucideNode = readonly [string, Readonly<Record<string, string>>];

/** A number as path data writes it: no trailing zeros, no float noise. */
function n(value: number): string {
  return String(Number(value.toFixed(4)));
}

/** A circle as two half arcs: one arc that ends where it starts draws nothing. */
function circle(cx: number, cy: number, r: number): string {
  return `M${n(cx - r)} ${n(cy)}A${n(r)} ${n(r)} 0 1 0 ${n(cx + r)} ${n(cy)}A${n(r)} ${n(r)} 0 1 0 ${n(cx - r)} ${n(cy)}Z`;
}

/** A rectangle, its corners rounded by `rx` and `ry` as SVG clamps them. */
function rect(x: number, y: number, w: number, h: number, rx: number, ry: number): string {
  const a = Math.min(rx, w / 2);
  const b = Math.min(ry, h / 2);
  if (a === 0 || b === 0) return `M${n(x)} ${n(y)}H${n(x + w)}V${n(y + h)}H${n(x)}Z`;
  const arc = (dx: number, dy: number) => `A${n(a)} ${n(b)} 0 0 1 ${n(dx)} ${n(dy)}`;
  return [
    `M${n(x + a)} ${n(y)}`,
    `H${n(x + w - a)}`,
    arc(x + w, y + b),
    `V${n(y + h - b)}`,
    arc(x + w - a, y + h),
    `H${n(x + a)}`,
    arc(x, y + h - b),
    `V${n(y + b)}`,
    arc(x + a, y),
    "Z",
  ].join("");
}

/** The path data of one Lucide element. Paths, circles and rectangles are what Purlis's icons
 *  use; any other element is refused, so a new icon that needs one adds it with its test. */
export function toPath([tag, attributes]: LucideNode): string {
  const num = (key: string, fallback?: number) => {
    const raw = attributes[key];
    if (raw === undefined && fallback !== undefined) return fallback;
    const value = Number(raw);
    if (!Number.isFinite(value)) throw new Error(`${tag} has no number ${key}`);
    return value;
  };
  switch (tag) {
    case "path":
      return attributes.d;
    case "circle":
      return circle(num("cx"), num("cy"), num("r"));
    case "rect": {
      const rx = attributes.rx ?? attributes.ry;
      const ry = attributes.ry ?? attributes.rx;
      return rect(
        num("x", 0),
        num("y", 0),
        num("width"),
        num("height"),
        rx === undefined ? 0 : Number(rx),
        ry === undefined ? 0 : Number(ry),
      );
    }
    default:
      throw new Error(`a ${tag} is not converted to a path yet`);
  }
}

/** A Lucide icon as a symbol of a theme's icon set: one path per element, in Lucide's order. */
export function fromLucide(node: readonly LucideNode[]): { paths: string[] } {
  return { paths: node.map(toPath) };
}
