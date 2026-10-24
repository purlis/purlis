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

import {
  MOTION_ATTRIBUTE,
  motionReduced,
  motionVariables,
  REDUCE,
  reduceMotionInPurlis,
  systemReducesMotion,
  type Motion,
} from "./motion";
import { apartChroma, chromaFor, hueOf, tintHex, type Apart } from "./tint";

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

  // **The grey tiers** (#2902): four for words, loudest first, each held to 4.5:1 on every
  // ground it is drawn on, and `faint` for what is not words (a tab's ×, a separator, a pin, a
  // chevron) and disabled words, held to 3:1. Quiet is informational text: line numbers,
  // placeholders, counts, paths, group labels. Informational text never takes faint.
  "text.primary",
  "text.secondary",
  "text.muted",
  "text.quiet",
  "text.faint",

  "border.subtle",
  "border.strong",
  // **A plain control's edge** (#3177): the secondary button's, a field's and a select's. Its
  // own token rather than `border.strong`, so a theme can draw its controls without hairlines
  // (Purlis: transparent, the control told by its fill) and keep every other strong rule. It is
  // never Allow's: an answer that grants keeps its accent edge whatever this is.
  "border.control",

  // What charter is drawing attention to, and where the keyboard is.
  "accent.base",
  "accent.surface",
  // **The accent's kin** (#3177): `wash` is a chosen rail or activity button's ground, a step
  // stronger than `surface`; `text` is words and icons drawn in the accent on its surface or
  // wash (the update pill, a chosen rail button); `fill` is the filled primary's ground,
  // `fill-hover` it under the pointer, and `fill-text` the words on both, held to 4.5:1.
  "accent.wash",
  "accent.text",
  "accent.fill",
  "accent.fill-hover",
  "accent.fill-text",
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

  // What waits on the person, or asks for care: the amber wash a pill or a soft warning stands on
  // (#2804's wash rule, #2913).
  "wash.warning",

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
  // track. `"platform"` ({@link PLATFORM}) leaves each but the track to the platform, as Purlis
  // Classic does; a colour, transparent included, is drawn as given. The track is drawn only with
  // a thumb, and selected text only on a selection the theme colours too.
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
 * **The colour tokens the platform draws unless a theme says** (the engine's `PLATFORM_DRAWN`):
 * one of them may be {@link PLATFORM} instead of a colour, and the window then writes no rule for
 * it, so the platform's selection, caret, placeholder and scrollbars, and the terminal's own
 * colour for selected text, are drawn as they were before a theme could say.
 */
export const PLATFORM_DRAWN = [
  "terminal.selection-foreground",
  "selection.background",
  "selection.text",
  "caret",
  "placeholder",
  "scrollbar.thumb",
] as const satisfies readonly Token[];

/** The syntax roles of {@link TOKENS}, each drawn with an italic and a bold flag. */
export const SYNTAX_ROLES = TOKENS.filter((token) => token.startsWith("syntax."));

/** The value that leaves a token of {@link PLATFORM_DRAWN} to the platform (the engine's
 *  `PLATFORM`). */
export const PLATFORM = "platform";

/** One token's name. */
export type Token = (typeof TOKENS)[number];

/** Whether a theme is drawn on a dark window or a light one. */
export type Appearance = "dark" | "light";

/** One face of a theme's font: the digest purlis serves its bytes by (`theme_font`). */
export type Face = { asset: string; weight: number; style: "normal" | "italic" | string };

/** One of a theme's fonts, as the engine resolved it (`purlis_core::theme::Font`): the family
 *  list to draw with, written by the engine, and the faces to load for it. */
export type Font = { family: string | null; stack: string; faces: Face[] };

/** The three fonts a theme sets. */
export type FontSlot = "ui" | "mono" | "terminal";

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
  /** The harness glyphs of its icon set (#2922), by name (`spark`, `braces`, `terminal`), drawn
   *  as a view's icon is. A glyph it does not draw is the window's own (`harnessGlyphs.ts`). */
  glyphs?: Readonly<Record<string, RailSymbol>>;
  /** The geometry's custom properties for each density, written by the engine from the theme's
   *  numbers (#2326, `purlis_core::theme::geometry`). */
  geometry?: Record<Density, Record<string, string>>;
  /** Its UI, mono and terminal fonts (#2326). */
  fonts?: Record<FontSlot, Font>;
  /** The tokens that are not colours (#2327), every one present: {@link FLAGS}, the
   *  {@link CURSOR_STYLE} and the {@link LINE_HEIGHT}. */
  typed: Typed;
  /** How a workspace's colour tints it (#2329, #2393): the recipe the core's theme engine
   *  resolved for it. The window keeps no list of its own and only does the arithmetic. */
  tint: TintRecipe;
  /** The stylesheet of its component parts (#2903), as the engine wrote it from their parsed
   *  values (`purlis_core::theme::parts::stylesheet`): empty, or absent, for a theme that names
   *  none, which draws every control as its family default. */
  parts?: string;
};

