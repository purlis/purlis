// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useSyncExternalStore } from "react";
import { commands, type DoctorFixed, type PlaneId } from "./bindings";

/**
 * **An alert's fix, applied from where it is pressed** (#1301, #3650): the doctor's registry by
 * its id; what it came to, said where it is drawn when it did not cure the alert; then every
 * project's alerts read again, since a fix that half-ran changed something too.
 *
 * **One per project and fix** (#3930's review): the Inbox's row and a workspace chip's card press
 * the same fix (the layout's `workspace-reinit`), so what it is doing and what it answered are
 * kept here, by project and fix, and both read them. A press while it runs, from either place,
 * sends nothing; a refusal is said in both; and another project's answer is never said on this
 * one's line.
 */

/** What one project's fix is doing, and what it last answered when it did not cure the alert. */
type Standing = { busy: boolean; said: string | undefined };

const IDLE: Standing = { busy: false, said: undefined };
const held = new Map<string, Standing>();
const listeners = new Set<() => void>();

const keyOf = (plane: PlaneId, id: string) => `${plane}\u0000${id}`;

function set(key: string, change: Partial<Standing>) {
  held.set(key, { ...(held.get(key) ?? IDLE), ...change });
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

/** Forgets every fix's state, as a new launch would. For tests. */
export function forgetAlertFixes(): void {
  held.clear();
  for (const listener of listeners) listener();
}

/**
 * **The fix `id` of `plane`'s alert**, as one of the places that press it: whether it is running,
 * what it last said, and the press. `label` is the button's words, which the sentence about a
 * refusal names (`purlis could not <label>: why`).
 */
export function useAlertFix(plane: PlaneId, id: string, label: string, reread: () => void) {
  const key = keyOf(plane, id);
  const { busy, said } = useSyncExternalStore(subscribe, () => held.get(key) ?? IDLE);
  /** The act, as the sentence says it: its verb in lower case, and no ellipsis. */
  const act = label.charAt(0).toLowerCase() + label.slice(1).replace(/…$/, "");
  const setSaid = useCallback((words: string | undefined) => set(key, { said: words }), [key]);
  const setBusy = useCallback((running: boolean) => set(key, { busy: running }), [key]);
  const landed = useCallback(
    (fixed: DoctorFixed) => {
      if (fixed.refused !== null) set(key, { said: `purlis could not ${act}: ${fixed.refused}` });
      else if (!fixed.complete) set(key, { said: fixed.said.join(" ") });
      reread();
    },
    [key, act, reread],
  );
  const apply = useCallback(
    (fix: string) => {
      if ((held.get(key) ?? IDLE).busy) return;
      set(key, { busy: true, said: undefined });
      void commands
        .planeDoctorFix(plane, fix)
        .then((answer) =>
          landed(
            answer.status === "ok"
              ? answer.data
              : { fix, refused: answer.error, said: [], complete: false },
          ),
        )
        .catch((err: unknown) => landed({ fix, refused: String(err), said: [], complete: false }))
        .finally(() => set(key, { busy: false }));
    },
    [key, plane, landed],
  );
  return { said, setSaid, busy, setBusy, landed, apply };
}
