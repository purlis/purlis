// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **a count that needs you, as `e2e/specs/forced-colors.e2e.ts` reads it drawn with
 *  forced colours active**: Mark and MarkText (`forced-colors.css`, #3481). */
import type { ForcedProbe } from "../../theme/forcedProbe.testkit";

export default {
  markup:
    '<span data-p="needs-activity" data-scope="activity" data-part="count" data-tone="needs-you">2</span>',
  forced: { "needs-activity": { color: { system: "MarkText" }, background: { system: "Mark" } } },
} satisfies ForcedProbe;
