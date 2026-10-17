// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * The theme: one data file, two consumers, and nothing to drift.
 *
 * **Why this module exists.** The window drew itself from five custom properties in `App.css`
 * and the terminal drew itself from hex written out again in `SessionPane.tsx` —
 * `background: "#181818"` is `--paper` spelled a second time, in a second language, with
 * nothing on earth making the two agree. That is two colour systems, and the second one is
 * the one an operator stares at all day. xterm cannot be handed a CSS custom property: its
 * theme is a JavaScript object of sixteen ANSI colours plus a background, a foreground, a
 * cursor and a selection. So the shared thing cannot be CSS, and it cannot be JavaScript
 * either without the CSS becoming a copy of it.
 *
 * It is **data**. A theme is a file of semantic tokens; charter writes CSS custom properties
 * from it and builds xterm's object from it, and neither consumer is a source. That is what
 * VS Code and Zed do, and it is the only arrangement in which the terminal is themed at all.
 *
 * **Semantic, not palette.** A theme sets `surface.raised`, never `gray-800`. The token names
 * are the contract a theme author writes against, so they are named for what they mean in
 * this window. `needs-you.base` and `danger.base` were once one value and are two tokens,
 * because they are two meanings: one marks a chat that wants the operator, the other marks an
 * answer that cannot be undone. Each has since moved alone, to clear the contrast its own use
 * asks for (`docs/design-system.md`). A theme that wanted the
 * first to shout and the second to whisper can say so; a palette token could not.
 *
 * **The window only applies a theme; the core's theme engine decides it** (spec #2313,
 * `purlis_core::theme`). Reading a theme file, holding every value to its closed grammar (hex
 * and only hex for a colour, numbers for motion), filling a token that is missing or malformed
 * from the built-in of the same appearance and saying so, choosing which theme a project draws,
 * and checking whether its words can be read: all of it is the engine's, in Rust, tested once.
 * What reaches this module is a resolved theme, every token present and every value one the
 * engine wrote from a parsed colour or number — so nothing a theme file says is ever copied into
 * a declaration (ADR 0041, property 4), and every value is one xterm accepts.
 *
 * **The first frame is drawn from what the window was handed as it was created** (ADR 0026's
 * 2 s cold start): the core resolves the person's own theme and the built-ins before the window
 * exists and puts them in the page ({@link AT_CREATION}), and `main.tsx` draws one before React
 * renders. The built-ins are compiled into the core, so nothing is read from disk for them.
 *
 * **A theme also says how the window moves** (M7.2). Durations and easings are a second, smaller
 * vocabulary under a theme file's `motion`, held to the same rules — semantic names, a closed
 * grammar, a fallback per token, nothing written outside this directory — and `motion.ts` is
 * where it and its reasons live, including why reduced motion is decided there and only there.
 */

import { motionReduced, motionVariables, REDUCE, type Motion } from "./motion";
import { hueOf, tintHex } from "./tint";

/**
 * Every token, in the order a theme file is easiest to read in.
 *
 * This array is the vocabulary as the window writes it: what goes on the document, and what the
 * guard in `literals.test.ts` says the rest of the window may not go around. The engine holds
 * the same list (`purlis_core::theme::vocabulary::TOKENS`), which a theme is validated against,
 * and `core.test.ts` fails when the two differ. Adding a colour to purlis means adding a name
 * here and there, and a value to both built-in themes; there is no third way to get one.
 */
