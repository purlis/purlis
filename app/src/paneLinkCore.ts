// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import type { LinkCore } from "./paneLinks";

/** **The window's link core** (#6215): the two commands a pane's links are decided and opened
 *  by, handed to each pane's link hook (`paneLinks.ts`). A command the core could not answer
 *  is a rejection: the hook asks again on the next hover, and an open says so in a sentence. */
export const LINK_CORE: LinkCore = {
  resolve: (from, candidates) => commands.paneLinkResolve(from, candidates),
  open: (from, candidate, token) => commands.paneLinkOpen(from, candidate, token),
};

/** The answer a command gave, or its refusal as a rejection. */
function answered<T>(said: { status: "ok"; data: T } | { status: "error"; error: string }): T {
  if (said.status === "error") throw new Error(said.error);
  return said.data;
}
