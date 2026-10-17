// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The icon theme** (FM-3, #1106; #1103 F5): what a file or a folder in a tree is drawn as.
 *
 * It is **data beside the colour theme**, and it is held to the colour theme's rules:
 *
 * - **A map, by name.** A file is looked up by its whole name first (`package.json`), then by
 *   its extensions, longest first (`d.ts` before `ts`); a folder by its name. Names are matched
 *   without regard to case. Whatever is not mapped is the theme's `file` or `folder`.
 * - **Colours are tokens, never literals.** Each shape of a symbol names one of the `icon.*`
 *   tokens (`theme.ts`), so the icons follow light and dark with the colour theme, a colour
 *   theme can recolour every icon set at once, and the literal guard covers the icons as it
 *   covers everything else. A shape with a colour of its own is not a shape.
 * - **Data, never markup.** A symbol is a view box and a list of path outlines, each checked
 *   against the path grammar (letters and numbers, nothing else) and drawn by `FileIcon.tsx` as
 *   React elements it builds itself. No SVG text is ever put in the page, so an icon theme an
 *   extension contributes cannot carry a script, a link, a style or a fetch: there is no field
 *   to put one in. That is ADR 0041's parse-and-re-emit, the rule the core's theme engine
 *   keeps for colour.
 * - **A bad theme never stops a tree.** `loadIcons` always answers with a complete theme: what
 *   it cannot read is said in a complaint, and the built-in's `file` and `folder` stand in for
 *   a theme that has none.
 *
 * - **Kinds of file, never states of a row** (D-1145-2, #1145). A theme says what a file *is*
 *   (TypeScript, a lockfile, a `src` folder). A row that could not be read, or a link, says what
 *   it is *not*, and the tree draws that with the window's own Lucide marks (`FileX`,
 *   `FileSymlink` in `Explorer.tsx`) in place of the theme's symbol, as it draws every other
 *   state of a row (ignored, changed, being worked on). A state drawn by the icon theme would be
 *   one a contributed theme could make look like an ordinary file, and VS Code's icon themes,
 *   which this format follows, carry no such state either.
 *
 * purlis's own, `purlis-icons`, is a small subset of Material Icon Theme's file-type icons
 * (MIT), vendored under `app/icons/material-icon-theme/` and converted to this format —
 * `icons.vendor.test.ts` holds the conversion to those files.
 */

import purlisIcons from "./purlis-icons.json";
import { TOKENS, type Token } from "./theme";

/** The tokens an icon may be coloured with: the colour theme's `icon.*`. */
export const ICON_TOKENS = TOKENS.filter((token) => token.startsWith("icon.")) as Token[];

/** One outline of a symbol, filled with one token. */
export type Shape = {
  /** SVG path data: commands and numbers only. */
  d: string;
  tone: Token;
  /** Fill by the even-odd rule rather than nonzero. */
  evenOdd: boolean;
};

/** One icon: a view box and its outlines, drawn in order. */
export type IconSymbol = {
  /** The symbol's name in its theme, which the tree puts on the row for tests and styling. */
  name: string;
  viewBox: string;
  shapes: Shape[];
};

/** An icon theme, complete: it always has a `file` and a `folder`. */
export type IconTheme = {
  name: string;
  file: IconSymbol;
  folder: IconSymbol;
  folderOpen: IconSymbol;
  /** By lower-cased extension, without its first dot: `ts`, `d.ts`. */
  extensions: ReadonlyMap<string, IconSymbol>;
  /** By lower-cased whole file name. */
  filenames: ReadonlyMap<string, IconSymbol>;
  /** By lower-cased folder name. */
  folders: ReadonlyMap<string, IconSymbol>;
  foldersOpen: ReadonlyMap<string, IconSymbol>;
};

/** What a tree row is: a folder, opened or not, or anything else (a file or a link). */
export type Row = { name: string; folder: false } | { name: string; folder: true; open: boolean };

/** The symbol `row` is drawn as. */
export function iconFor(icons: IconTheme, row: Row): IconSymbol {
  const name = row.name.toLowerCase();
  if (row.folder) {
    if (row.open) return icons.foldersOpen.get(name) ?? icons.folders.get(name) ?? icons.folderOpen;
    return icons.folders.get(name) ?? icons.folder;
  }
  const whole = icons.filenames.get(name);
  if (whole !== undefined) return whole;
  // Each extension, longest first: `a.d.ts` tries `d.ts`, then `ts`. A leading dot is part of
  // the name (`.env` has no extension, as git and every shell read it).
  for (let at = name.indexOf(".", 1); at !== -1; at = name.indexOf(".", at + 1)) {
    const found = icons.extensions.get(name.slice(at + 1));
    if (found !== undefined) return found;
  }
  return icons.file;
}

