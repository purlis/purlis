// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **the window's own forced-colours rules, as `e2e/specs/forced-colors.e2e.ts` reads
 *  them drawn** (`forced-colors.css`, #2328, #3481). */
import type { ForcedProbe } from "./forcedProbe.testkit";

export default {
  markup: [
    '<input data-p="focus" type="text" aria-label="probe">',
    '<div role="tablist"><button data-p="tab" type="button" role="tab" aria-selected="true">tab</button></div>',
    '<div><div data-p="option" role="option" aria-selected="true">option</div></div>',
    '<p data-p="trouble" class="trouble">trouble</p>',
    '<div data-p="dialog" role="dialog" aria-label="probe">dialog</div>',
    '<div data-p="alertdialog" role="alertdialog" aria-label="probe">alert</div>',
  ].join(""),
  focus: "focus",
  forced: {
    // Focus: a solid ring in Highlight wherever Tab lands.
    focus: {
      focusVisible: "true",
      outlineStyle: "solid",
      outlineWidth: "2px",
      outlineColor: { system: "Highlight" },
    },
    // What is selected or current: the system's pair, and a tab is underlined.
    tab: {
      color: { system: "SelectedItemText" },
      background: { system: "SelectedItem" },
      decoration: "underline",
    },
    option: { color: { system: "SelectedItemText" }, background: { system: "SelectedItem" } },
    // A ground that says something is wrong: a dashed outline inside its edge.
    trouble: {
      outlineStyle: "dashed",
      outlineWidth: "2px",
      outlineColor: { system: "CanvasText" },
      outlineOffset: "-2px",
    },
    // What is opened over the window: outlined, in place of its shadow.
    dialog: { outlineStyle: "solid", outlineWidth: "1px", outlineColor: { system: "CanvasText" } },
    alertdialog: {
      outlineStyle: "solid",
      outlineWidth: "1px",
      outlineColor: { system: "CanvasText" },
    },
  },
  unforced: {
    dialog: { outlineStyle: "solid" },
    focus: { outlineColor: { system: "Highlight" } },
  },
} satisfies ForcedProbe;