export const TOKENS = [
  // The layers of the window, from the deepest to the one nearest the operator. `deep` is
  // the topmost strip and the bottom region, which read as furthest back; `base` is the
  // window itself; `raised` is a strip above it; `overlay` is a menu or a dialog.
  "surface.base",
  "surface.sunken",
  "surface.deep",
  "surface.raised",
  "surface.overlay",
  "surface.hover",

  // Things that are pressed. `aimed` is the row the keyboard is on, which is not the row the
  // pointer is over and must not look like it.
  "control.base",
  "control.hover",
  "control.aimed",
  "control.count",

  "text.primary",
  "text.secondary",
  "text.muted",

  "border.subtle",
  "border.strong",

  // What charter is drawing attention to, and where the keyboard is.
  "accent.base",
  "accent.surface",
  "focus.ring",
  "tab.active",

  // **The row a tree or a list has selected** (#1672): the chat in front in the Chats tree, the
  // spot the next chat starts in, the file a file tab shows. A fill in the accent's hue and an
  // edge down the row's start, so it is told by a shape as well as a colour. Its own group,
  // because it is neither the pointer (`surface.hover`, which it was drawn in until a selected
  // chat looked exactly like a hovered one) nor the tab you are on (`layer.selected`, which
  // belongs to the three strips of the axis). The keyboard's place is `focus.ring`, a ring and
  // not a fill, so the three never read as one.
  "list.selected",
  "list.selected-edge",

  // A tree's indent guides: one straight hairline per level, under the row a level hangs from
  // (#1672). Quieter than `border.subtle`'s rules between things, since a guide is structure
  // to be read past, not a divider.
  "tree.guide",

  // **The three strips of the axis, one quiet shade each, and the tab you are on** (ADR
  // 0036, charter-app#193). A project holds workspaces and a workspace holds chats; #171
  // drew that by indenting each row under the one above, the operator read the indent as stray
  // padding, and then turned down coloured rules in its place — *"this is not looks
  // professional, it should be minimalistic, and i prefer to change little bit backgrounds of
  // tabs and little lighter for selected tab"*. So the depth is a background, one small step
  // per row, outermost deepest, and `selected` is a step lighter than any of them.
  //
  // Neutral greys, no hue: the distinction is meant to be felt rather than noticed. Their own
  // group rather than `surface.*`, because what they mean is "which row of the axis" and a
  // theme author should be able to move them without moving every other surface in the window.
  "layer.project",
  "layer.workspace",
  "layer.chat",
  "layer.selected",
  // The hairline between two tabs of one strip — the operator: *"for tabs lets add very very
  // light visible border - just for little bit highlight separation"*. Its own token rather
  // than `border.subtle`, which sat so close to the layer shades that the dividers it drew were
  // invisible, and a theme should be able to lift the tabs apart without lifting every other
  // subtle rule in the window.
  "layer.divider",

  // The one signal this whole app exists for: a chat that has stopped and is waiting.
  "needs-you.base",
  "needs-you.text",

  // An answer that cannot be taken back, and the words charter says when something went wrong.
  "danger.base",
  "danger.surface",
  "danger.text",
  "danger.wash",

  // A diff in the light editor's merge view (RC-5): the wash on a line git reported changed,
  // on the base side and on the head side, and the stronger mark on the words that changed
  // inside it. Washes, so the text drawn over them keeps the terminal's own contrast.
  "diff.deleted",
  "diff.inserted",
  "diff.deleted-text",
  "diff.inserted-text",

  // A file's or a folder's icon in a tree (FM-3, #1106): the colours an icon theme
  // (`icons.ts`) may name, and the only ones. Categorical hues, as VS Code's `charts.*` are,
  // because what tells a Rust file from a TypeScript one at a glance is a hue; `folder` is its
  // own so folders can stand apart from plain files, and `motive` is the emblem drawn over a
  // named folder. Every one but `motive` is held to 3:1 on the window (`contrast.test.ts`).
  "icon.folder",
  "icon.motive",
  "icon.grey",
  "icon.red",
  "icon.orange",
  "icon.yellow",
  "icon.green",
  "icon.teal",
  "icon.blue",
  "icon.purple",
  "icon.pink",

  // What a chat, or a check on a branch, is doing. `waiting-glow` is the ring around a chat
  // that wants the operator — a colour and not a shadow recipe, so a theme can turn it off by
  // making it transparent.
  "state.running",
  "state.waiting",
  "state.waiting-glow",
  "state.failed",
  "state.success",
  "state.unreadable",

  // What goes over the window when something modal is up.
  "overlay.scrim",
  "overlay.shadow",

  // The terminal. These are the reason a theme is data: none of them can be a CSS variable,
  // because xterm is handed an object.
  "terminal.background",
  "terminal.foreground",
  "terminal.cursor",
  "terminal.cursor-accent",
  "terminal.selection",
  // What a find in a pane draws under every match, and under the one it is on (SI-4). Drawn
  // as the cells' background, beneath the terminal's own text, so both are held to the
  // foreground's contrast floor.
  "terminal.find-match",
  "terminal.find-match-active",
  // What the terminal draws selected text in (clear: each cell keeps its own colour), what it
  // draws a selection in while the pane is not focused, and its scrollbar: at rest, under the
  // pointer and held (#2327).
  "terminal.selection-foreground",
  "terminal.selection-inactive",
  "terminal.scrollbar",
  "terminal.scrollbar-hover",
  "terminal.scrollbar-active",
  "terminal.ansi.black",
  "terminal.ansi.red",
  "terminal.ansi.green",
  "terminal.ansi.yellow",
  "terminal.ansi.blue",
  "terminal.ansi.magenta",
  "terminal.ansi.cyan",
  "terminal.ansi.white",
  "terminal.ansi.bright-black",
  "terminal.ansi.bright-red",
  "terminal.ansi.bright-green",
  "terminal.ansi.bright-yellow",
  "terminal.ansi.bright-blue",
  "terminal.ansi.bright-magenta",
  "terminal.ansi.bright-cyan",
  "terminal.ansi.bright-white",

  // **Code in the light editor** (#2327, `editor/look.ts`): one colour per syntax role, each
  // with an italic and a bold flag ({@link FLAGS}). `keyword` is a language's own words and
  // modifiers; `string` strings and regular expressions; `number` numbers; `constant` true,
  // false, null and atoms; `comment` comments and meta; `function` a function's name where it
  // is called or defined; `type` type, class and namespace names; `property` a property's name;
  // `attribute` an attribute's name; `tag` a markup tag; `heading` a markdown heading;
  // `invalid` what the language could not read; `variable`, `operator` and `punctuation` the
  // rest of the text, in the editor's own colour unless a theme says otherwise.
  "syntax.keyword",
  "syntax.string",
  "syntax.number",
  "syntax.constant",
  "syntax.comment",
  "syntax.function",
  "syntax.type",
  "syntax.property",
  "syntax.attribute",
  "syntax.tag",
  "syntax.heading",
  "syntax.invalid",
  "syntax.variable",
  "syntax.operator",
  "syntax.punctuation",

  // **A markdown render** (#2327: a memory, a todo, a session record, a release's notes): its
  // links, inline code, code blocks, quotes and headings.
  "markdown.link",
  "markdown.code",
  "markdown.code-block",
  "markdown.quote",
  "markdown.heading",

  // **What the platform draws unless a theme says** (#2327): the window's text selection and
  // the words in it, the caret in a field, a field's placeholder, and the scrollbars' thumb and
  // track. Fully clear ({@link CLEAR_IS_UNSET}) leaves each to the platform, as Purlis Classic
  // does; the track is drawn only with a thumb.
  "selection.background",
  "selection.text",
  "caret",
  "placeholder",
  "scrollbar.thumb",
  "scrollbar.track",

  // **A chart's series** (#2327, `Views.tsx`'s `Chart`), a categorical palette: the first is
  // the series a chart draws today, and the rest are for a chart with more than one.
  "chart.1",
  "chart.2",
  "chart.3",
  "chart.4",
  "chart.5",
  "chart.6",
  "chart.7",
  "chart.8",

  // **The window's own background** (#2327): what is behind everything, and what the native
  // window is painted in before the first frame.
  "window.background",
] as const;

