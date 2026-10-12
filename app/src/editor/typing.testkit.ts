// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Typing into a CodeMirror text in a test** (DS-10). jsdom has no layout and no input events
 * CodeMirror reads, so a key typed by user-event never reaches its document. These go through
 * the view the text belongs to instead, as a typed edit (`input.type`, `delete.backward`), so
 * what the editor hands its owner is what typing would hand it. A test finds the text as a
 * person does — `getByRole("textbox", { name })` — and passes it here. Each is inside `act`, so
 * what the owner does with the change is drawn before the test goes on.
 */
import { act } from "@testing-library/react";
import { deleteCharBackward } from "@codemirror/commands";
import { EditorSelection } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

/** The view a CodeMirror text (its `.cm-content`, or anything inside the editor) belongs to. */
export function viewOf(text: Element): EditorView {
  const view = EditorView.findFromDOM(text.closest(".cm-editor") as HTMLElement);
  if (!view) throw new Error("not a CodeMirror text");
  return view;
}

/** What the text holds. */
export function textOf(text: Element): string {
  return viewOf(text).state.doc.toString();
}

/** Whether the text can be typed into. */
export function typeable(text: Element): boolean {
  const view = viewOf(text);
  return !view.state.readOnly && text.getAttribute("contenteditable") === "true";
}

/** Types `what` at the end of the text, as a person would, and focuses it. A read-only text
 *  takes nothing, as it takes no key. */
export function typeIn(text: Element, what: string): void {
  const view = viewOf(text);
  act(() => {
    view.focus();
    if (view.state.readOnly) return;
    const end = view.state.doc.length;
    view.dispatch({
      changes: { from: end, insert: what },
      selection: EditorSelection.cursor(end + what.length),
      userEvent: "input.type",
    });
  });
}

/** Replaces the whole text with `what`, as selecting all and typing over it would. */
export function retype(text: Element, what: string): void {
  const view = viewOf(text);
  act(() => {
    view.focus();
    if (view.state.readOnly) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: what },
      selection: EditorSelection.cursor(what.length),
      userEvent: "input.type",
    });
  });
}

/** Presses Backspace `times` at the end of the text. */
export function backspace(text: Element, times = 1): void {
  const view = viewOf(text);
  act(() => {
    view.focus();
    view.dispatch({ selection: EditorSelection.cursor(view.state.doc.length) });
  });
  for (let at = 0; at < times; at++) act(() => void deleteCharBackward(view));
}
