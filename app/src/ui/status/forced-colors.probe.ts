// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **a waiting item's number, which needs you, as `e2e/specs/forced-colors.e2e.ts`
 *  reads it drawn with forced colours active**: Mark and MarkText (`forced-colors.css`, #3481). */
import type { ForcedProbe } from "../../theme/forcedProbe.testkit";

export default {
  markup:
    '<button type="button" data-scope="status-item" data-part="root" data-variant="waiting"><span data-p="needs-number" data-scope="status-item" data-part="count">2</span></button>',
  forced: { "needs-number": { color: { system: "MarkText" }, background: { system: "Mark" } } },
} satisfies ForcedProbe;
