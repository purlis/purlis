// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { drawnNow, PANEL, panelShown, type Arrangement, type Toggled } from "./regions";
import type { ViewId } from "./sideViews";

/**
 * **The regions put away, and the Panel while no half of it is drawn now** (#1737, #1752): what
 * the palette's Show and Hide rows are titled by. The Panel counts as shown only where a half is
 * drawn as the frame draws it ({@link drawnNow}): a half holding only extensions' panels that
 * are not contributed now (`present`) is not drawn, so it does not keep the Panel "shown".
 */
export function putAway(arrangement: Arrangement, present: readonly ViewId[]): Toggled[] {
  const away: Toggled[] = arrangement.filter((one) => one.collapsed).map((one) => one.id);
  return panelShown(drawnNow(arrangement, present)) ? away : [...away, PANEL];
}
