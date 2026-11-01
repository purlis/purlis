// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import type { AlertRow, PlaneId } from "./bindings";
import {
  AT_CREATION,
  type Complaint,
  type Theme,
  moreContrastAsked,
  systemIsLight,
} from "./theme/theme";

/**
 * **What the window was handed as it was created**: the operator's layout file, and their theme
 * file as the core's theme engine resolved it, read by the Rust side before the window existed
 * (`src-tauri/src/windowprefs.rs`). The built-in themes come with them (`theme.ts` reads those).
 *
 * Both are files beside `machine.json` (`docs/design-system.md` has their formats), and both
 * are needed by the very first frame — a layout drawn after it is a re-layout the operator
 * watches happen. A Tauri command is asynchronous, so they are not fetched: the window's
 * initialization script defines {@link GLOBAL} before any of this code runs, and reading it is
 * a property access.
 */

/** The global the initialization script defines. `windowprefs.rs` names it too. */
export const GLOBAL = AT_CREATION;

/** One file's reading — `purlis_core::windowprefs::Reading`, as serialised. */
export type Reading = {
  /** Where the file is, or would be. */
  path: string;
  /** Whether there is a file at all. */
  found: boolean;
  /** What it holds, when charter can use it. Untrusted: a file anybody may have edited. */
  document: unknown;
  /** Why a file that is there is not in force. */
  trouble: string | null;
};

/** The person's own theme as the core read and resolved it — `purlis_core::theme::Theirs`. */
export type TheirReading = {
  /** Where the file is, or would be. */
  path: string;
  /** Whether there is a file at all. */
  found: boolean;
  /** Why a file that is there is not in force. */
  trouble: string | null;
  /** The theme it resolves to, when it is in force: complete, every value the engine's. */
  theme: Theme | null;
  /** What resolving it had to put right. */
  complaints: Complaint[];
  /** The Notice that names the consent floors it breaks, when it breaks one. */
  floors: string | null;
  /** Which of the person's colours it left out (#3023). */
  mine: readonly string[];
  /** Whether it is a format 1 file in force, offered once a move into a theme folder (#2323). */
  format_1: boolean;
  /** What is drawn in its place while the system asks for more contrast (#2328): its
   *  appearance tagged high contrast, when it has one; `null` when that draws the same. */
  more: TheirDrawn | null;
};

/** One appearance of the person's theme, as the engine drew it, its floors' Notice, and which of
 *  the person's colours it left out (#3023). */
export type TheirDrawn = { theme: Theme; floors: string | null; mine: readonly string[] };

/** One frame of {@link FirstFrame}: for a light system (when the theme follows it) and for more
 *  contrast, or not. */
export type Frame = TheirDrawn & { light: boolean; more: boolean };

/**
 * **What the first frame draws** (#2602): the theme the levels choose for the project and the
 * workspace the window was created on, resolved once by the core, which created the native
 * window in the same answer. One frame for each system appearance when it follows the system,
 * and for each contrast where the core read none (`contrast` `null`).
 */
export type FirstFrame = {
  follows_system: boolean;
  contrast: "standard" | "more" | null;
  frames: Frame[];
};

/** Both readings, and the first frame. */
export type AtCreation = { layout: Reading; theme: TheirReading; first: FirstFrame | null };

const NOTHING: Reading = { path: "", found: false, document: null, trouble: null };
const NO_THEME: TheirReading = {
  path: "",
  found: false,
  trouble: null,
  theme: null,
  complaints: [],
  floors: null,
  mine: [],
  format_1: false,
  more: null,
};

/**
 * What the window was handed, or two empty readings when it was handed nothing — a page opened
 * outside the app, or a test that has not set one. Checked field by field rather than trusted,
 * because a reading the window cannot make sense of must cost the preference and never the
 * window.
 */
export function atCreation(): AtCreation {
  const given = (globalThis as Record<string, unknown>)[GLOBAL];
  const held =
    given !== null && typeof given === "object" ? (given as Record<string, unknown>) : {};
  return {
    layout: reading(held.layout),
    theme: theirReading(held.theme),
    first: firstFrame(held.first),
  };
}