/**
 * **The tokens a theme gives as a flag, not a colour** (#2327): each syntax role's italic and
 * bold, whether the terminal's cursor blinks, and whether bold text is drawn in the bright ANSI
 * colours. The engine's `vocabulary::FLAGS`, which `core.test.ts` holds this to.
 */
export const FLAGS = [
  "syntax.keyword.italic",
  "syntax.keyword.bold",
  "syntax.string.italic",
  "syntax.string.bold",
  "syntax.number.italic",
  "syntax.number.bold",
  "syntax.constant.italic",
  "syntax.constant.bold",
  "syntax.comment.italic",
  "syntax.comment.bold",
  "syntax.function.italic",
  "syntax.function.bold",
  "syntax.type.italic",
  "syntax.type.bold",
  "syntax.property.italic",
  "syntax.property.bold",
  "syntax.attribute.italic",
  "syntax.attribute.bold",
  "syntax.tag.italic",
  "syntax.tag.bold",
  "syntax.heading.italic",
  "syntax.heading.bold",
  "syntax.invalid.italic",
  "syntax.invalid.bold",
  "syntax.variable.italic",
  "syntax.variable.bold",
  "syntax.operator.italic",
  "syntax.operator.bold",
  "syntax.punctuation.italic",
  "syntax.punctuation.bold",
  "terminal.cursor-blink",
  "terminal.bold-as-bright",
] as const;

