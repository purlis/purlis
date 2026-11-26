// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What stands over the window now** (#7728): how many of the dialog family's dialogs (the
 * palette among them) and how many menus (a select's list among them) are open in this window.
 * A dialog counts itself (`ui/dialog/Dialog.tsx`), a menu its surface (`ui/popover/Popover.tsx`);
 * an alert the app raises waits until none is up (`ui/dialog/raised.ts`).
 */
import { useEffect } from "react";

export type Layer = "dialog" | "menu";

const open: Record<Layer, number> = { dialog: 0, menu: 0 };
const hearing = new Set<() => void>();

/** How many layers of `kind` are open. */
export const layersUp = (kind: Layer): number => open[kind];

/** Hears every change of the count; returns the way to stop. */
export function hearLayers(tell: () => void): () => void {
  hearing.add(tell);
  return () => void hearing.delete(tell);
}

/**
 * Tells everyone who hears. One that throws is reported once the rest have heard, and the count
 * it heard of stands: a throw out of the effect would take its undoing with it.
 */
function tellAll() {
  for (const tell of [...hearing]) {
    try {
      tell();
    } catch (error) {
      queueMicrotask(() => {
        throw error;
      });
    }
  }
}

/** Counts a layer of `kind` while `isOpen`. */
export function useLayerUp(kind: Layer, isOpen: boolean): void {
  useEffect(() => {
    if (!isOpen) return;
    open[kind] += 1;
    tellAll();
    return () => {
      open[kind] -= 1;
      tellAll();
    };
  }, [kind, isOpen]);
}
