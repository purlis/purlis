// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { MoveIntoReposPlan } from "./bindings";
import { AlertDialog } from "./ui/dialog/Dialog";

/** The doctor's fix that moves a workspace's repos at its root into its `repos/` (#5374). */
export const MOVE_INTO_REPOS = "move-into-repos";

/**
 * **Move into repos/, asked before anything moves** (#5374, ADR 0114 decision 4): which repos
 * move in each workspace, which workspaces stay as they are because a chat is open in them, and
 * which chats their harness can no longer resume afterwards, because they were started in a repo
 * itself rather than in a branch folder. Their session records still open.
 *
 * An alert dialog: the move puts a repo somewhere else on disk, so the keyboard starts on Cancel
 * and Return never presses Move.
 */
export function MoveIntoReposConfirm({
  plans,
  onCancel,
  onMove,
}: {
  plans: readonly MoveIntoReposPlan[];
  onCancel: () => void;
  onMove: () => void;
}) {
  const moving = plans.filter((plan) => plan.open.length === 0);
  const held = plans.filter((plan) => plan.open.length > 0);
  const unresumable = moving.flatMap((plan) => plan.unresumable);
  return (
    <AlertDialog
      onClose={onCancel}
      title="Move repos into repos/?"
      description={
        <p>
          Each workspace's repos move into its repos/ folder, and every branch folder cut from them
          is repaired. If any step fails, the workspace is put back as it was.
        </p>
      }
      main={
        moving.length > 0 ? { children: "Move repos", byReturn: false, onPress: onMove } : undefined
      }
    >
      {moving.length > 0 ? (
        <ul aria-label="What moves">
          {moving.map((plan) => (
            <li key={plan.workspace}>{`${plan.workspace}: ${plan.repos.join(", ")}`}</li>
          ))}
        </ul>
      ) : (
        <p>Nothing can move now: every workspace below has a chat open in it.</p>
      )}
      {held.length > 0 && (
        <section aria-label="Left as they are">
          <p>These workspaces stay as they are until their chats are closed:</p>
          <ul>
            {held.map((plan) => (
              <li key={plan.workspace}>{`${plan.workspace}: ${plan.open.join(", ")}`}</li>
            ))}
          </ul>
        </section>
      )}
      {unresumable.length > 0 && (
        <section aria-label="Chats that cannot be resumed">
          <p>
            These chats were started in a repo that moves. Their harness can no longer resume them,
            and their session records still open:
          </p>
          <ul>
            {unresumable.map((title, at) => (
              <li key={`${at}:${title}`}>{title}</li>
            ))}
          </ul>
        </section>
      )}
    </AlertDialog>
  );
}