/** The terminal cursor's shape, one of {@link CURSOR_STYLES} (the engine's `CURSOR_STYLE`). */
export const CURSOR_STYLE = "terminal.cursor-style";

/** The terminal cursor's shapes, as xterm names them. */
export const CURSOR_STYLES = ["block", "underline", "bar"] as const;

/** The terminal's line height, a multiple of its text size (the engine's `LINE_HEIGHT`). */
export const LINE_HEIGHT = "terminal.line-height";

/**
 * **The colour tokens a fully clear value leaves to what draws them by default** (the engine's
 * `CLEAR_IS_UNSET`): the window writes no rule for one of them while it is clear, so the
 * platform's selection, caret, placeholder and scrollbars, and the terminal's own colour for
 * selected text, are drawn as they were before a theme could say.
 */
export const CLEAR_IS_UNSET = [
  "terminal.selection-foreground",
  "selection.background",
  "selection.text",
  "caret",
  "placeholder",
  "scrollbar.thumb",
] as const satisfies readonly Token[];

/** The syntax roles of {@link TOKENS}, each drawn with an italic and a bold flag. */
export const SYNTAX_ROLES = TOKENS.filter((token) => token.startsWith("syntax."));

/** Whether a colour of the engine's grammar is fully clear: `#rgba` or `#rrggbbaa` with no alpha. */
export function isClear(value: string): boolean {
  return /^#(?:[0-9a-fA-F]{3}0|[0-9a-fA-F]{6}00)$/.test(value);
}

/** One token's name. */
export type Token = (typeof TOKENS)[number];

/** Whether a theme is drawn on a dark window or a light one. */
export type Appearance = "dark" | "light";

/** A theme, complete: every token in {@link TOKENS} has a value. */
export type Theme = {
  /** What to call it, as a person would. */
  name: string;
  /** Which built-in a partial theme falls back to, and what `color-scheme` the window gets so
   *  native scrollbars and form controls are drawn the right way round. */
  appearance: Appearance;
  values: Record<Token, string>;
  /** How the window moves: the durations and easings in `motion.ts`, which a theme sets under
   *  `motion` exactly as it sets colours under `tokens`. */
  motion: Motion;
  /** The theme's icon set (#2337): each view it draws, as path data the core's theme engine
   *  checked, for `RailIcon` to stroke. A view it does not draw, and every view of a theme
   *  without one, is the window's own mark (`VIEW_MARKS`). */
  icons?: Readonly<Record<string, RailSymbol>>;
  /** The tokens that are not colours (#2327), every one present: {@link FLAGS}, the
   *  {@link CURSOR_STYLE} and the {@link LINE_HEIGHT}. */
  typed: Typed;
};

/** A resolved theme's tokens that are not colours, by token (`purlis_core::theme::Typed`). */
export type Typed = {
  flags: Readonly<Record<string, boolean>>;
  choices: Readonly<Record<string, string>>;
  numbers: Readonly<Record<string, number>>;
};

/** One icon of a theme's set: path data on a 24 grid. */
export type RailSymbol = { readonly paths: readonly string[] };

/** The custom property a token is written to: `surface.base` becomes `--surface-base`. */
export function property(token: Token): string {
  return `--${token.split(".").join("-")}`;
}

/**
 * The global the core puts what the window needs for its first frame on, before any of the
 * window's own code runs (`app/src-tauri/src/windowprefs.rs`, `windowprefs.ts`).
 */
export const AT_CREATION = "__CHARTER_AT_CREATION__";

/** The themes purlis ships, as the core resolved them and handed them to the window: dark first,
 *  the theme the window comes up in. The window always has them; a page opened without them is
 *  not purlis's window. */
