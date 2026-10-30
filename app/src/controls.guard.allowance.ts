// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The control guard's allowance** (`controls.guard.test.ts`, #1837): each button, field,
 * select, table or menu the window drew before it had a class per control, by file, one per
 * line as its tag is written (`<button className="tab-close">`).
 *
 * **This list only shrinks.** When a control moves onto its family, or takes its `ui-` class
 * (`docs/ui-primitives.md`, "Every control is a family's"), delete its line: the guard fails on
 * an entry that no longer names anything, on an unclassed control that is not here, and on a
 * file's list that grows past its ceiling. Never add a line; give the control its family or its
 * class instead. A file whose list empties is deleted from here.
 */
// prettier-ignore
export const CONTROLS_ALLOWANCE: Readonly<Record<string, readonly string[]>> = {
  "StripMarks.tsx": [
    "<button className={clsx(\"ui-tabs-control closer\", { keeps: closesOnly(offer.does), minimise })}>",
  ],
  "settings/RawToml.tsx": [
    "<button className=\"ui-settings-raw-file\">",
  ],
  "settings/components.tsx": [
    "<button className={one.sub ? \"ui-settings-nav-item ui-settings-sub\" : \"ui-settings-nav-item\"}>",
  ],
};
