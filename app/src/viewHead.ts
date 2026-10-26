// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext } from "react";

/**
 * **Where a view's own actions go: its region's head** (#1738 P-11 for a half, #3330 for a
 * sidebar).
 *
 * A region that shows a view heads it with one line, `RegionFrame`'s: the open view's title, the
 * view's own actions, and the control that puts the region away (a half's chevron, the right
 * panel's ×). The view's section heading (`PanelSection`) hands its actions to that line and
 * draws no title of its own, so a view carries one title, as a view tab does (#3147).
 *
 * `undefined` outside a region: a view drawn on its own (a story, a test) draws its own heading.
 * `null` in a region whose line is not mounted, or does not show this view, yet. Otherwise the
 * element on the line that holds this view's actions.
 */
export const HeadActions = createContext<HTMLElement | null | undefined>(undefined);