function handedBuiltIns(): Theme[] {
  const given = (globalThis as Record<string, unknown>)[AT_CREATION] as
    { built_in?: unknown } | undefined;
  const themes = given?.built_in;
  if (!Array.isArray(themes) || themes.length === 0)
    throw new Error("purlis's window was opened without the themes its core hands it");
  return themes as Theme[];
}

const [DARK, LIGHT] = handedBuiltIns();

/** The two themes purlis ships, by the name a theme file or a setting names them by. */
export const BUILT_IN: Record<string, Theme> = {
  [DARK.name]: DARK,
  [LIGHT.name]: LIGHT,
};

/**
 * **The alias table** (#1838, the #1719 theme-id item): each id a built-in had before the
 * rename, and the id it has now, as Settings labels a pick that names one. Which theme an old
 * id draws is the core's (`purlis_core::theme::built_in`); this is the window's half of
 * `purlis_core::names`' `THEME_DARK` and `THEME_LIGHT`, whose `reads` it must equal
 * (`themeIds.test.ts` holds the two to each other), and the only place in the window that
 * spells an old id. It goes when the rename's compatibility window closes (ADR 0091).
 */
export const THEME_ALIASES: Readonly<Record<string, string>> = {
  "charter-dark": DARK.name,
  "charter-light": LIGHT.name,
};

/** The theme the window comes up in when nothing has said otherwise. */
export const DEFAULT_THEME: Theme = DARK;

/** Purlis Classic's own id (`purlis_core::theme::CLASSIC`): a pick of it draws its first
 *  appearance, the dark one, as the engine's `built_in` answers. Not an old id, so not in
 *  {@link THEME_ALIASES}. */
export const CLASSIC = "purlis-classic";

/** The pick that follows the operating system's appearance, as a project's `[theme] use` holds it
 *  (charter-app#273; `purlis_core::extension::project::theme::SYSTEM`). */
export const SYSTEM = "system";

/** The media query the operating system answers with its appearance: true when it is light. */
export const PREFERS_LIGHT = "(prefers-color-scheme: light)";

/** Whether the operating system is light right now: what the core is told a project that picked
 *  "follow the system" draws. Dark where the platform cannot say. */
export function systemIsLight(): boolean {
  return typeof matchMedia === "function" && matchMedia(PREFERS_LIGHT).matches;
}

/** Whether two resolved themes draw the same, so a theme the core answers again is not drawn
 *  again: the terminal is told only of a change. */
export function sameTheme(a: Theme, b: Theme): boolean {
  if (a === b) return true;
  if (a.name !== b.name || a.appearance !== b.appearance) return false;
  if (TOKENS.some((token) => a.values[token] !== b.values[token])) return false;
  if (JSON.stringify(a.icons ?? {}) !== JSON.stringify(b.icons ?? {})) return false;
  if (JSON.stringify(a.typed) !== JSON.stringify(b.typed)) return false;
  return JSON.stringify(a.motion) === JSON.stringify(b.motion);
}

/**
 * **What a workspace's colour tints on the whole window** (charter-app#281): the accent, the
 * focus ring and the active-tab wash, from the workspace in front ({@link drawTint}). The
 * operator's *"make same theme but with different collor"* — and not the text, and not the
 * terminal, so every contrast the theme cleared still holds (`contrast.test.ts`).
 */
export const TINTED_WINDOW: readonly Token[] = [
  "accent.base",
  "accent.surface",
  "focus.ring",
  "tab.active",
  // A selected row is drawn in the accent's hue, so it turns with the accent (#1672).
  "list.selected",
  "list.selected-edge",
  // A markdown link and a chart's first series are drawn in the accent in Purlis Classic, and
  // turn with it as they did when they were the accent (#2327).
  "markdown.link",
  "chart.1",
];

/**
 * **And on a workspace's own tab and its chat strip**: the same, and the layer shades. Set on
 * those elements rather than on the window, because the project strip is not a workspace's and
 * every workspace tab shows its own colour whether or not it is in front
 * ({@link tintVariables}).
 */
export const TINTED_TABS: readonly Token[] = [
  ...TINTED_WINDOW,
  "layer.workspace",
  "layer.chat",
  "layer.selected",
];

/** Each theme's tints, made once per colour: so a redraw with the same colour is the same
 *  object, and a strip of fifty tabs does the arithmetic once. */