/** A resolved theme's tokens that are not colours, by token (`purlis_core::theme::Typed`). */
export type Typed = {
  flags: Readonly<Record<string, boolean>>;
  choices: Readonly<Record<string, string>>;
  numbers: Readonly<Record<string, number>>;
};

/**
 * A theme's tint recipe, as the engine hands it (`purlis_core::theme::Tint`): the tokens the
 * colour of the workspace in front turns on the whole window, the tokens a workspace's colour
 * turns on its own tab and its chat strip, each workspace colour's hue in this theme, in the
 * order Settings offers them, and the rules every tint holds. Both lists are empty
 * for a theme a workspace's colour does not tint.
 */
export type TintRecipe = {
  readonly window: readonly string[];
  readonly tabs: readonly string[];
  readonly hues: readonly { readonly name: string; readonly hue: number }[];
  readonly apart: readonly Apart[];
};

/** One icon of a theme's set: path data on a 24 grid. */
export type RailSymbol = { readonly paths: readonly string[] };

/** How tightly the window is laid out: the person's setting, honoured by every theme. */
export type Density = "comfortable" | "compact";

/**
 * **The person's layer, drawn over every theme** (#2326, `purlis_core::theme::Layer`): which of
 * a theme's two geometries is drawn, and the family list of each font the person named, as the
 * engine read and wrote them. The window draws the theme, then this over it, so the person's
 * font beats the theme's and their density picks the theme's geometry.
 */
export type Person = {
  density: Density;
  fonts: Partial<Record<FontSlot, string>>;
  /** Whether the person turned motion off in purlis (#2925), whatever the system says: `reduced`
   *  turns every motion off; `system` follows the system's own setting. */
  motion?: "system" | "reduced";
  /** The contrast the person asks purlis to hold, at least (#2925): `more` draws a theme's
   *  high-contrast appearance and raises the floors, which the engine does to the theme it
   *  answers (`purlis_core::theme::Least`). */
  contrast?: "standard" | "more";
  /** The person's colour for each token they set (#2925), as the engine read it. The engine
   *  draws them over each theme it answers with; the window keeps them only to know when to ask
   *  again. */
  colours?: Record<string, string>;
  complaints?: string[];
};

/** Nothing set: comfortable, the theme's fonts, motion as the system has it, the standard
 *  contrast. */
export const NO_PERSON: Person = { density: "comfortable", fonts: {} };

/** Whether more contrast is asked for, by the system or in purlis (#2925): what the window tells
 *  the engine, which then draws a theme's high-contrast appearance and, for the person's own
 *  setting, raises the floors. Either asking is enough. */
export function moreContrastAsked(over: Person = person): boolean {
  return systemAsksMoreContrast() || over.contrast === "more";
}

/** What of the person's layer the engine draws into a theme (#2925): their contrast and their
 *  colours, as one key. A theme answered for another key is answered again. */
export function personLook(over: Person = person): string {
  return `${over.contrast ?? "standard"}\u0000${JSON.stringify(over.colours ?? {})}`;
}

