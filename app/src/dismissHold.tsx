// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { UNDO_MS } from "./undo";

/**
 * **A Dismiss held for its Undo** (#5650, ADR 0119 decision 9): Dismiss and Dismiss all on Mine,
 * Project and Notices take the lines away at once, and an Undo stands for {@link UNDO_MS}. Only
 * when it goes is each line's own Dismiss carried out, by its own rule: an update is put away for
 * its day in the core's inbox store, a Notice until its cause changes in the layout file
 * (`dismissals.ts`), a teammate's change recorded as seen. So Undo is only ever the lines coming
 * back, whatever their Dismiss does, and nothing is written that Undo would have to take back.
 *
 * A second Dismiss while an Undo stands carries out the first at once, and its Undo takes the
 * first one's place. What is held is carried out too when the view goes or the window is put
 * away, so a Dismiss is never lost by moving on; only a window that ends inside the few seconds
 * without a `pagehide` loses it, and then the lines simply show again.
 */
export type DismissHold = {
  /** The lines taken away and not carried out yet, by key. */
  held: ReadonlySet<string>;
  /** How many lines the standing Undo brings back; `undefined` while none stands. */
  undoable: number | undefined;
  /**
   * Takes the lines `keys` away now, and carries `out` their Dismiss once the Undo goes, handed
   * the keys still held then (a line {@link DismissHold.release}d is not). Where what was done
   * is already done (Snooze all writes at once), `back` is what its Undo does.
   */
  hold: (
    keys: readonly string[],
    out: (still: ReadonlySet<string>) => void,
    back?: () => void,
  ) => void;
  /** Brings the held lines back, carries out nothing, and does the hold's `back` if it has one. */
  undo: () => void;
  /**
   * **Brings back the one line `key`** (R-334): the rest stay held for the same Undo, and only
   * theirs is carried out. A hold with a `back` was done at once, so it is undone whole.
   */
  release: (key: string) => void;
};

const NONE: ReadonlySet<string> = new Set();

type Held = {
  keys: ReadonlySet<string>;
  out: (still: ReadonlySet<string>) => void;
  back?: () => void;
};

/** One list's held Dismiss and its Undo. */
export function useDismissHold(): DismissHold {
  const [holding, setHolding] = useState<Held | undefined>();
  /** The same, read where no render is: a timer, the view going, the window put away. */
  const now = useRef<Held | undefined>(undefined);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  /** What is held is carried out, and the lines' own Dismiss keeps them away from here on. */
  const carryOut = useCallback(() => {
    clearTimeout(timer.current);
    const was = now.current;
    now.current = undefined;
    setHolding(undefined);
    was?.out(was.keys);
  }, []);

  const hold = useCallback(
    (keys: readonly string[], out: (still: ReadonlySet<string>) => void, back?: () => void) => {
      if (keys.length === 0) return;
      carryOut();
      const held = { keys: new Set(keys), out, back };
      now.current = held;
      setHolding(held);
      timer.current = setTimeout(carryOut, UNDO_MS);
    },
    [carryOut],
  );

  const undo = useCallback(() => {
    clearTimeout(timer.current);
    const was = now.current;
    now.current = undefined;
    setHolding(undefined);
    was?.back?.();
  }, []);

  const release = useCallback(
    (key: string) => {
      const was = now.current;
      if (was === undefined || !was.keys.has(key)) return;
      const keys = new Set([...was.keys].filter((one) => one !== key));
      if (was.back !== undefined || keys.size === 0) {
        undo();
        return;
      }
      const held = { ...was, keys };
      now.current = held;
      setHolding(held);
    },
    [undo],
  );

  useEffect(() => {
    window.addEventListener("pagehide", carryOut);
    return () => {
      window.removeEventListener("pagehide", carryOut);
      carryOut();
    };
  }, [carryOut]);

  return useMemo(
    () => ({ held: holding?.keys ?? NONE, undoable: holding?.keys.size, hold, undo, release }),
    [holding, hold, undo, release],
  );
}

/** **Dismiss all**, as every list that has one says it (#5650): one label and one tooltip. */
export const DISMISS_ALL = "Dismiss all";
export const DISMISS_ALL_TIP = "Put every line here away. Nothing is answered.";

/**
 * Holds the lines `keys` for their Undo where a list holds its Dismiss, and carries `out` their
 * Dismiss at once where it does not.
 */
export function holdOrCarryOut(
  hold: DismissHold | undefined,
  keys: readonly string[],
  out: (still: ReadonlySet<string>) => void,
): void {
  if (hold === undefined) out(new Set(keys));
  else hold.hold(keys, out);
}

/** What the Undo line says: how many lines went. */
export const dismissedSaid = (count: number): string =>
  `Dismissed ${counted(count, "line", "lines")}.`;

/**
 * **The Undo line** of a held Dismiss, while it stands: a Notice of its own, never in a list.
 * What it says and its cause are a Dismiss's unless it is handed another's (an ask's Ignore).
 */
export function DismissUndo({
  hold,
  cause = "lines-dismissed",
  said = dismissedSaid,
}: {
  hold: DismissHold | undefined;
  cause?: string;
  said?: (count: number) => string;
}) {
  if (hold?.undoable === undefined) return null;
  return (
    <Notice cause={cause} tone="news" fixes={[{ label: "Undo", onPress: hold.undo }]}>
      {said(hold.undoable)}
    </Notice>
  );
}
