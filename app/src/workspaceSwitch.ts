// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Switching workspace from the title bar** (#2916, spec #2897 R98, R111–R113): what a step,
 * a sideways swipe, a pin's key and the switcher's filter decide, as functions of what they are
 * given, so each is tested without a window.
 *
 * - **A step** goes to the next or the previous pin of the project in front, round from the last
 *   to the first; from a workspace that is not pinned, to the first pin (next) or the last
 *   (previous). Choosing the workspace already current does nothing (R113).
 * - **A swipe** is a sideways wheel over the title bar (two fingers on a trackpad, or Shift and
 *   the wheel). It steps exactly one pin per gesture: once its travel passes {@link SWIPE_TRAVEL}
 *   it steps, and every further event of that gesture, the trackpad's momentum included, is
 *   ignored until the wheel has been quiet for {@link SWIPE_QUIET} (R112).
 * - **The workspace popover's key** is ⌥⌘P on a Mac and Ctrl+Alt+Shift+P elsewhere (the operator's
 *   ruling of 2026-10-24, amending R106: ⌘P stays the palette's finder of files and projects).
 * - **A pin's key** is ⌘1–⌘9 on a Mac and Ctrl+1–9 elsewhere (R111). Off a Mac a chat keeps a
 *   Ctrl chord while it has the keyboard (`Palette.theChatKeepsIt`, charter-app#106), as every
 *   key the window claims does.
 * - **The switcher's filter** matches a project by its name or by any of its workspaces' names,
 *   and narrows the workspace column to the names it matches (R98).
 */

/** How far a sideways swipe travels before it steps, in px (R112). */
export const SWIPE_TRAVEL = 60;

/** How long the wheel is quiet before a swipe has ended, in ms (R112). */
export const SWIPE_QUIET = 220;

/** A step: to the next pin, or to the previous one. */
export type Step = 1 | -1;

/**
 * **The pin a step lands on**, or `undefined` where it lands nowhere new: no pins, or the one pin
 * that is already current.
 */
export function stepTarget(
  pins: readonly string[],
  current: string | undefined,
  step: Step,
): string | undefined {
  if (pins.length === 0) return undefined;
  const here = current === undefined ? -1 : pins.indexOf(current);
  const at =
    here < 0 ? (step > 0 ? 0 : pins.length - 1) : (here + step + pins.length) % pins.length;
  const to = pins[at];
  return to === current ? undefined : to;
}

/** What a wheel event says, as much of it as a swipe reads. */
export type Wheel = {
  deltaX: number;
  deltaY: number;
  /** `WheelEvent.deltaMode`: 0 is px, 1 is lines, 2 is pages. */
  deltaMode?: number;
  shiftKey: boolean;
};

/** A line of a wheel that counts in lines, in px: what a browser scrolls a line by. */
const LINE = 16;

/**
 * **How far a wheel event goes sideways**, in px, or `undefined` for one that goes mostly up or
 * down: Shift turns a plain wheel's travel sideways, as every platform does.
 */
export function sideways(wheel: Wheel): number | undefined {
  const scale = wheel.deltaMode === 1 ? LINE : wheel.deltaMode === 2 ? LINE * 20 : 1;
  const dx = wheel.shiftKey && wheel.deltaX === 0 ? wheel.deltaY : wheel.deltaX;
  if (!wheel.shiftKey && Math.abs(dx) <= Math.abs(wheel.deltaY)) return undefined;
  if (dx === 0) return undefined;
  return dx * scale;
}

/**
 * **One swipe at a time** (R112): fed each sideways wheel event with when it came, it answers
 * the one step the gesture takes, once, and nothing more until the wheel has been quiet.
 */
export class Swipe {
  private travel = 0;
  private stepped = false;
  private last = Number.NEGATIVE_INFINITY;

  /** A sideways event of `dx` px at `at` ms: the step it takes, if this is the one. */
  feed(dx: number, at: number): Step | undefined {
    if (at - this.last > SWIPE_QUIET) {
      this.travel = 0;
      this.stepped = false;
    }
    this.last = at;
    if (this.stepped) return undefined;
    this.travel += dx;
    if (Math.abs(this.travel) <= SWIPE_TRAVEL) return undefined;
    this.stepped = true;
    return this.travel > 0 ? 1 : -1;
  }
}

/** What a key event says, as much of it as a pin's key reads. */
export type Keyed = Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">;

/**
 * **Which pin a key jumps to**, 1 to 9, or `undefined`: ⌘ and the digit on a Mac, Ctrl and the
 * digit elsewhere, with nothing else held (R111).
 */
export function pinOfKey(e: Keyed, mac: boolean): number | undefined {
  if (!/^[1-9]$/.test(e.key) || e.altKey || e.shiftKey) return undefined;
  const held = mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  return held ? Number(e.key) : undefined;
}

/** What a key event says of the physical key and the modifiers held. */
export type Coded = Keyed &
  Pick<KeyboardEvent, "code"> & { getModifierState?: (key: string) => boolean };

/**
 * **Whether a key opens the title bar's workspace popover** (the operator's ruling of 2026-10-24):
 * ⌥⌘P on a Mac, Ctrl+Alt+Shift+P elsewhere, read by the physical key, since ⌥ turns P into
 * another letter on a Mac.
 *
 * **On a Mac it takes nothing from a chat**: a terminal sends nothing for a ⌘ chord. **Off a Mac
 * it is taken from a chat on purpose**, as VS Code takes its own app keys, so the key works from
 * a chat too (the operator's decision of 2026-10-24): xterm.js would send Ctrl+Alt+Shift+P to a
 * focused terminal as `ESC ^P` (C-M-p), which bash, zsh and readline leave unbound and Emacs
 * binds to backward-list. **AltGr is not this key**: Windows reports AltGr as Ctrl+Alt, and on a
 * layout where AltGr+Shift+P types a letter (US-International's Ö) that letter is left alone.
 */
export function opensTheWorkspaces(e: Coded, mac: boolean): boolean {
  if (e.code !== "KeyP" || !e.altKey) return false;
  if (mac) return e.metaKey && !e.ctrlKey && !e.shiftKey;
  if (e.getModifierState?.("AltGraph") === true) return false;
  return e.ctrlKey && e.shiftKey && !e.metaKey;
}

/** How the workspace popover's key is spelled on this platform. */
export function workspacesKeySaid(mac: boolean): string {
  return mac ? "⌥⌘P" : "Ctrl+Alt+Shift+P";
}

/**
 * **The workspace popover's key as its cap draws it**, in a switcher's filter row: the symbols,
 * which fit beside the filter's words (`⌃⌥⇧P` off a Mac); the ⌄'s tooltip spells it out.
 */
export function workspacesKeyCap(mac: boolean): string {
  return mac ? "⌥⌘P" : "⌃⌥⇧P";
}

/** How a pin's key is spelled on this platform: `⌘3`, or `Ctrl+3`. */
export function pinKeySaid(place: number, mac: boolean): string {
  return mac ? `⌘${place}` : `Ctrl+${place}`;
}

/** A project as the switcher lists it. */
export type Listed = { name: string; workspaces: readonly string[] };

/** Whether `typed` finds `name`: case aside, anywhere in it. */
const finds = (name: string, typed: string) => name.toLowerCase().includes(typed);

/**
 * **The projects the filter keeps** (R98), by their index in `projects`: each whose name, or any
 * of whose workspaces' names, holds what was typed. Nothing typed keeps every one.
 */
export function projectsFound(projects: readonly Listed[], typed: string): number[] {
  const q = typed.trim().toLowerCase();
  return projects.flatMap((project, at) =>
    q === "" || finds(project.name, q) || project.workspaces.some((ws) => finds(ws, q)) ? [at] : [],
  );
}

/**
 * **The workspaces of `project` the filter keeps** (R98): every one when the project's own name
 * holds what was typed, else the ones whose names do.
 */
export function workspacesFound(project: Listed, typed: string): string[] {
  const q = typed.trim().toLowerCase();
  if (q === "" || finds(project.name, q)) return [...project.workspaces];
  return project.workspaces.filter((ws) => finds(ws, q));
}