/** **What the window hands the engine with each ask of a theme** (#2925): its own person's
 *  contrast and colours, as JSON text, which the engine draws into the theme it answers with.
 *  Each window asks with the layer it draws, so no window's answer is drawn with another's.
 *  `null` when the layer draws every theme as it is. */
export function personAsked(over: Person = person): string | null {
  const colours = over.colours ?? {};
  if (over.contrast !== "more" && Object.keys(colours).length === 0) return null;
  return JSON.stringify({ contrast: over.contrast ?? "standard", colours });
}

/** Each follower of the person's look: told when Settings changes it, to ask for the theme in
 *  force again. */
const lookFollowers = new Set<() => void>();

/** Calls `follow` whenever the person's look changes ({@link personLook}); answers the way to
 *  stop. */
export function onPersonLook(follow: () => void): () => void {
  lookFollowers.add(follow);
  return () => {
    lookFollowers.delete(follow);
  };
}

/** The person's layer the window was handed as it was created. */
function handedPerson(): Person {
  const given = (globalThis as Record<string, unknown>)[AT_CREATION] as
    { person?: Person } | undefined;
  const person = given?.person;
  return person && typeof person === "object" && person.fonts ? person : NO_PERSON;
}

/** The custom property a token is written to: `surface.base` becomes `--surface-base`. */
export function property(token: Token): string {
  return `--${token.split(".").join("-")}`;
}

/**
 * The global the core puts what the window needs for its first frame on, before any of the
 * window's own code runs (`app/src-tauri/src/windowprefs.rs`, `windowprefs.ts`).
 */
export const AT_CREATION = "__CHARTER_AT_CREATION__";

/** **The built-in appearances, by the pick that names each**, as the core resolved them and
 *  handed them to the window (`windowprefs::handed_built_ins`): `purlis-dark`, the default's
 *  (#3034), and `purlis-classic`, Purlis Classic's dark. A pick of any other (`purlis-light`)
 *  asks the core. The window always has them; a page opened without them is not purlis's
 *  window. */
function handedBuiltIns(): Record<string, Theme> {
  const given = (globalThis as Record<string, unknown>)[AT_CREATION] as
    { built_in?: unknown } | undefined;
  const themes = given?.built_in;
  if (typeof themes !== "object" || themes === null || Array.isArray(themes))
    throw new Error("purlis's window was opened without the themes its core hands it");
  return themes as Record<string, Theme>;
}

/** The default theme's appearance, Purlis's dark (#3034), as the core handed it: what the window
 *  comes up in when nothing has said otherwise. */
function handedDefault(): Theme {
  const given = (globalThis as Record<string, unknown>)[AT_CREATION] as
    { default_theme?: unknown } | undefined;
  const theme = given?.default_theme;
  if (typeof theme !== "object" || theme === null)
    throw new Error("purlis's window was opened without the default theme its core hands it");
  return theme as Theme;
}

/** Purlis's own id (`purlis_core::theme::PURLIS`): the default theme. */
export const PURLIS = "purlis";

/** Purlis Classic's own id (`purlis_core::theme::CLASSIC`): the alternative, and the one id that
 *  names it. A pick of it draws its first appearance, the dark one, unless it names another. */
export const CLASSIC = "purlis-classic";

/** The built-in appearances a pick names, by the pick: what a project switch draws in the frame
 *  it is made. `purlis-dark` and `purlis-light` were written as the default's dark or light, and
 *  draw Purlis's (#3034); Settings no longer offers them, and labels a file that holds one. */
export const BUILT_IN: Readonly<Record<string, Theme>> = handedBuiltIns();

/** Purlis's dark and light by the ids a file may still hold, as Settings labels them. */
export const OLD_PICKS: readonly string[] = ["purlis-dark", "purlis-light"];

