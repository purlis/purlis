// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **a switch, on and off, as `e2e/specs/forced-colors.e2e.ts` reads it drawn with
 *  forced colours active** (`forced-colors.css`, #3481). */
import type { ForcedProbe } from "../../theme/forcedProbe.testkit";

export default {
  markup: [
    '<label data-scope="switch" data-part="root" data-selected="true"><span data-p="switch-on" data-scope="switch" data-part="track"><span data-p="thumb-on" data-scope="switch" data-part="thumb"></span></span></label>',
    '<label data-scope="switch" data-part="root"><span data-p="switch-off" data-scope="switch" data-part="track"><span data-p="thumb-off" data-scope="switch" data-part="thumb"></span></span></label>',
  ].join(""),
  forced: {
    // On is filled in Highlight, with its thumb in HighlightText.
    "switch-on": { background: { system: "Highlight" } },
    "thumb-on": { background: { system: "HighlightText" } },
    // Off has a CanvasText thumb, and its track is outlined in the text's colour, on or off, so
    // the pill keeps its shape.
    "switch-off": {
      outlineStyle: "solid",
      outlineWidth: "1px",
      outlineColor: { system: "CanvasText" },
    },
    "thumb-off": { background: { system: "CanvasText" } },
  },
} satisfies ForcedProbe;