function firstFrame(raw: unknown): FirstFrame | null {
  if (raw === null || typeof raw !== "object") return null;
  const from = raw as Record<string, unknown>;
  const frames: Frame[] = [];
  for (const one of Array.isArray(from.frames) ? (from.frames as unknown[]) : []) {
    const drawn = theirDrawn(one);
    if (drawn === null) continue;
    const frame = one as Record<string, unknown>;
    frames.push({ ...drawn, light: frame.light === true, more: frame.more === true });
  }
  if (frames.length === 0) return null;
  return {
    follows_system: from.follows_system === true,
    contrast: from.contrast === "more" || from.contrast === "standard" ? from.contrast : null,
    frames,
  };
}

/** `raw` as a theme the engine resolved, or `null` when it is not one. */
function aTheme(raw: unknown): Theme | null {
  const theme = raw as { values?: unknown } | null | undefined;
  return theme !== null && typeof theme === "object" && typeof theme.values === "object"
    ? (theme as Theme)
    : null;
}

function theirDrawn(raw: unknown): TheirDrawn | null {
  if (raw === null || typeof raw !== "object") return null;
  const from = raw as Record<string, unknown>;
  const theme = aTheme(from.theme);
  if (theme === null) return null;
  return {
    theme,
    floors: typeof from.floors === "string" ? from.floors : null,
    mine: lines(from.mine),
  };
}

/** `raw` as the sentences the core wrote, or none. */
function lines(raw: unknown): readonly string[] {
  return Array.isArray(raw)
    ? (raw as unknown[]).filter((one): one is string => typeof one === "string")
    : [];
}

function theirReading(raw: unknown): TheirReading {
  if (raw === null || typeof raw !== "object") return NO_THEME;
  const from = raw as Record<string, unknown>;
  const complaints = Array.isArray(from.complaints) ? (from.complaints as unknown[]) : [];
  return {
    path: typeof from.path === "string" ? from.path : "",
    found: from.found === true,
    trouble: typeof from.trouble === "string" ? from.trouble : null,
    theme: aTheme(from.theme),
    complaints: complaints.filter(
      (one): one is Complaint =>
        one !== null && typeof one === "object" && typeof (one as Complaint).said === "string",
    ),
    floors: typeof from.floors === "string" ? from.floors : null,
    mine: lines(from.mine),
    format_1: from.format_1 === true,
    more: theirDrawn(from.more),
  };
}

function reading(raw: unknown): Reading {
  if (raw === null || typeof raw !== "object") return NOTHING;
  const from = raw as Record<string, unknown>;
  return {
    path: typeof from.path === "string" ? from.path : "",
    found: from.found === true,
    document: from.document ?? null,
    trouble: typeof from.trouble === "string" ? from.trouble : null,
  };
}

/**
 * **What charter says about this machine rather than about a project** — a layout file it could
 * not use, a theme file with a token it had to put right, a layout it could not keep.
 *
 * The Inbox is where purlis says what is wrong (#1695), and every alert the core has is about a
 * plane. These are not: they are about files in the operator's config directory, and they are
 * known to the window rather than to the core, because it is the window's vocabulary — the
 * regions and the tokens — that decides whether a file said something usable. So the window
 * keeps them here, one per subject, and every project's Inbox lists them as Notices.
 */
/**
 * One thing said about this machine: an alert and its way out (NO-6) — when its fix is a
 * setting, the Settings group it is made in, by the group's address (`you.text`; SE-22), which
 * the row links to; for the theme file, Use built-in; for a layout file purlis could not use
 * or write into, Use the default layout (#1289). One with none of these can be dismissed for
 * this launch. `remedy` is a sentence, never a command: the row reads it after the detail.
 */
export type MachineAlert = Pick<AlertRow, "severity" | "subject" | "detail"> & {
  remedy: string;
  settings?: string;
  /** The theme file's way out: move it aside and draw the built-in ({@link usingTheBuiltIn}). */
  builtIn?: true;
  /** The layout file's way out: move it aside, so the next launch starts from the default
   *  layout and the next change writes a new file ({@link usingTheDefaultLayout}). */
  defaultLayout?: true;
  /** The old theme file's offer (#2323): move it into a theme folder, once
   *  ({@link THEME_FOLDER_OFFERED}). */
  themeFolder?: true;
  /** A refused theme's way out (#2647): the project whose trust ask, asked again in place, can
   *  let it be drawn — a font file of its theme folders that its approval does not hold. */
  review?: PlaneId;
};

