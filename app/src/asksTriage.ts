// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { OTHER_SCOPES } from "./AskDecision";

/**
 * **Keyboard triage** (ADR 0121 decision 9, #6016; amended 2026-11-10): ⌘⇧I (Ctrl+Shift+I)
 * opens Asks with the first ask's lead focused, answering one moves the keyboard to the next
 * ask's, there are keys for later, go to chat, snooze and stop, and when Asks is empty or the
 * person leaves it, the keyboard goes back to where it was before ⌘⇧I.
 *
 * **Within the scope in view** (ADR 0121, amended 2026-11-12, decision 8, #6346): Asks opens on
 * Chat, and triage walks only the asks the scope picked lists, round to the first past the last;
 * it never goes on into another scope or another project, whose own Asks answers its asks.
 *
 * What is decided here is where the keyboard goes, read off the rows the list family drew
 * (#3123, ruling B: grid rows, ↑ and ↓ between rows, ← and → through a row's buttons). Nothing
 * here presses an answer: a key presses the row's own button, so the settle guard (ADR 0097,
 * #1695) decides a key press exactly as it decides a click.
 */

/** The rows of Asks the keyboard can land on, in their order: a row of the list family. */
const ROWS = '[role="row"][data-key]';

/** **What marks a high-risk ask's row** (ADR 0121 decision 8, #6011): triage never lands on it. */
export const HIGH_RISK = "data-risk";

/**
 * **Where the keyboard lands in a row** (the lead), in this order:
 *
 * 1. its **Answer**, the button that opens what answers it (`ChatActions`), never a grant; or, on
 *    a group of identical requests (#6009, `SameAskRow`), its **Answer each**, which opens the
 *    group's chats, each a row triage then walks, with Allow for all one ← away;
 * 2. a decision's **primary** answer, the one its source suggests (#2241);
 * 3. a decision's first answer that grants nothing (Deny, Keep blocked): **a grant never takes
 *    the keyboard as it is drawn** (#2980, `grantFocus.guard.test.ts`), and a row the keyboard
 *    was moved onto is one drawn to it. Allow stays one → away, pressed on purpose. Nor is it
 *    **Other scopes…**, which answers nothing: it only opens more grants;
 * 4. its **Go to chat**: a row answered in its chat;
 * 5. the row itself.
 */
export function leadOf(row: HTMLElement): HTMLElement {
  const safe = ':not([data-variant="grants"])';
  return (
    row.querySelector<HTMLElement>(`:is([data-lead], [data-triage-lead])${safe}`) ??
    row.querySelector<HTMLElement>('[data-way="fix"][data-variant="primary"]') ??
    [...row.querySelectorAll<HTMLElement>(`[data-way="fix"]${safe}`)].find(
      (one) => one.textContent?.trim() !== OTHER_SCOPES,
    ) ??
    row.querySelector<HTMLElement>(".inbox-go") ??
    row
  );
}

/** The rows of the list at `list` triage can land on, in order: every row but a high-risk ask's. */
export function triageRows(list: ParentNode): HTMLElement[] {
  return [...list.querySelectorAll<HTMLElement>(ROWS)].filter(
    (row) => !row.hasAttribute(HIGH_RISK),
  );
}

/** The row the element `at` stands in, where it is one of {@link triageRows}'s. */
export function rowOf(at: Element | null): HTMLElement | null {
  return at?.closest<HTMLElement>(ROWS) ?? null;
}

/**
 * **The row after `row`** for later (skip), from the first again past the last, so an ask put off
 * comes round once the rest are answered. `undefined` where nothing else can be landed on.
 */
export function nextRow(list: ParentNode, row: HTMLElement | null): HTMLElement | undefined {
  const rows = triageRows(list);
  const at = row === null ? -1 : rows.indexOf(row);
  const next = rows[(at + 1) % Math.max(rows.length, 1)];
  return next === row ? undefined : next;
}

/** **The first ask's lead in the Asks at `list`**: what ⌘⇧I gives the keyboard. None where it
 *  lists no row triage can land on. */