/**
 * **The alias table** (#1838, the #1719 theme-id item): each id a built-in had before the
 * rename, and the id it has now, as Settings labels a pick that names one. Which theme an old
 * id draws is the core's (`purlis_core::theme::built_in`); this is the window's half of
 * `purlis_core::names`' `THEME_DARK` and `THEME_LIGHT`, whose `reads` it must equal
 * (`themeIds.test.ts` holds the two to each other), and the only place in the window that
 * spells an old id. It goes when the rename's compatibility window closes (ADR 0091).
 */
export const THEME_ALIASES: Readonly<Record<string, string>> = {
  "charter-dark": "purlis-dark",
  "charter-light": "purlis-light",
};

/** The theme the window comes up in when nothing has said otherwise: Purlis's dark (#3034,
 *  `purlis_core::theme::default_theme`). */
export const DEFAULT_THEME: Theme = handedDefault();

/** What a pick of {@link CLASSIC} draws with no appearance named: Purlis Classic's dark. */
export const CLASSIC_THEME: Theme = BUILT_IN[CLASSIC];

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

/** The media query the operating system answers when it asks for more contrast (#2328): true
 *  with Increase contrast on (macOS), a high-contrast theme (Linux), or contrast themes
 *  (Windows). */
export const PREFERS_MORE_CONTRAST = "(prefers-contrast: more)";

/** Whether the operating system asks for more contrast right now: what the core is told, so a
 *  theme with an appearance tagged high contrast draws it. No where the platform cannot say. */
export function systemAsksMoreContrast(): boolean {
  return typeof matchMedia === "function" && matchMedia(PREFERS_MORE_CONTRAST).matches;
}

/** Whether two resolved themes draw the same, so a theme the core answers again is not drawn
 *  again: the terminal is told only of a change. */
export function sameTheme(a: Theme, b: Theme): boolean {
  if (a === b) return true;
  if (a.name !== b.name || a.appearance !== b.appearance) return false;
  if (TOKENS.some((token) => a.values[token] !== b.values[token])) return false;
  if (JSON.stringify(a.icons ?? {}) !== JSON.stringify(b.icons ?? {})) return false;
  if (JSON.stringify(a.glyphs ?? {}) !== JSON.stringify(b.glyphs ?? {})) return false;
  if (JSON.stringify(a.tint) !== JSON.stringify(b.tint)) return false;
  return (
    JSON.stringify(a.typed) === JSON.stringify(b.typed) &&
    JSON.stringify(a.motion) === JSON.stringify(b.motion) &&
    JSON.stringify(a.geometry) === JSON.stringify(b.geometry) &&
    JSON.stringify(a.fonts) === JSON.stringify(b.fonts) &&
    (a.parts ?? "") === (b.parts ?? "")
  );
}

/**
 * **The colours a workspace may name** (charter-app#281), in the order Settings offers them: the
 * names the engine hands each theme's hues by (`purlis_core::extension::project::theme::PALETTE`).
 * A workspace may also name a `#rrggbb`, whose own hue it tints with.
 */
export const WORKSPACE_COLOURS: readonly string[] = DEFAULT_THEME.tint.hues.map((one) => one.name);

/** The hue a workspace's colour gives in `theme`: a palette name's is the theme's own, a
 *  `#rrggbb`'s is its own. `undefined` for no colour, one purlis does not read, and a grey. */
export function hueIn(theme: Theme, colour: string | null | undefined): number | undefined {
  const named = theme.tint.hues.find((one) => one.name === colour);
  return named !== undefined ? named.hue : hueOf(colour);
}

/** Whether `colour` is one a workspace can be tinted by: a palette name, or a `#rrggbb` with a
 *  hue. */
export function isWorkspaceColour(colour: string | null | undefined): boolean {
  return hueIn(DEFAULT_THEME, colour) !== undefined;
}

/** Each theme's tints, made once per colour and token list: so a redraw with the same colour is
 *  the same object, and a strip of fifty tabs does the arithmetic once. */
const tints = new WeakMap<Theme, Map<string, Theme>>();