const tints = new WeakMap<Theme, Map<string, Theme>>();

/**
 * `theme` with a workspace's colour — a `tint.PALETTE` name or `#rrggbb` — turning
 * {@link TINTED_TABS} to its hue (`tint.ts` has the arithmetic and why the luminance is kept).
 * The theme itself for no colour, or one with no hue.
 */
export function tinted(theme: Theme, colour: string | null | undefined): Theme {
  const hue = hueOf(colour);
  if (hue === undefined || colour == null) return theme;
  let made = tints.get(theme);
  if (made === undefined) {
    made = new Map();
    tints.set(theme, made);
  }
  const known = made.get(colour);
  if (known !== undefined) return known;
  const values = { ...theme.values };
  for (const token of TINTED_TABS) values[token] = tintHex(theme.values[token], hue);
  const tint = { ...theme, values };
  made.set(colour, tint);
  return tint;
}

/**
 * The custom properties that draw `tokens` of `theme` in a workspace's colour — what a
 * workspace tab and its chat strip put on themselves as a `style`. Empty with no colour, so an
 * element of a workspace without one reads the window's own.
 */
export function tintVariables(
  theme: Theme,
  colour: string | null | undefined,
  tokens: readonly Token[],
): Record<string, string> {
  const tint = tinted(theme, colour);
  if (tint === theme) return {};
  return Object.fromEntries(tokens.map((token) => [property(token), tint.values[token]]));
}

/** The colour of the workspace in front, which the window's accent is drawn in. */
let tint: string | null = null;

/**
 * Tints the window's accent and focus ring with the colour of the workspace in front
 * (charter-app#281), or takes the tint off with `null`. The theme is not redrawn and the
 * terminal is not told: nothing it draws is tinted.
 */
export function drawTint(colour: string | null): void {
  if (colour === tint) return;
  tint = colour;
  if (drawnOn !== null) paint(current, drawnOn, tint);
}

/** What the engine had to put right in a theme: a value the file did not supply usably
 *  (`purlis_core::theme::Complaint`). */
export type Complaint = {
  /** What was wrong, in the words to put in front of whoever wrote the file. */
  said: string;
  /** The token it was about, when it was about one. */
  token?: string;
};

/**
 * The theme as the custom properties the stylesheet reads.
 *
 * Every token, always — a theme is complete by the time it gets here, so there is no such
 * thing as a property the stylesheet asks for and does not find, and therefore no `var(--x,
 * #3a3a3a)` fallback anywhere in `App.css`. Those fallbacks were a second palette hiding in
 * the first one.
 */
export function cssVariables(theme: Theme): Record<string, string> {
  const written: Record<string, string> = {};
  for (const token of TOKENS) written[property(token)] = theme.values[token];
  return written;
}

/**
 * **The custom properties the syntax roles' flags are written to** (#2327), which the editor's
 * highlighting reads (`editor/look.ts`): `--syntax-comment-font-style` is `italic` or `normal`,
 * `--syntax-comment-font-weight` is `bold` or `normal`.
 */
export const SYNTAX_FACE_PROPERTIES: readonly string[] = SYNTAX_ROLES.flatMap((role) => [
  `${property(role)}-font-style`,
  `${property(role)}-font-weight`,
]);

/** The syntax roles' italic and bold as the custom properties {@link SYNTAX_FACE_PROPERTIES}
 *  names. */
export function syntaxFaces(theme: Theme): Record<string, string> {
  const written: Record<string, string> = {};
  for (const role of SYNTAX_ROLES) {
    written[`${property(role)}-font-style`] = theme.typed.flags[`${role}.italic`]
      ? "italic"
      : "normal";
    written[`${property(role)}-font-weight`] = theme.typed.flags[`${role}.bold`]
      ? "bold"
      : "normal";
  }
  return written;
}

/**
 * The tokens of {@link CLEAR_IS_UNSET} the window draws: those `theme` gives a colour that is not
 * clear, as the space-separated list the stylesheet's `[data-theme-draws~="…"]` rules read.
 */
export function themeDraws(theme: Theme): string {
  return CLEAR_IS_UNSET.filter(
    (token) => !token.startsWith("terminal.") && !isClear(theme.values[token]),
  ).join(" ");
}