/** The subject of the offer to move a format 1 theme file into a theme folder, and the cause it
 *  is seen by, once on this machine (`purlis_core::windowprefs::ONCE_ON_THIS_MACHINE`). */
export const THEME_FOLDER_OFFERED = "theme-folder-offered";

const said = new Map<string, MachineAlert>();
let listed: MachineAlert[] = [];
const listeners = new Set<() => void>();

/** Says something about this machine under `subject`, replacing what was said under it before,
 *  or takes it back with `undefined`. */
export function sayAboutThisMachine(
  subject: string,
  row: Omit<MachineAlert, "subject"> | undefined,
) {
  if (row === undefined) {
    if (!said.delete(subject)) return;
  } else {
    said.set(subject, { ...row, subject });
  }
  listed = [...said.values()];
  for (const listener of listeners) listener();
}

/** What is being said about this machine right now. */
export function aboutThisMachine(): MachineAlert[] {
  return listed;
}

/** {@link aboutThisMachine}, for a component that redraws when it changes. */
export function useAboutThisMachine(): MachineAlert[] {
  return useSyncExternalStore((listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }, aboutThisMachine);
}

/** {@link theirTheme} of what the window was handed, read once: so drawing it again after a
 *  project's own theme is the same object, and repaints nothing that is already drawn. `more`
 *  is what is drawn in its place while the system asks for more contrast (#2328). */
let theirs: { theme: Theme | undefined; more: Theme | undefined } | undefined;

export function theirThemeOnce(): Theme | undefined {
  theirs ??= { theme: theirTheme(), more: atCreation().theme.more?.theme };
  if (theirs.theme === undefined) return undefined;
  return moreContrastAsked() && theirs.more !== undefined ? theirs.more : theirs.theme;
}

/** The frame drawn first, read once: so drawing it again is the same object. */
let first: Frame | undefined;

/**
 * **The frame the window's first frame draws** (#2602): of what the core handed, the one for the
 * system's appearance (while the theme follows it) and its contrast: the contrast the core read
 * where it read one, else `prefers-contrast`. `undefined` when nothing was handed. Which of the
 * person's colours it left out is said as it is read, so the Notice is up with the frame (#3023).
 */
export function firstFrameOnce(): Frame | undefined {
  if (first !== undefined) return first;
  const handed = atCreation().first;
  if (handed === null) return undefined;
  const light = handed.follows_system && systemIsLight();
  const more = handed.contrast === null ? moreContrastAsked() : handed.contrast === "more";
  const fits = (frame: Frame) => frame.light === light;
  first =
    handed.frames.find((frame) => fits(frame) && frame.more === more) ??
    handed.frames.find(fits) ??
    handed.frames[0];
  sayYourColours(first.mine);
  return first;
}

/** Whether the first frame follows the system's appearance, as the core resolved it. */
export function firstFollowsSystem(): boolean {
  return atCreation().first?.follows_system === true;
}

/** The Notice naming the consent floors the person's theme breaks as it is drawn now: for more
 *  contrast, its appearance tagged high contrast's, when it has one. */
export function theirFloors(reading: TheirReading = atCreation().theme): string | null {
  return moreContrastAsked() && reading.more !== null ? reading.more.floors : reading.floors;
}

/** Which of the person's colours their own theme left out as it is drawn now (#3023), as
 *  {@link theirFloors} says its floors. */
export function theirMine(reading: TheirReading = atCreation().theme): readonly string[] {
  return moreContrastAsked() && reading.more !== null ? reading.more.mine : reading.mine;
}

/** Which of the person's colours the built-in the window was handed for `pick` left out (#3023);
 *  the core leaves a pick out of the table where it left nothing out. */
export function builtInMine(pick: string): readonly string[] {
  const given = (globalThis as Record<string, unknown>)[GLOBAL] as
    { built_in_mine?: unknown } | undefined;
  const table = given?.built_in_mine;
  if (table === null || typeof table !== "object") return [];
  return Object.prototype.hasOwnProperty.call(table, pick)
    ? lines((table as Record<string, unknown>)[pick])
    : [];
}

/** Says, or takes back, which of the person's colours the theme in force leaves out, and why
 *  (#2925): each would put a pair under its floor in this theme. */
export function sayYourColours(mine: readonly string[] | undefined) {
  sayAboutThisMachine(
    "your colours",
    mine !== undefined && mine.length > 0
      ? {
          severity: "warn",
          detail: mine.join("; "),
          remedy: "pick another colour in Settings, or clear it to draw the theme's",
          settings: "you.colours",
        }
      : undefined,
  );
}

