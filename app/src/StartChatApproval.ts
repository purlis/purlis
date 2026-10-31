// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Start a chat's main answer: a start, or the approval of a profile's command** (ADR 0022, ADR
 * 0097 §6). A profile purlis has not run before asks for its command to be approved, and the
 * dialog's main answer is then that approval: a grant, drawn as every grant is and never the
 * primary look, never found by Return and never sent by ⌘⏎. Kept in a file of its own, named
 * for the approval, so the profile icons the dialog lists (`HarnessIcon`, #2922) can never be
 * drawn beside a grant (`harnessIcon.guard.test.ts`).
 */
import type { DialogAction } from "./ui/dialog/Dialog";
import { submitKeySaid, submitKeyShortcut } from "./ui/field/submitKey";

/** What a start says, by itself and without the sandbox (#2918, R33). */
export const START_WORD = "Start chat";
export const START_WITHOUT_SANDBOX = "Start without the sandbox";

/**
 * **The words this dialog's main answer can reach**, and only those: their room is kept for the
 * longest, so a choice moves and widens nothing (R163), and a dialog that can say none of the long
 * ones is not drawn as wide as them (#2918). The options are fixed while the dialog is open.
 */
export function reachableWords({
  canApprove,
  canOptOut,
}: {
  /** Some profile listed asks for its command to be approved. */
  canApprove: boolean;
  /** Some profile listed can start without the sandbox: it is on, and policy allows the opt-out. */
  canOptOut: boolean;
}): string[] {
  return [
    START_WORD,
    "Starting…",
    ...(canOptOut ? [START_WITHOUT_SANDBOX] : []),
    ...(canApprove ? ["Approve and start"] : []),
    ...(canApprove && canOptOut ? ["Approve and start without the sandbox"] : []),
  ];
}

/** What the main answer needs to know of the dialog. */
export type StartAnswer = {
  /** The profile picked asks for its command to be approved. */
  approves: boolean;
  /** The chat starts without the sandbox. */
  withoutSandbox: boolean;
  starting: boolean;
  /** It cannot be pressed now: nothing picked, or the project's policy refuses it. */
  unavailable: boolean;
  /** The words it can reach in this dialog ({@link reachableWords}), and whether one is a grant. */
  reach: { words: readonly string[]; grant: boolean };
  onPress: () => void;
};

/**
 * **The dialog's main answer.** An approval grants the command its run, so it is the grant's
 * look and only ever a press of its own button: neither Return nor the chord finds it. A start
 * is the suggested answer, sent by ⌘⏎ (N26, Ctrl+Enter off macOS) unless it starts without the
 * sandbox, which grants the chat more than the sandbox gives.
 */
export function startAnswer({
  approves,
  withoutSandbox,
  starting,
  unavailable,
  reach,
  onPress,
}: StartAnswer): DialogAction {
  const chord = !approves && !withoutSandbox;
  const said = withoutSandbox ? START_WITHOUT_SANDBOX : START_WORD;
  return {
    children: starting
      ? "Starting…"
      : approves
        ? withoutSandbox
          ? "Approve and start without the sandbox"
          : "Approve and start"
        : said,
    // Every word it can reach here, and the lock's room where it can be a grant, whatever is
    // chosen: choosing a profile that asks for approval moves and widens nothing (R163).
    reserve: reach.words,
    reserveGrant: reach.grant,
    variant: approves ? "grants" : "primary",
    isDisabled: unavailable || starting,
    // Return in the Name box never starts a command: Start is pressed (D-1719-3). The command is
    // said above before the press (V69), and the chat it starts can be closed.
    byReturn: false,
    byChord: chord,
    "aria-keyshortcuts": chord && !starting ? submitKeyShortcut() : undefined,
    // The key, said inside the button (R33): its room is kept, undrawn, while the key does not
    // press it, so turning to an approval or a start without the sandbox widens nothing (R163).
    // Nor while the chat starts: the chord does nothing then, so "Starting…" names no key.
    shortcut: submitKeySaid(),
    shortcutOff: !chord || starting,
    onPress,
  };
}