export function firstLead(list: ParentNode | null): HTMLElement | null {
  const first = list === null ? undefined : triageRows(list)[0];
  return first === undefined ? null : leadOf(first);
}

/** Gives the first ask's lead the keyboard, and answers whether there was one. */
export function landOnFirst(list: ParentNode | null): boolean {
  const lead = firstLead(list);
  lead?.focus();
  return lead !== null;
}

// --- Where the keyboard was --------------------------------------------------------------------

/** What marks the Asks view's list, which triage stays in (`Inbox.tsx`). */
export const ASKS_LIST = '[data-view="asks"] .inbox-list';

/**
 * **Triage, from ⌘⇧I until the person leaves Asks**: on, and the element the keyboard was on when
 * it began. One for the window, as the keyboard is.
 */
let triage: { from: HTMLElement | null } | undefined;

/**
 * **⌘⇧I begins triage** from `at`, where the keyboard is. Pressed again in Asks, it keeps where
 * it began, so leaving still goes back there.
 */
export function beginTriage(at: Element | null): void {
  const inAsks = at instanceof Element && at.closest(ASKS_LIST) !== null;
  if (triage !== undefined && inAsks) return;
  triage = { from: at instanceof HTMLElement && at !== document.body && !inAsks ? at : null };
}

/** Whether triage is on. */
export function triaging(): boolean {
  return triage !== undefined;
}

/** Triage ends, and forgets where it began: the person went somewhere of their own. */
export function endTriage(): void {
  triage = undefined;
}

/**
 * **Triage ends, and the keyboard goes back where it was before ⌘⇧I.** Answers whether it could:
 * an element taken away since, or one now hidden, can take nothing.
 */
export function returnFromTriage(): boolean {
  const from = triage?.from ?? null;
  triage = undefined;
  if (from === null || !from.isConnected) return false;
  from.focus();
  return document.activeElement === from;
}

// --- Its keys ----------------------------------------------------------------------------------

/** The acts triage has a key for, beside Enter on the lead, which answers. */
export type TriageKey = "later" | "go" | "snooze" | "stop";

/** Each act's letter, read by its place on the keyboard (⌥ turns a Mac's letter into another). */
export const TRIAGE_LETTER: Readonly<Record<TriageKey, string>> = {
  later: "L",
  go: "G",
  snooze: "S",
  stop: "X",
};

/**
 * **Which triage key this is** (`asks.*` in the key registry): ⌘⌥ and a letter on a Mac, Alt with
 * Shift and the letter elsewhere, as the finder's own keys on its aimed row are (⌘⌥C, Alt+Shift+C).
 * **Never a letter alone** (I-11): a stray keystroke in Asks does nothing.
 */
export function triageKeyOf(e: KeyboardEvent, mac: boolean): TriageKey | undefined {
  const held = mac
    ? e.metaKey && e.altKey && !e.ctrlKey && !e.shiftKey
    : e.altKey && e.shiftKey && !e.ctrlKey && !e.metaKey;
  if (!held) return undefined;
  return (Object.keys(TRIAGE_LETTER) as TriageKey[]).find(
    (one) => `Key${TRIAGE_LETTER[one]}` === e.code,
  );
}

/** How a triage key is spelled. */
export function triageKeySaid(key: TriageKey, mac: boolean): string {
  const letter = TRIAGE_LETTER[key];
  return mac ? `⌘⌥${letter}` : `Alt+Shift+${letter}`;
}

/** What each key's button is named by, on the row it is pressed on (`ChatActions`). */
const BUTTONS: Readonly<Record<Exclude<TriageKey, "later">, string>> = {
  go: '.inbox-go, [aria-label^="Go to chat "]',
  snooze: '[aria-label^="Snooze "]',
  stop: '[aria-label^="Stop "]',
};

/** **The row's own button for `key`**, which the key presses: none where the row has none. */
export function buttonFor(row: HTMLElement, key: Exclude<TriageKey, "later">): HTMLElement | null {
  return row.querySelector<HTMLElement>(BUTTONS[key]);
}
