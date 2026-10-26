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
import { submitKeyShortcut } from "./ui/field/submitKey";

/** Every word the main answer can say: its room is kept for the longest (R163). */
const SAID = [
  "Start",
  "Start without the sandbox",
  "Starting…",
  "Approve and start",
  "Approve and start without the sandbox",
];

/** What the main answer needs to know of the dialog. */
export type StartAnswer = {
  /** The profile picked asks for its command to be approved. */
  approves: boolean;
  /** The chat starts without the sandbox. */
  withoutSandbox: boolean;
  starting: boolean;
  /** What a start says: Start, or Start without the sandbox. */
  startWord: string;
  /** It cannot be pressed now: nothing picked, or the project's policy refuses it. */
  unavailable: boolean;
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
  startWord,
  unavailable,
  onPress,
}: StartAnswer): DialogAction {
  const chord = !approves && !withoutSandbox;
  return {
    children: starting
      ? "Starting…"
      : approves
        ? `Approve and ${startWord.toLowerCase()}`
        : startWord,
    // Every word it can say, and the lock's room, whatever is chosen: choosing a profile that
    // asks for approval moves and widens nothing (R163).
    reserve: SAID,
    reserveGrant: true,
    variant: approves ? "grants" : "primary",
    isDisabled: unavailable || starting,
    // Return in the Name box never starts a command: Start is pressed (D-1719-3). The command is
    // said above before the press (V69), and the chat it starts can be closed.
    byReturn: false,
    byChord: chord,
    "aria-keyshortcuts": chord ? submitKeyShortcut() : undefined,
    onPress,
  };
}
