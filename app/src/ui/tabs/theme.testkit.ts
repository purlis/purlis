// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **what the tabs family says of its own rules to the theme's tests**
 *  (`theme/families.testkit.ts`, #3387). */
import type { FamilyTheme } from "../../theme/families.testkit";

export default {
  faint: [
    // A session tab's `×`, a mark (`ui/tabs/tabs.css`, #2914).
    '[data-scope="tabs"][data-part="tab"] [data-part="closer"][data-scope="tabs"]',
  ],
  spreadGrounds: [
    // The chat strip (`tabsPart("strip")`): none docked, the chat strip's shade on its own. It is
    // the engine's too, under `tabs.layout.attach: strip` (`attached` in the core's parts).
    ["bar", ["layer.chat"]],
    // The editor's toolbar over a file (`editorToolbar`): the code's own surface.
    ["piece-files-head", ["terminal.background"]],
  ],
  classless: [
    // The editor's toolbar (#2914): its attributes are spread by `editorToolbar`, and its ground
    // is listed by class in `spreadGrounds`.
    '[data-scope="editor"][data-part="toolbar"]',
    // A session tab's `×` under the pointer (`tabs.css`, #2914): its attributes are the family's
    // (`tabPart`), and it holds no button.
    '[data-scope="tabs"][data-part="tab"] [data-part="closer"][data-scope="tabs"]:hover',
    '[data-scope="tabs"][data-part="tab"] [data-part="closer"][data-scope="tabs"][data-keeps]:hover',
  ],
} satisfies FamilyTheme;
