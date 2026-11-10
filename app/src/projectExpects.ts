// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ExpectedItem, ExpectedShown } from "./bindings";

// **What an answer to a project's expects is sent** (#1888), by its Notice and by its row in Asks
// alike (#6000): one answer, whichever place it was given in.

/** Whether an Allow would keep `one`: it can be allowed on this machine now. */
export const allowable = (one: ExpectedItem) => one.refused === null;

/**
 * **What an Allow is sent: each item as it was drawn**, so only that is kept. The name it was
 * drawn under goes with it, and nothing for one drawn as "cannot be allowed here now": a path a
 * repository declared may lead somewhere else by the time of the press, and the backend keeps
 * an item only while it still stands as the person saw it.
 */
export const asDrawn = (items: readonly ExpectedItem[]): ExpectedShown[] =>
  items.map((one) => ({ target: one.target, named: allowable(one) ? one.named : null }));

/**
 * **What a Not now is sent: each item with the name it was drawn under** (#1927), one that
 * cannot be allowed here now too. The answer is to what the person saw: the app remembers it by
 * that name, and asks again when a path the project declared comes to name another socket.
 */
export const asNamed = (items: readonly ExpectedItem[]): ExpectedShown[] =>
  items.map((one) => ({ target: one.target, named: one.named }));
