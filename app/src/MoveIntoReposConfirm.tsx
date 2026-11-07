// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { MoveIntoReposPlan } from "./bindings";
import { AlertDialog } from "./ui/dialog/Dialog";

/** The doctor's fix that moves a workspace's repos at its root into its `repos/` (#5374). */
export const MOVE_INTO_REPOS = "move-into-repos";

/**
 * **What the confirmation shows as moving or finishing**: every workspace but those it leaves as
 * they are, because a chat is open in them. Only these are sent with Move repos, so a workspace
 * shown as left alone is never moved, even once its chat closes (#5569). A move cut short is
 * finished whatever is sent.
 */
export function shownMoving(plans: readonly MoveIntoReposPlan[]): MoveIntoReposPlan[] {
  return plans.filter((plan) => plan.repos.length === 0 || plan.open.length === 0);
}

/**
 * **Move into repos/, asked before anything moves** (#5374, ADR 0114 decision 4): which repos
 * move in each workspace, which workspaces stay as they are because a chat is open in them, and
 * which chats their harness can no longer resume afterwards, because they were started in a repo
 * itself rather than in a branch folder. Their session records still open.
 *
 * An alert dialog: the move puts a repo somewhere else on disk, so the keyboard starts on Cancel
 * and Return never presses Move. Where what would move could not be read, it says why and offers
 * only Cancel: nothing moves without being asked.
 */
export function MoveIntoReposConfirm(
  props: ({ plans: readonly MoveIntoReposPlan[] } | { failed: string }) & {
    onCancel: () => void;
    onMove: () => void;
  },
) {
  const { onCancel, onMove } = props;
  if ("failed" in props)
    return (
      <AlertDialog
        onClose={onCancel}
        title="Move repos into repos/?"
        description={
          <p>
            {`Could not read what would move: ${props.failed}. Nothing was moved. Run the doctor again, and press Fix once it shows the row.`}
          </p>
        }
      />
    );
  const { plans } = props;
  const toMove = plans.filter((plan) => plan.repos.length > 0);
  const moving = toMove.filter((plan) => plan.open.length === 0);
  const held = toMove.filter((plan) => plan.open.length > 0);
  // A move a crash cut short is finished whatever is open: it moves nothing more (#5569).
  const finishing = plans.filter((plan) => plan.finishing.length > 0);
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
        moving.length > 0 || finishing.length > 0
          ? { children: "Move repos", byReturn: false, onPress: onMove }
          : undefined
      }
    >
      {finishing.length > 0 && (
        <section aria-label="Cut short">
          <p>
            A move into repos/ stopped partway in these workspaces, so git may not read some branch
            folders. Moving finishes it first by repairing them:
          </p>
          <ul>
            {finishing.map((plan) => (
              <li key={plan.workspace}>{`${plan.workspace}: ${plan.finishing.join(", ")}`}</li>
            ))}
          </ul>
        </section>
      )}
      {moving.length > 0 ? (
        <ul aria-label="What moves">
          {moving.map((plan) => (
            <li key={plan.workspace}>{`${plan.workspace}: ${plan.repos.join(", ")}`}</li>
          ))}
        </ul>
      ) : (
        held.length > 0 && <p>Nothing can move now: every workspace below has a chat open in it.</p>
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
