// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { AlertDialog } from "../ui/dialog/Dialog";
import type { PoolState } from "./poolState";

/**
 * **Forget pool…, asked in Pool setup** (#7191): what stays on the machine if the pool is
 * forgotten here, in the words of Settings › Pools (what may be left there: the runner's
 * container and home volume, the browsers it ran, and the logins given on it), and Remove beside
 * Forget it here only where the machine answers: Remove cleans the machine, Forget never
 * contacts it.
 */

/** Whether the machine answers in this state: it answered a refused add and a gone runner, and
 *  doesn't answer while unreachable or showing another key. */
export function machineAnswers(state: PoolState): boolean {
  return state === "refused" || state === "runner-gone";
}

export function ForgetPoolDialog({
  pool,
  where,
  canRemove,
  onForget,
  onRemove,
  onCancel,
}: {
  pool: string;
  /** The machine, or the cluster's context, the pool is on. */
  where: string;
  /** The machine answers and the pool was made: Remove cleans it. */
  canRemove: boolean;
  onForget: () => void;
  onRemove: () => void;
  onCancel: () => void;
}) {
  return (
    <AlertDialog
      onClose={onCancel}
      title={`Forget pool ${pool}?`}
      description={
        <>
          <p className="came-back ui-text-secondary">
            {`Forget it here forgets pool ${pool} on this machine without contacting ${where}. Whatever may be left on ${where} stays there:`}
          </p>
          <ul className="came-back ui-text-secondary" aria-label={`What may be left on ${where}`}>
            <li>its runner&apos;s container and home volume</li>
            <li>the browsers it ran</li>
            <li>the logins given on it</li>
          </ul>
          <p className="came-back ui-text-secondary">
            {canRemove
              ? `Remove cleans them off ${where} first, then forgets the pool. There is no undo.`
              : `purlis then says the commands that remove them on ${where}. There is no undo.`}
          </p>
        </>
      }
      main={{ children: "Forget it here", variant: "danger", onPress: onForget }}
      alternative={
        canRemove ? { children: "Remove", variant: "danger", onPress: onRemove } : undefined
      }
    />
  );
}
