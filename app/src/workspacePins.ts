// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A project's pins changed from outside its view** (#2916): the title bar's two-column switcher
 * pins a workspace of a project behind the one in front, whose view builds no catalogue while it
 * is behind (`PlaneView`'s `offers`), so the pin is written straight to the store and the view is
 * told to read its pins again. Window-wide, as the store is the machine's.
 */

const heard = new Set<(plane: string) => void>();

/** Tells the view of `plane` that the store's pins for it changed. */
export function pinsChanged(plane: string): void {
  for (const one of heard) one(plane);
}

/** Hears every {@link pinsChanged}; what it gives back stops hearing. */
export function onPinsChanged(hear: (plane: string) => void): () => void {
  heard.add(hear);
  return () => {
    heard.delete(hear);
  };
}
