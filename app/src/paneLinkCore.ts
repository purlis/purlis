// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import { invisible } from "./invisible";
import type { LinkCore } from "./paneLinks";

/** **The window's link core** (#6215): the two commands a pane's links are decided and opened
 *  by, handed to each pane's link hook (`paneLinks.ts`). */
export const LINK_CORE: LinkCore = {
  resolve: (from, candidates) => commands.paneLinkResolve(from, candidates),
  open: (from, candidate, token) => commands.paneLinkOpen(from, candidate, token),
  // As the core's `drawn` reads a link's words: a control, a line or paragraph separator, or an
  // invisible character that is no blank (Unicode's White_Space, as Rust reads it: JavaScript's
  // `\s` also takes U+FEFF, which draws nothing) draws nothing.
  drawsNothing: (ch) =>
    /^[\p{Cc}\u2028\u2029]$/u.test(ch) || (invisible(ch) && !/^\p{White_Space}$/u.test(ch)),
};
