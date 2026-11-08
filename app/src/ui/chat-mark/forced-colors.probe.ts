// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** Test-only: **a chat's mark, as `e2e/specs/forced-colors.e2e.ts` reads it drawn with forced
 *  colours active**: it keeps the system's ground, and its glyph tells its state
 *  (`forced-colors.css`, #3481). */
import type { ForcedProbe } from "../../theme/forcedProbe.testkit";

export default {
  markup: '<span data-p="mark" data-scope="chat-mark" data-part="badge"></span>',
  forced: { mark: { background: { system: "Canvas" } } },
} satisfies ForcedProbe;
