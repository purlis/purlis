// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where a keystroke landed, and what is up when it does** (#2921, spec #2897 N24): the
 * questions the key registry's context rules ask (`registry.ts`), each answered once here, so a
 * key declared as yielding to a modal, a chat's terminal or an editor yields the same way
 * wherever its handler is.
 *
 * Every question is about the event or the page, never about a component's state: what a
 * component knows (the palette is up) is handed in by its handler (`Held`).
 */
import { CHAT_KEYBOARD, RENAMES_ON_F2 } from "../actions";

/**
 * **What a modal dialog is drawn as** (the operator's ruling of 2026-10-24): the dialog family's
 * own root, or an alert dialog. React Aria marks no dialog `aria-modal`, so the family's root is
 * what is looked for. A popover, a menu and a tooltip are not modal.
 */
export const MODAL = '[data-scope="dialog"][data-part="root"], [role="alertdialog"]';

/** Whether a modal dialog is up anywhere in the window: shortcuts are not taken from under it. */
export function aModalIsUp(): boolean {
  return typeof document !== "undefined" && document.querySelector(MODAL) !== null;
}

/**
 * Whether a dialog has the keyboard: a question the person is answering keeps it, whatever a
 * pane behind it is drawn as. Any dialog, a popover's included: the focus is in it.
 */
export function inADialog(): boolean {
  const at = typeof document === "undefined" ? null : document.activeElement;
  return at instanceof Element && at.closest('[role="dialog"], [role="alertdialog"]') !== null;
}

/**
 * Whether the keystroke landed in a chat's terminal: xterm's own textarea, inside the holder
 * marked {@link CHAT_KEYBOARD}. **Where it landed, not where the focus is**: a capture listener
 * on the window runs before the focus has any say, and an undispatched event has no target, so
 * it is nobody's.
 */
export function inAChat(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(`[${CHAT_KEYBOARD}]`) !== null;
}

/**
 * Whether the keystroke landed where the person types text that is not a chat's: a field, a
 * text box or an editor. A terminal reads from a text box of its own, inside a pane that says the
 * chat has the keyboard there, and is not one of these.
 */
export function typedInto(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.closest(`[${CHAT_KEYBOARD}]`) !== null) return false;
  return target.isContentEditable || target.tagName === "INPUT" || target.tagName === "TEXTAREA";
}

/**
 * **What a text editor's holder is marked with** (`editor/LightEditor.tsx`, `TextEditor.tsx`): the
 * editor's own mark, not its engine's class, so this module (in the window's main bundle) names
 * nothing of the editor, which loads lazily (`tools/main-chunk-has-no-codemirror.mjs`).
 */
export const EDITOR_HOLDER = "data-editor";

/** Whether the keystroke landed in a text editor, whose own keymap has it first. */
export function inAnEditor(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(`[${EDITOR_HOLDER}]`) !== null;
}

/** Whether the keystroke landed on a chat's tab, where `F2` renames it (charter-app#254). */
export function onARenamingTab(target: EventTarget | null): boolean {
  return target instanceof Element && target.hasAttribute(RENAMES_ON_F2);
}

/** Whether the keystroke landed in an open menu, whose own keys it is. */
export function inAMenu(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[role="menu"]') !== null;
}
