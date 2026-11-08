// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **a button that ends or destroys, as `e2e/specs/forced-colors.e2e.ts` reads it drawn
 *  with forced colours active**: a dashed outline inside its edge (`forced-colors.css`, #3481). */
import type { ForcedProbe } from "../../theme/forcedProbe.testkit";

export default {
  markup:
    '<button data-p="danger" type="button" data-scope="button" data-part="root" data-variant="danger" data-size="md">end</button>',
  forced: {
    danger: {
      outlineStyle: "dashed",
      outlineWidth: "2px",
      outlineColor: { system: "CanvasText" },
      outlineOffset: "-2px",
    },
  },
  unforced: { danger: { outlineStyle: "dashed" } },
} satisfies ForcedProbe;
