// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { AT_STATUS } from "./ChipNotices";

/**
 * **Opens the card behind `place`'s dot, as a person does** (#6004): no card comes by itself, so
 * a test that reads one presses the dot first: `place` is a chip's workspace, or `AT_STATUS`.
 * Does nothing where no such dot is drawn (its card up already, or nothing on it yet).
 */
export function pressTheDot(place: string): void {
  const dot = document.querySelector<HTMLElement>(
    place === AT_STATUS
      ? '[data-scope="status-item"][data-part^="dot"]'
      : `[data-scope="titlebar"][data-dot-of="${CSS.escape(place)}"]`,
  );
  if (dot !== null) fireEvent.click(dot);
}

/** The notice of the newest result once it is drawn: on the status bar's card while a project is
 *  in front (#3653), opened by its dot's press as a person does (#6004), else at the top of the
 *  opener's page (#3344). */
export function theResult(): HTMLElement {
  if (document.querySelector(`[data-chip="${AT_STATUS}"]`) === null) pressTheDot(AT_STATUS);
  const at =
    document.querySelector<HTMLElement>(`[data-chip="${AT_STATUS}"] > .action-result`) ??
    document.querySelector<HTMLElement>(".window-work > .action-result");
  if (at === null) throw new Error("no result is drawn on the status bar's card or the page");
  return at;
}

/** Where the newest result is said now: its notice, or the palette's line while the palette is
 *  up and draws over it. */
function saying(): HTMLElement {
  const palette = document.querySelector<HTMLElement>("p.said");
  return palette ?? theResult();
}

/** **The newest result's words**, found in its notice (or the
 *  palette's line while the palette is up): the same
 *  result is also listed under Results in the Inbox's Notices (#3344), so a bare `findByText`
 *  would find it twice. */
export function findResult(text: string | RegExp): Promise<HTMLElement> {
  return waitFor(() => within(saying()).getByText(text));
}

/** **`workspace`'s chip's card, opened by its dot's press** as a person opens it (#6004): the card
 *  up already, or the one its dot shows once the dot is drawn. */
export function openChipCard(workspace: string): Promise<HTMLElement> {
  return waitFor(() => {
    if (screen.queryByRole("region", { name: `Notices for ${workspace}` }) === null)
      pressTheDot(workspace);
    return screen.getByRole("region", { name: `Notices for ${workspace}` });
  });
}

/** **A notice on `workspace`'s chip's card** (#3648, #3649) whose words match `text`, once it is
 *  drawn: the whole notice, its title, its words and its ways. */
export function findOnCard(workspace: string, text: string | RegExp): Promise<HTMLElement> {
  return waitFor(() => {
    if (screen.queryByRole("region", { name: `Notices for ${workspace}` }) === null)
      pressTheDot(workspace);
    const card = screen.getByRole("region", { name: `Notices for ${workspace}` });
    const notice = [...card.querySelectorAll<HTMLElement>(".action-result")].find((one) =>
      typeof text === "string" ? one.textContent?.includes(text) : text.test(one.textContent ?? ""),
    );
    if (notice === undefined) throw new Error(`no notice on ${workspace}'s card says ${text}`);
    return notice;
  });
}
