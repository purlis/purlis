// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";

/**
 * **A message that names a window control links to it, and it opens focused** (Q182, R-333).
 *
 * The core's messages name what to press where only the window can do it ("press Trust this
 * clone on its notice in the window", `forge/gitcredential.rs`). On 2026-11-12 that control was
 * one Notice among hundreds, and nothing led to it. Each control a message names, and that
 * stands on a Notice, is listed here: what it says and the family of the Notice it stands on.
 * Where the window shows such a message (a result's words), it carries **Go to** that control:
 * the project's Notices view opens, showing that kind, with the keyboard on the control.
 *
 * `windowControls.test.ts` finds every core message that names a window control: each is
 * listed here, or held in its exact list of those that link nowhere yet (Q234), so a new one
 * fails it until it is one or the other.
 */
export type WindowControl = {
  /** The control's label, as the message names it and the Notice draws it. */
  words: string;
  /** The family of the Notice it stands on (`Notice.tsx`'s `familyOf`). */
  family: string;
};

export const WINDOW_CONTROLS: readonly WindowControl[] = [
  // `CloneVouchNotice.tsx`'s CLONE_VOUCH, written out: that module draws a Notice, and `Notice.tsx`
  // reads this one.
  { words: "Trust this clone", family: "clone-vouch" },
];

/** The controls `message` names, in the list's order. */
export const controlsNamedIn = (message: string): WindowControl[] =>
  WINDOW_CONTROLS.filter((control) => message.includes(control.words));

/** The words of the way to `control` on a message that names it. */
export const goToSays = (control: WindowControl): string => `Go to ${control.words}`;

/**
 * **A Go to, asked for**: the control, the project whose Notices view goes to it, and the words
 * of the message that named it, which say which of its Notices it is about (a clone's folder).
 */
export type ControlAsk = { control: WindowControl; project: string; names: string };

/** How long an ask waits for its project's Notices view to reach it, before it is let go: a
 *  view that opens later never jumps to it. */
export const ASK_WAITS_MS = 2000;

/** The control asked for, until its project's Notices view has reached it. One per window. */
let asked: ControlAsk | undefined;
let lapse: ReturnType<typeof setTimeout> | undefined;
const listeners = new Set<() => void>();
const told = () => listeners.forEach((listener) => listener());

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => void listeners.delete(listener);
};

/** Asks `project`'s Notices view for `control`, as a message whose words are `names` named it:
 *  the view opens, shows it and puts the keyboard on it. Let go after {@link ASK_WAITS_MS}. */
export function goToControl(control: WindowControl, project: string, names: string): void {
  const ask = { control, project, names };
  asked = ask;
  clearTimeout(lapse);
  lapse = setTimeout(() => controlReached(ask), ASK_WAITS_MS);
  told();
}

/** `ask` is done with: its Notices view reached it, or it waited too long. */
export function controlReached(ask: ControlAsk): void {
  if (asked !== ask) return;
  asked = undefined;
  clearTimeout(lapse);
  told();
}

/** The control asked for of `project`'s Notices view and not reached yet. */
export const useAskedControl = (project: string | undefined): ControlAsk | undefined =>
  useSyncExternalStore(subscribe, () =>
    project !== undefined && asked?.project === project ? asked : undefined,
  );
