// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext } from "react";

/**
 * **Where a view's own actions go when it is drawn in a half of the Panel** (#1738, P-11).
 *
 * A half is headed by one line, `RegionFrame`'s: the open view's title, the view's own actions,
 * and the chevron that puts the half away. The actions are the view's, drawn where its section
 * heading has them in a sidebar (`PanelSection`), so the heading hands them to that line instead
 * of drawing a second one under it.
 *
 * `undefined` outside a half: a view in a sidebar draws its own heading, as it always has.
 * `null` in a half whose line is not mounted yet. Otherwise the element on the half's line that
 * holds this view's actions.
 */
export const HalfActions = createContext<HTMLElement | null | undefined>(undefined);
