// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The control guard's allowance** (`controls.guard.test.ts`, #1837): each button, field,
 * select, table or menu the window drew before it had a class per control, by file, one per
 * line as its tag is written (`<button className="tab-close">`).
 *
 * **This list only shrinks.** When a control takes its `ui-` class (`docs/ui-primitives.md`,
 * "The control classes"), delete its line: the guard fails on an entry that no longer names
 * anything, on an unclassed control that is not here, and on a file's list that grows past its
 * ceiling. Never add a line; give the control its class instead. A file whose list empties is
 * deleted from here.
 */
// prettier-ignore
export const CONTROLS_ALLOWANCE: Readonly<Record<string, readonly string[]>> = {
  "ActivityTab.tsx": [
    "<button className=\"vault-secret\">",
  ],
  "Brief.tsx": [
    "<button className=\"pane-brief\">",
  ],
  "ChatStrip.tsx": [
    "<button>",
    "<button>",
  ],
  "ChatsSection.tsx": [
    "<button className=\"needs-you-mark rolled-up\">",
  ],
  "HarnessCard.tsx": [
    "<button className=\"pane-harness\">",
  ],
  "LayoutPanes.tsx": [
    "<button className={offer.id === \"pane.close\" && !closesOnly(offer.does) ? \"ends-a-chat\" : undefined}>",
  ],
  "PaneCrumbs.tsx": [
    "<button className={at === 0 ? \"crumb own\" : \"crumb between\"}>",
  ],
  "PlaneUpdated.tsx": [
    "<button className=\"plane-updated fresh-mark\">",
  ],
  "StripMarks.tsx": [
    "<button className={clsx( offer.id === \"pane.close\" && !closesOnly(offer.does) && \"ends-a-chat\", bare && \"bare\", )}>",
    "<button className={arrived ? \"show-more arrived\" : \"show-more\"}>",
    "<button className={clsx(\"closer\", closesOnly(offer.does) && \"keeps\", minimise && \"minimise\")}>",
    "<button className=\"gear\">",
  ],
  "TabChip.tsx": [
    "<button className=\"tab-tasks-hand needs-you-mark\">",
    "<button className=\"tab-tasks-counts\">",
    "<button className=\"tasks-menu-place\">",
  ],
  "TaskEnd.tsx": [
    "<button className=\"task-end\">",
  ],
  "Welcome.tsx": [
    "<button className=\"welcome-door\">",
  ],
  "YourRepos.tsx": [
    "<button className=\"welcome-link repo-card\">",
  ],
  "settings/RawToml.tsx": [
    "<button className=\"ui-settings-raw-file\">",
  ],
  "settings/SettingsTab.tsx": [
    "<input>",
  ],
  "settings/components.tsx": [
    "<button className={one.sub ? \"ui-settings-nav-item ui-settings-sub\" : \"ui-settings-nav-item\"}>",
    "<input>",
    "<input type=\"checkbox\" className=\"m-0\">",
  ],
};