/** The most a theme may hold: past these it is a theme written to stall a window. */
export const MOST_SYMBOLS = 2048;
export const MOST_SHAPES = 32;
export const MOST_PATH = 16384;
export const MOST_NAMES = 8192;
/** The longest view box read: four numbers fit in far less, and a field read before it is
 *  measured is a field that can cost what its writer likes. */
export const MOST_VIEW_BOX = 64;

/** SVG path data, and nothing that is not: command letters, numbers, signs, separators. */
const PATH = /^[Mm][MmZzLlHhVvCcSsQqTtAa0-9eE.,+\-\s]*$/;

/** Whether `d` is path data in the grammar and under {@link MOST_PATH}: what a file icon's
 *  outline is held to here, and a theme's rail icon is held to again where it is drawn
 *  (`RailIcon`), after the core's theme engine checked it. */
export function isPath(d: unknown): d is string {
  return typeof d === "string" && d.length <= MOST_PATH && PATH.test(d);
}
/** One number of a view box. */
const NUMBER = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/;

/** A theme, and what it cost to get one. */
export type LoadedIcons = { icons: IconTheme; complaints: string[] };

const isObject = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

/** `raw`'s own entries, read into a `Map` by the callers: so a name like `__proto__` is a
 *  name, never the prototype of anything. */
const entries = (raw: unknown): [string, unknown][] => (isObject(raw) ? Object.entries(raw) : []);

/** A view box as four numbers, written back from them; `undefined` when it is not one. */
function viewBoxOf(raw: unknown): string | undefined {
  if (typeof raw !== "string" || raw.length > MOST_VIEW_BOX) return undefined;
  const parts = raw.trim().split(/[\s,]+/);
  if (parts.length !== 4 || !parts.every((part) => NUMBER.test(part))) return undefined;
  const [x, y, width, height] = parts.map(Number);
  if (![x, y, width, height].every(Number.isFinite) || width <= 0 || height <= 0) return undefined;
  return `${x} ${y} ${width} ${height}`;
}

/** One symbol, or why it is not one. */
function symbolOf(name: string, raw: unknown): IconSymbol | string {
  if (!isObject(raw)) return `the symbol ${name} is not an object`;
  const viewBox = viewBoxOf(raw.viewBox);
  if (viewBox === undefined) return `the symbol ${name} has no view box of four numbers`;
  if (!Array.isArray(raw.paths) || raw.paths.length === 0) return `the symbol ${name} has no paths`;
  if (raw.paths.length > MOST_SHAPES)
    return `the symbol ${name} has more than ${MOST_SHAPES} paths`;
  const tones = new Set<string>(ICON_TOKENS);
  const shapes: Shape[] = [];
  for (const path of raw.paths as unknown[]) {
    if (!isObject(path)) return `a path of ${name} is not an object`;
    const { d, tone } = path;
    if (!isPath(d)) return `a path of ${name} is not path data`;
    if (typeof tone !== "string" || !tones.has(tone))
      return `a path of ${name} is coloured ${JSON.stringify(tone)}, which is not one of ${ICON_TOKENS.join(", ")}`;
    shapes.push({ d, tone: tone as Token, evenOdd: path.evenOdd === true });
  }
  return { name, viewBox, shapes };
}

/**
 * Reads whatever an icon theme file parsed to, and always answers with a complete theme.
 *
 * `raw` is untrusted: `JSON.parse` of a file an extension contributed. Nothing here throws, and
 * everything that reaches the page is a string this function checked or wrote.
 */