/** Each terminal token, and the key xterm knows it by. Written out rather than derived: the
 *  names differ in shape (`cursor-accent` against `cursorAccent`, `selection` against
 *  `selectionBackground`) and a clever conversion would be one rename away from silently
 *  handing xterm a key it ignores. */
const XTERM: [Token, string][] = [
  ["terminal.background", "background"],
  ["terminal.foreground", "foreground"],
  ["terminal.cursor", "cursor"],
  ["terminal.cursor-accent", "cursorAccent"],
  ["terminal.selection", "selectionBackground"],
  ["terminal.selection-inactive", "selectionInactiveBackground"],
  ["terminal.scrollbar", "scrollbarSliderBackground"],
  ["terminal.scrollbar-hover", "scrollbarSliderHoverBackground"],
  ["terminal.scrollbar-active", "scrollbarSliderActiveBackground"],
  ["terminal.ansi.black", "black"],
  ["terminal.ansi.red", "red"],
  ["terminal.ansi.green", "green"],
  ["terminal.ansi.yellow", "yellow"],
  ["terminal.ansi.blue", "blue"],
  ["terminal.ansi.magenta", "magenta"],
  ["terminal.ansi.cyan", "cyan"],
  ["terminal.ansi.white", "white"],
  ["terminal.ansi.bright-black", "brightBlack"],
  ["terminal.ansi.bright-red", "brightRed"],
  ["terminal.ansi.bright-green", "brightGreen"],
  ["terminal.ansi.bright-yellow", "brightYellow"],
  ["terminal.ansi.bright-blue", "brightBlue"],
  ["terminal.ansi.bright-magenta", "brightMagenta"],
  ["terminal.ansi.bright-cyan", "brightCyan"],
  ["terminal.ansi.bright-white", "brightWhite"],
];

/**
 * The theme as xterm's own object — the second consumer, built from the same values as the
 * first, which is the whole point of the file.
 */
export function xtermTheme(theme: Theme): Record<string, string> {
  const object: Record<string, string> = {};
  for (const [token, key] of XTERM) object[key] = theme.values[token];
  // Clear, the selected cells keep their own colours: xterm is not told one (#2327).
  const selected = theme.values["terminal.selection-foreground"];
  if (!isClear(selected)) object.selectionForeground = selected;
  return object;
}

/** What xterm is told of a theme besides its colours (#2327): its own option names. */
export type XtermLook = {
  cursorStyle: (typeof CURSOR_STYLES)[number];
  cursorBlink: boolean;
  lineHeight: number;
  drawBoldTextInBrightColors: boolean;
};

/** The theme's terminal tokens that are not colours, as xterm's options. */
export function xtermLook(theme: Theme): XtermLook {
  const style = theme.typed.choices[CURSOR_STYLE];
  return {
    cursorStyle: CURSOR_STYLES.find((one) => one === style) ?? "block",
    cursorBlink: theme.typed.flags["terminal.cursor-blink"] === true,
    lineHeight: theme.typed.numbers[LINE_HEIGHT] ?? 1,
    drawBoldTextInBrightColors: theme.typed.flags["terminal.bold-as-bright"] !== false,
  };
}

/** What `@xterm/addon-search` is told to draw its matches in. Its own shape, and its own
 *  required keys: the overview-ruler colours are required even with no ruler on screen. */
export type SearchDecorations = {
  matchBackground: string;
  matchOverviewRuler: string;
  activeMatchBackground: string;
  activeMatchColorOverviewRuler: string;
};

/**
 * A theme value as `#rrggbb`: the one form the search addon takes for a match's background.
 * A short value is spelled long, and an alpha is dropped — a match is a cell's background, so
 * it is drawn opaque whatever was written.
 */
function sixDigits(value: string): string {
  const digits = value.slice(1);
  const long = digits.length <= 4 ? [...digits].map((d) => d + d).join("") : digits;
  return `#${long.slice(0, 6)}`;
}

/**
 * The theme as the search addon's decorations — a third consumer of the same file, so a
 * match in a pane is coloured where every other colour is (SI-4).
 */
