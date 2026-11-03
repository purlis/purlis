// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import type { ExtensionView, PlaneId } from "./bindings";
import { offersFrom, OFFERS } from "./offersPref";
import type { ViewRef } from "./tabs";
import {
  dismissed,
  NO_OFFERS,
  type OfferState,
  type OfferWorld,
  offered,
  opened,
  stopped,
  type ViewOffer,
  withdrawn,
} from "./viewOffers";

/**
 * **The offers standing in each project this window holds** (#4769), for this launch: the rules
 * are `viewOffers.ts`'s, and this only keeps their answer per project and tells whoever draws
 * it. It lasts the session, so a relaunch starts with nothing offered and nothing quiet.
 */

const byPlane = new Map<PlaneId, OfferState>();
const listeners = new Set<() => void>();

function tell() {
  for (const listener of listeners) listener();
}

function change(plane: PlaneId, how: (state: OfferState) => OfferState) {
  const was = byPlane.get(plane) ?? NO_OFFERS;
  const now = how(was);
  if (now === was) return;
  byPlane.set(plane, now);
  tell();
}

/** **Offers heard** from the extensions that heard an event in `plane`. `world` is read once,
 *  as it is now; the person's stopped extensions are read from their preference. */
export function heardOffers(
  plane: PlaneId,
  offers: readonly ViewOffer[],
  world: Omit<OfferWorld, "off">,
): void {
  const off = OFFERS.value();
  change(plane, (state) =>
    offers.reduce((held, offer) => offered(held, offer, { ...world, off }), state),
  );
}

/** **Dismiss**: the note goes, and its view and subject stay quiet until relaunch. */
export function dismissOffer(plane: PlaneId, offer: ViewOffer): void {
  change(plane, (state) => dismissed(state, offer));
}

/** **Stop offers from `extension`**: every note of its goes from every project, and the
 *  preference holds the next ones until Settings › Extensions turns them back on. */
export function stopOffers(extension: string): void {
  offersFrom(extension, false);
  for (const plane of [...byPlane.keys()]) change(plane, (state) => stopped(state, extension));
}

/** **`view` opened in `plane`, by any route**: its offer goes. */
export function viewOpened(plane: PlaneId, view: ViewRef): void {
  change(plane, (state) => opened(state, view));
}

/** **What `plane` has in force is `declared`**: an offer of a view no longer offered goes. */
export function offersInForce(plane: PlaneId, declared: readonly ExtensionView[]): void {
  change(plane, (state) => withdrawn(state, declared));
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The offers standing in `plane`, for a component that redraws when they change. */
export function useOffers(plane: PlaneId): readonly ViewOffer[] {
  return useSyncExternalStore(subscribe, () => (byPlane.get(plane) ?? NO_OFFERS).standing);
}

/** Forgets every offer, as a new launch would. For tests. */
export function forgetOffers(): void {
  byPlane.clear();
  tell();
}