export function loadIcons(raw: unknown, fallback: IconTheme = DEFAULT_ICONS): LoadedIcons {
  const complaints: string[] = [];
  if (!isObject(raw)) {
    return { icons: fallback, complaints: ["an icon theme is a JSON object"] };
  }
  const symbols = new Map<string, IconSymbol>();
  const given = entries(raw.symbols);
  if (given.length > MOST_SYMBOLS)
    complaints.push(`only the first ${MOST_SYMBOLS} symbols are read`);
  for (const [name, value] of given.slice(0, MOST_SYMBOLS)) {
    const symbol = symbolOf(name, value);
    if (typeof symbol === "string") complaints.push(symbol);
    else symbols.set(name, symbol);
  }
  const named = (field: string, value: unknown): IconSymbol | undefined => {
    if (value === undefined) return undefined;
    const symbol = typeof value === "string" ? symbols.get(value) : undefined;
    if (symbol === undefined)
      complaints.push(`${field} names ${JSON.stringify(value)}, which is no symbol`);
    return symbol;
  };
  const map = (field: string): Map<string, IconSymbol> => {
    const out = new Map<string, IconSymbol>();
    const pairs = entries(raw[field]);
    if (raw[field] !== undefined && !isObject(raw[field]))
      complaints.push(`${field} is an object of names to symbols`);
    for (const [key, value] of pairs.slice(0, MOST_NAMES)) {
      const symbol = named(`${field}.${key}`, value);
      if (symbol !== undefined) out.set(key.toLowerCase(), symbol);
    }
    if (pairs.length > MOST_NAMES)
      complaints.push(`only the first ${MOST_NAMES} of ${field} are read`);
    return out;
  };
  const folder = named("folder", raw.folder) ?? fallback.folder;
  const icons: IconTheme = {
    name: typeof raw.name === "string" && raw.name.trim() !== "" ? raw.name.trim() : fallback.name,
    file: named("file", raw.file) ?? fallback.file,
    folder,
    folderOpen: named("folderOpen", raw.folderOpen) ?? folder,
    extensions: map("extensions"),
    filenames: map("filenames"),
    folders: map("folders"),
    foldersOpen: map("foldersOpen"),
  };
  return { icons, complaints };
}

/** A built-in, checked into this repo: a complaint about it is a defect, so it throws. */
function builtIn(raw: unknown, base?: IconTheme): IconTheme {
  const { icons, complaints } = loadIcons(raw, base);
  if (complaints.length > 0) throw new Error(`a built-in icon theme: ${complaints.join("; ")}`);
  return icons;
}

/** A stand-in for loading the default itself: never drawn, only there to be a fallback. */
const NOTHING: IconSymbol = { name: "", viewBox: "0 0 1 1", shapes: [] };
const BARE: IconTheme = {
  name: "",
  file: NOTHING,
  folder: NOTHING,
  folderOpen: NOTHING,
  extensions: new Map(),
  filenames: new Map(),
  folders: new Map(),
  foldersOpen: new Map(),
};

/** purlis's own icon theme: what a project that picks none draws. */
export const DEFAULT_ICONS: IconTheme = builtIn(purlisIcons, BARE);

/** The icon themes purlis ships, by the name `[theme] icons` picks them by
 *  (`purlis_core::extension::BUILT_IN_ICON_THEMES`). */
export const BUILT_IN_ICONS: Record<string, IconTheme> = { [DEFAULT_ICONS.name]: DEFAULT_ICONS };

/**
 * **The icon theme's alias table** (#1854, as `theme.ts`' `THEME_ALIASES` for the colour themes,
 * #1838): each id the built-in icon theme had before the rename, and the id it has now. A
 * project file, a workspace or a saved pick that still names the old id draws the renamed icon
 * theme. It is the window's half of `purlis_core::names::ICON_THEME`, whose `reads` it must equal
 * (`themeIds.test.ts` holds the two to each other), and the only place in the window that spells
 * the old id. It goes when the rename's compatibility window closes (ADR 0091).
 */
export const ICON_THEME_ALIASES: Readonly<Record<string, string>> = {
  "charter-icons": DEFAULT_ICONS.name,
};

/** The built-in icon theme `name` names, by its id or by an id it had before
 *  ({@link ICON_THEME_ALIASES}). */
export function builtInIcons(name: string): IconTheme | undefined {
  const own = (table: object, key: string) => Object.prototype.hasOwnProperty.call(table, key);
  const id = own(ICON_THEME_ALIASES, name) ? ICON_THEME_ALIASES[name] : name;
  return own(BUILT_IN_ICONS, id) ? BUILT_IN_ICONS[id] : undefined;
}

/** What a shape carries as `data-tone` for `App.css` to colour: the token's custom property
 *  without its dashes, `icon.blue` as `icon-blue`. */
export function toneOf(token: Token): string {
  return token.split(".").join("-");
}