/** For tests: read the handed theme again. */
export function forgetTheirTheme() {
  theirs = undefined;
  first = undefined;
}

/**
 * **The theme file was moved aside** (NO-6, Use built-in): from here on the window has no theme
 * of the operator's, and nothing more to say about one. The caller redraws what is in force.
 */
export function usingTheBuiltIn() {
  theirs = { theme: undefined, more: undefined };
  sayAboutThisMachine("theme", undefined);
  sayTheFloors(null);
}

/**
 * **The consent floors the theme drawn breaks** (ADR 0097 §6): the engine drew purlis's own
 * values for those tokens, and `floors` is its sentence naming the floors and the theme. One
 * Notice, for the theme in force: a theme that clears them, `null`, takes it back. `theirs` is
 * the person's own theme file's path, when it is theirs, whose way out is Use built-in.
 */
export function sayTheFloors(floors: string | null, { theirs }: { theirs?: string } = {}) {
  if (floors === null) {
    sayAboutThisMachine("consent floors", undefined);
    return;
  }
  sayAboutThisMachine("consent floors", {
    severity: "warn",
    detail: floors,
    remedy:
      theirs === undefined
        ? "pick another theme, or ask its author to fix it"
        : `fix ${theirs}, or use the built-in, which moves it aside`,
    ...(theirs === undefined ? {} : { builtIn: true as const }),
  });
}

/**
 * **The layout file was moved aside** (#1289, Use the default layout): there is no file of the
 * operator's in the way any more, so nothing is left to say about the one that could not be used
 * or written into. The window keeps what it draws now; the next change it keeps writes a new
 * file, and the next launch starts from the default layout.
 */
export function usingTheDefaultLayout() {
  sayAboutThisMachine("layout", undefined);
  sayAboutThisMachine("dismissed", undefined);
  for (const listener of layoutMovedAside) listener();
}

const layoutMovedAside = new Set<() => void>();

/** Told when the layout file is moved aside (`regions.ts` forgets what it read from it, so a
 *  project opened afterwards starts from the default). Answers the way to stop being told. */
export function onLayoutMovedAside(listener: () => void): () => void {
  layoutMovedAside.add(listener);
  return () => layoutMovedAside.delete(listener);
}

/**
 * **The operator's own theme, or none** (M6.7) — `charter/theme.json`, the address
 * `docs/design-system.md` gave it, and until now read by nothing.
 *
 * It arrives with the window, so `main.tsx` draws it **before the first frame**: an operator
 * with a theme of their own never sees the built-in painted first. The core's theme engine is
 * what judged it — every token against the vocabulary and the hex grammar — and whatever it had
 * to put right is said in the Inbox, with the file's path, because a colour that silently
 * came out as the built-in's is a theme that looks like it was ignored.
 *
 * A file that is not a theme at all is said the same way, and the window keeps the built-in.
 */
export function theirTheme(reading: TheirReading = atCreation().theme): Theme | undefined {
  const where = reading.path || "the theme file";
  if (reading.trouble !== null) {
    sayAboutThisMachine("theme", {
      severity: "warn",
      detail: `${reading.trouble} — the window is drawn in the built-in theme`,
      remedy: `fix ${where}, or use the built-in, which moves it aside`,
      builtIn: true,
    });
    return undefined;
  }
  const { theme, complaints } = reading;
  if (theme === null) return undefined;
  sayTheFloors(theirFloors(reading), { theirs: where });
  if (complaints.length > 0) {
    sayAboutThisMachine("theme", {
      severity: "warn",
      detail: `${where}: ${complaints.map((one) => one.said).join("; ")}`,
      remedy: `fix ${where}; everything else in it is drawn`,
      builtIn: true,
    });
  }
  // An old single-file theme keeps working, and is offered once to move into a theme folder
  // (spec #2313): the row draws only while the offer was not seen on this machine.
  if (reading.format_1)
    sayAboutThisMachine(THEME_FOLDER_OFFERED, {
      severity: "warn",
      detail: `${where} is a theme in the old single-file format; it keeps working`,
      remedy:
        "move it into a theme folder of your own, where it can grow fonts, icons and more appearances",
      themeFolder: true,
    });
  return theme;
}
