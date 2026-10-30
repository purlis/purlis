// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Focus mode** (#2921, spec #2897 N32): one key hides the project rail, the activity bars, the
 * side panels, the bottom Panel and the status bar, and shows the work alone; the same key brings
 * them back where they were.
 *
 * - **Nothing is written** (D-2921-9): the arrangement, the rail's mode and the panels' sizes are
 *   untouched, so the layout comes back exactly as it was, and a launch never starts in focus
 *   mode. The window says it with one attribute on its root (`data-focus-mode`) and the
 *   stylesheet takes the parts out of the flow (`App.css`); nothing is unmounted, so a view keeps
 *   its scroll and its folds, and a terminal keeps its rows.
 * - **The title bar stays** (D-2921-10): it holds the window's controls and what needs the person,
 *   and the work is under it.
 * - **Motion only as they come back** (the operator's rule: a part animates only when it appears
 *   or leaves): the parts fade in over `duration.enter` (`data-focus-back`), and go at once, as
 *   every part that closes does (R128). Reduced motion makes the duration nothing.
 * - **The keyboard stays on the work**: a key pressed in a part that is going takes the keyboard
 *   to the work in front, else it would be left on nothing; and so does a surface that gives the
 *   keyboard back into a hidden part once focus mode is on (the palette, #4072).
 */
import { useCallback, useEffect, useRef, useState } from "react";

/** The root's attribute while focus mode is on. */
export const FOCUS_MODE = "data-focus-mode";

/** The root's attribute while the parts focus mode hid are coming back. */
export const FOCUS_BACK = "data-focus-back";

/** How long the parts' return is marked for: longer than any `duration.enter` a theme may set. */
const BACK_FOR_MS = 600;

/** What focus mode hides, as the stylesheet finds it: the work is everything else. */
export const HIDDEN_IN_FOCUS = [
  '[data-scope="rail"][data-part="slot"]',
  ".region-frame > .activity-column",
  ".region-slot.slot-left",
  ".region-slot.slot-right",
  ".region-slot.slot-panel",
  'footer[data-scope="status-bar"]',
  // The card that pops from the status bar's Notices item (`AT_STATUS`, #3653): portaled over
  // the window, no part of the footer, but it goes as the footer goes.
  '[data-chip="status/bar/notices"]',
].join(", ");

/** **The window's focus mode**: whether it is on, whether its parts are coming back, and the
 *  toggle the key and the palette's row press. */
export function useFocusMode() {
  const [on, setOn] = useState(false);
  const [back, setBack] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const onNow = useRef(false);
  const toggle = useCallback(() => {
    const was = onNow.current;
    onNow.current = !was;
    window.clearTimeout(timer.current);
    if (was) {
      setBack(true);
      timer.current = window.setTimeout(() => setBack(false), BACK_FOR_MS);
    } else {
      setBack(false);
      keepTheKeyboardOnTheWork();
    }
    setOn(!was);
  }, []);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  // Once the parts are hidden, a keyboard that reached one of them meanwhile is taken off it:
  // a surface that closed while the toggle was being drawn may have handed it there (#4072).
  useEffect(() => {
    if (on) keepTheKeyboardOnTheWork();
  }, [on]);
  return { on, back, toggle };
}

/**
 * **The keyboard kept off what focus mode hides, wherever it was put** (#4072): while focus mode
 * is on, a keyboard given to an element in a part it hides goes to the work instead. The
 * palette gives the keyboard back to where it was opened from as it closes, after its row ran:
 * Focus mode run from a part it hides would otherwise hand the keyboard to a part no longer
 * drawn, which leaves it on the page.
 */
export function keepTheKeyboardOffWhatFocusModeHides(): void {
  if (document.querySelector(`[${FOCUS_MODE}]`) === null) return;
  keepTheKeyboardOnTheWork();
}

/**
 * The keyboard taken to the work in front when it is in a part focus mode is about to hide: the
 * focused pane of the editor region, its first control the keyboard can reach.
 */
export function keepTheKeyboardOnTheWork(): void {
  const at = document.activeElement;
  if (!(at instanceof Element) || at.closest(HIDDEN_IN_FOCUS) === null) return;
  const work = document.querySelector(".region-centre");
  const to = work?.querySelector<HTMLElement>(
    '[data-chat-keyboard] textarea, [role="tab"][tabindex="0"], button:not([disabled]), [tabindex="0"]',
  );
  to?.focus({ preventScroll: true });
}