/**
 * `theme` with `tokens` turned to the hue `colour` gives in it, every rule the engine hands it
 * held (`tint.apartChroma`): the accent apart from danger, and Allow's edge from Keep blocked's,
 * the sides of a rule given less colour where they must be and left untinted where no amount
 * holds them, every other token at the hue's full colour (`tint.chromaFor`, #3034). The theme
 * itself for no colour, one with no hue, or no tokens.
 */
function tintedAs(
  theme: Theme,
  colour: string | null | undefined,
  tokens: readonly string[],
): Theme {
  if (colour == null || tokens.length === 0) return theme;
  let made = tints.get(theme);
  if (made === undefined) {
    made = new Map();
    tints.set(theme, made);
  }
  const key = `${colour} ${tokens.join(" ")}`;
  const known = made.get(key);
  if (known !== undefined) return known;
  const hue = hueIn(theme, colour);
  if (hue === undefined) return theme;
  const rules = theme.tint.apart
    .map((rule) => ({
      token: theme.values[rule.token as Token],
      tokenTinted: tokens.includes(rule.token),
      from: theme.values[rule.from as Token],
      fromTinted: tokens.includes(rule.from),
      distance: rule.distance,
    }))
    .filter((rule) => rule.tokenTinted || rule.fromTinted);
  const most = apartChroma(rules, hue);
  const values = { ...theme.values };
  for (const one of tokens) {
    const value = theme.values[one as Token];
    // Only the sides of a rule are held to `most`; a side no amount holds is drawn untinted.
    const chroma = chromaFor(one, most, theme.tint.apart);
    // Only a colour turns: a value left to the platform (`platform`) is drawn as it is.
    if (chroma !== undefined && typeof value === "string" && value.startsWith("#"))
      values[one as Token] = tintHex(value, hue, chroma);
  }
  const tint = { ...theme, values };
  made.set(key, tint);
  return tint;
}

/**
 * `theme` with a workspace's colour — a palette name or `#rrggbb` — turning the tokens the
 * theme's recipe tints on a workspace's tab (`theme.tint.tabs`), as the engine's `tinted` does
 * (`tint.ts` has the arithmetic and why the luminance is kept). The theme itself for no colour,
 * one with no hue, or a theme a workspace's colour does not tint.
 */
export function tinted(theme: Theme, colour: string | null | undefined): Theme {
  return tintedAs(theme, colour, theme.tint.tabs);
}

/**
 * The custom properties that draw `tokens` of `theme` in a colour, whatever the theme's recipe
 * says — what a persona's mark puts on itself as a `style` (`MARK_TOKENS`): a persona's colour
 * marks who it is, which a theme that turns the workspace tint off does not take away. Empty with
 * no colour, so an element without one reads the window's own.
 */
export function tintVariables(
  theme: Theme,
  colour: string | null | undefined,
  tokens: readonly Token[],
): Record<string, string> {
  const tint = tintedAs(theme, colour, tokens);
  if (tint === theme) return {};
  return Object.fromEntries(tokens.map((token) => [property(token), tint.values[token]]));
}

/**
 * The custom properties a workspace's tab and its chat strip put on themselves as a `style`:
 * the tokens the theme's recipe tints there, in the workspace's colour. Empty with no colour, or
 * in a theme a workspace's colour does not tint, so the element reads the window's own.
 */
export function tabTint(theme: Theme, colour: string | null | undefined): Record<string, string> {
  return tintVariables(theme, colour, theme.tint.tabs as readonly Token[]);
}

/** The colour of the workspace in front, which the window's accent is drawn in. */
let tint: string | null = null;

/**
 * Tints the tokens the theme's recipe tints on the whole window with the colour of the workspace
 * in front (charter-app#281), or takes the tint off with `null`. The theme is not redrawn and the
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
 * The tokens of {@link PLATFORM_DRAWN} the window draws: those `theme` gives a colour rather than
 * {@link PLATFORM}, as the space-separated list the stylesheet's `[data-theme-draws~="…"]` rules
 * read.
 */