export function searchDecorations(theme: Theme): SearchDecorations {
  const match = sixDigits(theme.values["terminal.find-match"]);
  const active = sixDigits(theme.values["terminal.find-match-active"]);
  return {
    matchBackground: match,
    matchOverviewRuler: match,
    activeMatchBackground: active,
    activeMatchColorOverviewRuler: active,
  };
}

/**
 * Puts a theme on the document, and says what the window is now drawn on.
 *
 * `reduced` is whether the operator has asked for less motion, and it is read here rather than
 * by any component: the motion layer collapses every duration in one place (`motion.ts`), so a
 * component can neither forget to honour the setting nor honour it differently.
 *
 * Called from `main.tsx` **before React renders**, so no frame is ever painted in one theme
 * and repainted in another. It is `TOKENS.length` calls to `setProperty` on one element —
 * see `theme.bench.ts` for what that costs against ADR 0026's 2 s.
 *
 * `color-scheme` goes on too: it is what makes the platform's own scrollbars, text selection
 * and form controls match, and no custom property can do it.
 */
export function apply(theme: Theme, to: HTMLElement, reduced = motionReduced()): void {
  for (const [name, value] of Object.entries(cssVariables(theme)))
    to.style.setProperty(name, value);
  for (const [name, value] of Object.entries(motionVariables(theme.motion, reduced)))
    to.style.setProperty(name, value);
  for (const [name, value] of Object.entries(syntaxFaces(theme))) to.style.setProperty(name, value);
  to.style.colorScheme = theme.appearance;
  to.dataset.theme = theme.name;
  to.dataset.themeDraws = themeDraws(theme);
}

/** Writes {@link TINTED_WINDOW} of `theme` in `colour` onto `to`, or as the theme has them. */
function paint(theme: Theme, to: HTMLElement, colour: string | null): void {
  const tint = tinted(theme, colour);
  for (const token of TINTED_WINDOW) to.style.setProperty(property(token), tint.values[token]);
}

/** What the running window is drawn in. Read by a pane that is opening a terminal, which then
 *  follows it through {@link onDrawn}. */
let current: Theme = DEFAULT_THEME;

/** The theme in force. */
export function inForce(): Theme {
  return current;
}

/** Puts a theme in force and on the document.
 *
 *  Not called `use`: `use` is a React hook's name in React 19, and `react-hooks/rules-of-hooks`
 *  reads any call to one as a hook call — which it then refuses inside a `try`. */
export function drawIn(theme: Theme, to: HTMLElement = document.documentElement): void {
  const changed = theme !== current;
  current = theme;
  drawnOn = to;
  apply(theme, to);
  if (tint !== null) paint(theme, to, tint);
  followReducedMotion();
  if (changed) for (const follow of followers) follow(theme);
}

/**
 * **The second consumer's half of a live switch** (M6.7). The stylesheet follows a theme drawn
 * while the window is up by itself — {@link apply} rewrites the custom properties it reads —
 * but xterm was handed an *object* when a pane was built, and nothing about a custom property
 * changing reaches it. So a pane subscribes here and hands its terminal the new object; without
 * this, "one source, two consumers" held at boot and only one consumer was live.
 *
 * Told only when the theme in force actually changes. Answers the way to stop being told, which
 * a pane calls when it goes.
 */
export function onDrawn(follow: (theme: Theme) => void): () => void {
  followers.add(follow);
  return () => {
    followers.delete(follow);
  };
}

/** Everything following the theme in force: one per terminal on screen. */
const followers = new Set<(theme: Theme) => void>();

/** Where the theme in force was last drawn, so a change of the reduced-motion setting can be
 *  drawn onto the same element. */
let drawnOn: HTMLElement | null = null;

/** Whether the setting is already being followed. One listener for the life of the window. */
let following = false;

/**
 * Redraws the theme in force when the operator changes the reduced-motion setting while the
 * window is up. The platform answers the query live, and a window that only read it at launch
 * would go on animating for somebody who has just asked it to stop.
 */
function followReducedMotion(): void {
  if (following || typeof matchMedia !== "function") return;
  following = true;
  matchMedia(REDUCE).addEventListener("change", (event) => {
    if (drawnOn === null) return;
    apply(current, drawnOn, event.matches);
    if (tint !== null) paint(current, drawnOn, tint);
  });
}
