// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **a hot count, which needs you, as `e2e/specs/forced-colors.e2e.ts` reads it drawn
 *  with forced colours active**: Mark and MarkText (`forced-colors.css`, #3481). */
import type { ForcedProbe } from "../../theme/forcedProbe.testkit";

export default {
  markup:
    '<span data-p="needs" data-scope="count" data-part="root" data-variant="hot">2</span>' +
    '<span data-p="round" data-scope="count" data-part="round" data-at="mark" data-drawn="2"></span>',
  forced: {
    needs: { color: { system: "MarkText" }, background: { system: "Mark" } },
    round: { color: { system: "MarkText" }, background: { system: "Mark" } },
  },
} satisfies ForcedProbe;