export function themeDraws(theme: Theme): string {
  return PLATFORM_DRAWN.filter(
    (token) => !token.startsWith("terminal.") && theme.values[token] !== PLATFORM,
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
  // Left to the platform, the selected cells keep their own colours: xterm is not told one
  // (#2327).
  const selected = theme.values["terminal.selection-foreground"];
  if (selected !== PLATFORM) object.selectionForeground = selected;
  return object;
}

/** What xterm is told of a theme besides its colours (#2327): its own option names. */
export type XtermLook = {
  cursorStyle: (typeof CURSOR_STYLES)[number];
  cursorBlink: boolean;
  lineHeight: number;
  drawBoldTextInBrightColors: boolean;
};

/** The theme's terminal tokens that are not colours, as xterm's options. xterm blinks its cursor
 *  on a timer of its own, which no stylesheet reaches, so under reduced motion (the system's or
 *  purlis's, #2925) the cursor does not blink whatever the theme says. */
export function xtermLook(theme: Theme, reduced: boolean = motionReduced()): XtermLook {
  const style = theme.typed.choices[CURSOR_STYLE];
  return {
    cursorStyle: CURSOR_STYLES.find((one) => one === style) ?? "block",
    cursorBlink: theme.typed.flags["terminal.cursor-blink"] === true && !reduced,
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
export function apply(
  theme: Theme,
  to: HTMLElement,
  reduced = motionReduced(),
  over: Person = person,
): void {
  // The person's own setting is layered on the system's (#2925): either one is enough.
  reduced = reduced || over.motion === "reduced";
  for (const [name, value] of Object.entries(cssVariables(theme)))
    to.style.setProperty(name, value);
  for (const [name, value] of Object.entries(motionVariables(theme.motion, reduced)))
    to.style.setProperty(name, value);
  for (const [name, value] of Object.entries(drawnGeometry(theme, over)))
    to.style.setProperty(name, value);
  for (const slot of ["ui", "mono"] as const) {
    const stack = drawnFont(slot, theme, over);
    if (stack !== undefined) to.style.setProperty(`--font-${slot}`, stack);
  }
  for (const [name, value] of Object.entries(syntaxFaces(theme))) to.style.setProperty(name, value);
  drawParts(theme, to.ownerDocument);
  to.style.colorScheme = theme.appearance;
  to.dataset.theme = theme.name;
  to.dataset.themeDraws = themeDraws(theme);
  to.dataset.density = over.density;
  // The one global switch reads this (`styles.css`): every transition and keyframe stops.
  if (reduced) to.setAttribute(MOTION_ATTRIBUTE, "reduced");
  else to.removeAttribute(MOTION_ATTRIBUTE);
  loadFaces(theme, over);
}

/** The id of the `<style>` a theme's component parts are drawn from. */
export const PARTS_STYLE = "purlis-parts";

/**
 * **Draws a theme's component parts** (#2903): the engine's stylesheet, in the one `<style>` the
 * window keeps for it, in the `parts` layer `styles.css` declares above the controls. Nothing is
 * written for a theme that names none, and a theme that names some and then none takes its rules
 * away. The text is the engine's, written from parsed values (ADR 0041, property 4).
 */
function drawParts(theme: Theme, document: Document): void {
  const sheet = theme.parts ?? "";
  const drawn = document.getElementById(PARTS_STYLE);
  if (sheet === "") {
    drawn?.remove();
    return;
  }
  if (drawn !== null) {
    if (drawn.textContent !== sheet) drawn.textContent = sheet;
    return;
  }
  // Appended after the bundled `styles.css`, whose `@layer` statement already named `parts`
  // between `components` and `utilities`: the order is the first statement's, not this one's.
  const style = document.createElement("style");
  style.id = PARTS_STYLE;
  style.textContent = sheet;
  document.head.append(style);
}

/** The geometry `theme` is drawn in for `over`'s density: its custom properties, as the engine
 *  wrote them (`purlis_core::theme::Layer::drawn_geometry`). */
export function drawnGeometry(theme: Theme, over: Person = person): Record<string, string> {
  return theme.geometry?.[over.density] ?? {};
}

/** The family list `slot` is drawn in: the person's, when they named one, else the theme's
 *  (`purlis_core::theme::Layer::drawn_font`). */
export function drawnFont(
  slot: FontSlot,
  theme: Theme = current,
  over: Person = person,
): string | undefined {
  return over.fonts[slot] ?? theme.fonts?.[slot].stack;
}

/** The terminal's family list: what every pane hands xterm (#2326). */
export function terminalFont(): string {
  return drawnFont("terminal") ?? "monospace";
}

/** The faces already made, each by the name it is registered under and its digest: two slots,
 *  or two themes, that share a file still each get a face under their own name. */
const made = new Set<string>();

/** Each face's bytes as asked for, by digest: one file is asked for once a launch, however many
 *  names it is made under. */
const asked = new Map<string, Promise<ArrayBuffer | undefined>>();

/** Each face being made: what {@link facesLoaded} waits on. */
const making = new Set<Promise<void>>();

/** Forgets the faces made and asked for, as a new launch would. For tests. */
export function forgetFaces(): void {
  made.clear();
  asked.clear();
  making.clear();
}

/** **Settles once every face asked for so far is made or given up** (#2536): what the theme
 *  specimen waits on before it lays itself out, so it is pictured in the theme's own fonts. */
export async function facesLoaded(): Promise<void> {
  await Promise.all([...making]);
}

/** How a face's bytes are asked for: `theme_font` (`faces.ts` sets it, so this module stays
 *  free of the IPC). */
let fetchFace: ((asset: string) => Promise<ArrayBuffer | undefined>) | undefined;

/** Says how the window asks purlis for a face's bytes. */
export function fetchFacesWith(fetch: typeof fetchFace): void {
  fetchFace = fetch;
}

/** Whether faces wait for the first frame to be on screen ({@link holdFacesForTheFirstFrame}). */
let facesHeld = false;

/**
 * **Holds every face back until the first frame is on screen** (ADR 0026, #3034): the window
 * calls it before it draws its first theme, so asking purlis for a theme's faces, decoding them
 * and laying the text out again in them is never on the way to the first frame. The default
 * theme, Purlis, brings nine faces; the first frame is drawn in each family list's fallback.
 * {@link facesAfterTheFirstFrame} lets them go.
 */
export function holdFacesForTheFirstFrame(): void {
  facesHeld = true;
}

/** Lets the faces held back go, once the first frame is on screen, and asks for the drawn
 *  theme's at once. */
export function facesAfterTheFirstFrame(): void {
  if (!facesHeld) return;
  facesHeld = false;
  if (drawnOn !== null) loadFaces(current, person);
}

/**
 * **Loads the faces the drawn fonts need, after the frame** (#2326): each from the bytes purlis
 * serves by digest, made with `FontFace` and added to the document's fonts under the name the
 * engine wrote into the family list. Nothing waits on it: the first frame is drawn in the
 * family list's fallback, and the text redraws when a face arrives. A slot the person set draws
 * the person's font, so its faces are not asked for.
 */
function loadFaces(theme: Theme, over: Person): void {
  const fetch = fetchFace;
  if (facesHeld || fetch === undefined || typeof FontFace === "undefined" || !theme.fonts) return;
  for (const slot of ["ui", "mono", "terminal"] as const) {
    if (over.fonts[slot] !== undefined) continue;
    const font = theme.fonts[slot];
    const name = /^"([a-z0-9-]+)"/.exec(font.stack)?.[1];
    if (name === undefined) continue;
    for (const face of font.faces) {
      const key = `${name}:${face.asset}`;
      if (made.has(key)) continue;
      made.add(key);
      let bytes = asked.get(face.asset);
      if (bytes === undefined) {
        bytes = fetch(face.asset).catch(() => undefined);
        asked.set(face.asset, bytes);
      }
      const pending = (async () => {
        try {
          const given = await bytes;
          if (given === undefined) return;
          // A face takes its own copy: one file made under two names is two faces.
          const one = new FontFace(name, given.slice(0), {
            weight: String(face.weight),
            style: face.style,
          });
          await one.load();
          document.fonts.add(one);
          if (slot === "terminal") for (const follow of fontFollowers) follow(terminalFont(), true);
        } catch {
          // A face that cannot be made draws the family list's fallback; it is not asked again.
        }
      })();
      making.add(pending);
      void pending.finally(() => making.delete(pending));
    }
  }
}

/** The person's layer in force. */
let person: Person = handedPerson();
reduceMotionInPurlis(person.motion === "reduced");

/** The person's layer in force: what Settings shows. */
export function personInForce(): Person {
  return person;
}

/** Draws the person's layer over the theme in force: Settings changed their density or a
 *  font. The terminals are told their family. */
export function drawPerson(next: Person): void {
  const looked = personLook(person);
  const moved = motionReduced();
  person = next;
  reduceMotionInPurlis(next.motion === "reduced");
  if (motionReduced() !== moved) tellTheTerminals();
  // The engine draws the colours and the contrast into the theme: ask for it again.
  if (personLook(next) !== looked) for (const follow of lookFollowers) follow();
  if (drawnOn !== null) {
    apply(current, drawnOn);
    if (tint !== null) paint(current, drawnOn, tint);
  }
  for (const follow of fontFollowers) follow(terminalFont(), false);
}

/** Everything following the terminal's family: one per terminal on screen. */
const fontFollowers = new Set<(family: string, arrived: boolean) => void>();

/** Calls `follow` with the terminal's family whenever it changes: a theme or the person's font,
 *  and with `arrived` when a face of it has just been made, so its cells are measured again.
 *  Answers the way to stop. */
export function onTerminalFont(follow: (family: string, arrived: boolean) => void): () => void {
  fontFollowers.add(follow);
  return () => {
    fontFollowers.delete(follow);
  };
}

/** Writes the tokens `theme`'s recipe tints on the whole window (`theme.tint.window`) in
 *  `colour` onto `to`, or as the theme has them. */
function paint(theme: Theme, to: HTMLElement, colour: string | null): void {
  const tint = tinted(theme, colour);
  for (const token of theme.tint.window as readonly Token[])
    to.style.setProperty(property(token), tint.values[token]);
}

/** The class the crossfade between two workspaces' themes puts on the root (`App.css`). */
export const CROSSFADE = "theme-crossfade";

/** The crossfade in progress, so a second switch inside one extends it. */
let fading: ReturnType<typeof setTimeout> | undefined;

/**
 * **Draws `theme` with a short crossfade** (spec #2313, #2323): what a workspace switch does.
 * Every colour eases from the last theme's to this one's across the motion token
 * `duration.enter`, and the class comes off once it has. With reduced motion the duration is 0
 * and the class is never put on: the repaint is at once.
 */
export function crossfadeIn(theme: Theme, to: HTMLElement = document.documentElement): void {
  const ms = motionReduced() ? 0 : theme.motion.durations["duration.enter"];
  if (ms > 0) {
    to.classList.add(CROSSFADE);
    if (fading !== undefined) clearTimeout(fading);
    fading = setTimeout(() => {
      to.classList.remove(CROSSFADE);
      fading = undefined;
    }, ms);
  }
  drawIn(theme, to);
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
  if (changed) {
    for (const follow of followers) follow(theme);
    for (const follow of fontFollowers) follow(terminalFont(), false);
  }
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

/** Tells every follower the theme in force again: reduced motion changed, and a terminal's
 *  cursor blinks only while motion is on ({@link xtermLook}). */
function tellTheTerminals(): void {
  for (const follow of followers) follow(current);
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
  matchMedia(REDUCE).addEventListener("change", () => {
    if (drawnOn === null) return;
    // The person's own setting still holds when the system's is turned off (#2925).
    apply(current, drawnOn, systemReducesMotion() || person.motion === "reduced");
    tellTheTerminals();
    if (tint !== null) paint(current, drawnOn, tint);
  });
}
