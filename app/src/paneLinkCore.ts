// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import type { LinkCore } from "./paneLinks";

/** **The window's link core** (#6215): the two commands a pane's links are decided and opened
 *  by, handed to each pane's link hook (`paneLinks.ts`). */
export const LINK_CORE: LinkCore = {
  resolve: (from, candidates) => commands.paneLinkResolve(from, candidates),
  open: (from, candidate, token) => commands.paneLinkOpen(from, candidate, token),
};
