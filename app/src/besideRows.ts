// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from "react";
import { besideTheFront, type Catalogued } from "./actions";
import { useHeldWhileSame } from "./heldWhileSame";

/**
 * **The rows the views beside the tabs read** (FS-W2, #7910): the catalogue without the rows
 * about the tabs and panes (`besideTheFront`), held while it says the same, so a catalogue made
 * anew for a press on a tab draws none of the views that read it.
 *
 * What is held is the list of entries, not the map: `dequal` compares two maps by their keys
 * alone and ignores their order, and the views read these rows in the catalogue's order
 * (`moveRows`, `curateRows`, `handedTo`), so the same rows in a new order are a change they draw.
 */
export function useBesideRows(found: Catalogued): Catalogued {
  const held = useHeldWhileSame(
    useMemo(() => [...found].filter(([id]) => besideTheFront(id)), [found]),
  );
  return useMemo(() => new Map(held), [held]);
}
