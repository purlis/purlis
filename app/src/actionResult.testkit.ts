// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { waitFor, within } from "@testing-library/react";

/** The notice of the newest result, at the top of the content (#3344), once it is drawn. */
export function theResult(): HTMLElement {
  const at = document.querySelector<HTMLElement>(".window-work > .action-result");
  if (at === null) throw new Error("no result is drawn at the top of the content");
  return at;
}

/** **The newest result's words**, found in its notice at the top of the content: the same
 *  result is also listed under Results in the Inbox's Notices (#3344), so a bare `findByText`
 *  would find it twice. */
export function findResult(text: string | RegExp): Promise<HTMLElement> {
  return waitFor(() => within(theResult()).getByText(text));
}
