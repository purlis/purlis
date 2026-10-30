// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { screen, waitFor, within } from "@testing-library/react";

/** The notice of the newest result, at the top of the content (#3344), once it is drawn. */
export function theResult(): HTMLElement {
  const at = document.querySelector<HTMLElement>(".window-work > .action-result");
  if (at === null) throw new Error("no result is drawn at the top of the content");
  return at;
}

/** Where the newest result is said now: its notice, or the palette's line while the palette is
 *  up and draws over it. */
function saying(): HTMLElement {
  const palette = document.querySelector<HTMLElement>("p.said");
  return palette ?? theResult();
}

/** **The newest result's words**, found in its notice at the top of the content (or the
 *  palette's line while the palette is up): the same
 *  result is also listed under Results in the Inbox's Notices (#3344), so a bare `findByText`
 *  would find it twice. */
export function findResult(text: string | RegExp): Promise<HTMLElement> {
  return waitFor(() => within(saying()).getByText(text));
}

/** **A notice on `workspace`'s chip's card** (#3648, #3649) whose words match `text`, once it is
 *  drawn: the whole notice, its title, its words and its ways. */
export function findOnCard(workspace: string, text: string | RegExp): Promise<HTMLElement> {
  return waitFor(() => {
    const card = screen.getByRole("region", { name: `Notices for ${workspace}` });
    const notice = [...card.querySelectorAll<HTMLElement>(".action-result")].find((one) =>
      typeof text === "string" ? one.textContent?.includes(text) : text.test(one.textContent ?? ""),
    );
    if (notice === undefined) throw new Error(`no notice on ${workspace}'s card says ${text}`);
    return notice;
  });
}
