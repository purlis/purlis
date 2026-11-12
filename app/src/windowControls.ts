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
 * the Notices view opens, filtered to that kind, with the keyboard on the control.
 *
 * `windowControls.test.ts` holds the list to the core: every core message that tells the person
 * to press a control on a Notice names one listed here.
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

/** The control asked for, until the Notices view has put the keyboard on it. One per window. */
let asked: WindowControl | undefined;
const listeners = new Set<() => void>();
const told = () => listeners.forEach((listener) => listener());

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => void listeners.delete(listener);
};

/** Asks the window for `control`: the Notices view shows it and puts the keyboard on it. */
export function goToControl(control: WindowControl): void {
  asked = control;
  told();
}

/** The Notices view has put the keyboard on the control asked for, or found none to put it on. */
export function controlReached(): void {
  if (asked === undefined) return;
  asked = undefined;
  told();
}

/** The control asked for and not reached yet, for what draws it or brings its view. */
export const useAskedControl = (): WindowControl | undefined =>
  useSyncExternalStore(subscribe, () => asked);
