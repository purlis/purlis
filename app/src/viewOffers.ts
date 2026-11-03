// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ExtensionView } from "./bindings";
import type { ViewRef } from "./tabs";

/**
 * **An extension's offer of its view** (#4769; #4703 rule 2; prototype `view-open-beside`,
 * accepted on #4705). Without a press an extension never opens anything: an extension that heard
 * an event may only offer one of its own declared views (the core refuses any other,
 * `executor::offer_of`), and the window holds it as one quiet note, on the pane band of the chat
 * it is about while that chat is on screen and always in the Notices centre. It takes no pane,
 * tab or focus, and the extension's program is asked nothing until the person presses Open or
 * Open beside.
 *
 * The rules, on plain values, so they are held in one table (`viewOffers.test.ts`):
 * - **One note per view and subject.** A second offer while one stands, or while the view is
 *   open, adds nothing.
 * - **Dismiss keeps that view and subject quiet until relaunch** (Q3 A): the quiet set lasts the
 *   session, so a new launch starts with none.
 * - **Stop offers from <extension>** holds every offer from it until Settings › Extensions turns
 *   them back on: the person's preference (`offersPref.ts`), no new capability (ADR 0041).
 * - **Opening the view by any route clears its offer.**
 * - **A removed extension's offers are withdrawn without a word** (Q4 A), and its open view says
 *   it is not here any more (`Views.tsx`).
 */

/** One view an extension offered: which, about what, and about which chat, when it said. */
export type ViewOffer = {
  extension: string;
  view: string;
  /** A persona's name, or empty for the whole plane's view. */
  key: string;
  /** The chat it is about, by its number. */
  chat?: number;
};

/** The offers standing, and the views and subjects dismissed this session. */
export type OfferState = {
  standing: readonly ViewOffer[];
  /** By {@link offerKey}: what Dismiss quieted, until relaunch. */
  quiet: ReadonlySet<string>;
};

/** A launch's: nothing offered, nothing quiet. */
export const NO_OFFERS: OfferState = { standing: [], quiet: new Set() };

/** What an offer is judged against that the offers alone do not say. */
export type OfferWorld = {
  /** The views approved extensions offer this window and are on here. */
  declared: readonly ExtensionView[];
  /** The extensions whose offers the person stopped. */
  off: readonly string[];
  /** Whether some pane shows the view already. */
  open: (view: ViewRef) => boolean;
};

/** Which view and subject an offer is about: one note each. */
export function offerKey(offer: Pick<ViewOffer, "extension" | "view" | "key">): string {
  return `${offer.extension}\u0000${offer.view}\u0000${offer.key}`;
}

/** The view an offer opens. */
export function viewOf(offer: ViewOffer): ViewRef {
  return { from: offer.extension, view: offer.view, key: offer.key };
}

function declares(declared: readonly ExtensionView[], offer: ViewOffer): ExtensionView | undefined {
  return declared.find((one) => one.extension === offer.extension && one.id === offer.view);
}

/** **`offer` heard**: one more note, or `state` itself where it adds nothing. */
export function offered(state: OfferState, offer: ViewOffer, world: OfferWorld): OfferState {
  const key = offerKey(offer);
  if (
    declares(world.declared, offer) === undefined ||
    world.off.includes(offer.extension) ||
    state.quiet.has(key) ||
    state.standing.some((one) => offerKey(one) === key) ||
    world.open(viewOf(offer))
  )
    return state;
  return { ...state, standing: [...state.standing, offer] };
}

/** **Dismissed**: the note goes, and that view and subject stay quiet until relaunch. */
export function dismissed(state: OfferState, offer: ViewOffer): OfferState {
  const key = offerKey(offer);
  return {
    standing: state.standing.filter((one) => offerKey(one) !== key),
    quiet: new Set([...state.quiet, key]),
  };
}

/** **Stop offers from `extension`**: every note of its goes. The preference that holds the
 *  next ones is the caller's to write. */
export function stopped(state: OfferState, extension: string): OfferState {
  const standing = state.standing.filter((one) => one.extension !== extension);
  return standing.length === state.standing.length ? state : { ...state, standing };
}

/** **`view` opened, by any route**: its offer goes, and nothing is quieted. */
export function opened(state: OfferState, view: ViewRef): OfferState {
  if (view.from === null) return state;
  const key = offerKey({ extension: view.from, view: view.view, key: view.key });
  const standing = state.standing.filter((one) => offerKey(one) !== key);
  return standing.length === state.standing.length ? state : { ...state, standing };
}

/** **What is in force now is `declared`**: an offer whose view no extension offers any more
 *  goes, without a word. */
export function withdrawn(state: OfferState, declared: readonly ExtensionView[]): OfferState {
  const standing = state.standing.filter((one) => declares(declared, one) !== undefined);
  return standing.length === state.standing.length ? state : { ...state, standing };
}

/** The view's name as its tab says it: its title, and the subject after a dot. */
export function offerTitle(offer: ViewOffer, declared: readonly ExtensionView[]): string {
  const title = declares(declared, offer)?.title ?? offer.view;
  return offer.key === "" ? title : `${title} · ${offer.key}`;
}

/** **What the note says**: who offers which view, about what. */
export function offerSaid(offer: ViewOffer, declared: readonly ExtensionView[]): string {
  return `${offer.extension} offers a view · ${offerTitle(offer, declared)}`;
}
