// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * **Variant C: a panel per pool, and one queue.** A pool's "Profiles on this pool" panel has a
 * checkbox per profile beside a single "Sign in what's needed" queue, which walks through each
 * sign-in in turn. Add a pool ends on the same panel; Move to runner opens it under the chat's
 * move Notice, with the chat's profile already ticked.
 */
import { Badge } from "../../ui/badge/Badge";
import { Button } from "../../ui/button/Button";
import { Card, CardTitle } from "../../ui/card/Card";
import { Checkbox } from "../../ui/choice/Checkbox";
import { DialogInPlace } from "../../ui/dialog/Dialog";
import { HarnessIcon } from "../../ui/harness-icon/HarnessIcon";
import { Notice } from "../../Notice";
import {
  addedPool,
  focusOf,
  profilesOn,
  PROTOTYPE_DEFAULT_PROFILE,
  PROTOTYPE_MOVE,
  PROTOTYPE_POOLS,
  PROTOTYPE_PROFILES,
  PROTOTYPE_PROJECT,
  standingAt,
  STANDING_SAID,
  type PrototypeMoment,
  type PrototypePool,
  type SignInState,
} from "./data";
import { PrototypeChatPane, PrototypeScrim, PrototypeSettingsFrame } from "./Frame";
import { FreshClaimNote, SignInStep } from "./SignInStep";

export const VARIANT_C_NAME = "Panel per pool, one sign-in queue";

/** The queue: every profile on the pool that needs a sign-in, walked in order. */
function Queue({
  pool,
  state,
  moment,
}: {
  pool: PrototypePool;
  state: SignInState;
  moment: PrototypeMoment;
}) {
  const focus = focusOf(state, moment);
  const kept = profilesOn(pool.name, focus, moment);
  const needed = kept.filter(
    (one) =>
      one.name === focus.profile.name ||
      standingAt(pool.name, one.name, "choose", focus) !== "signed-in",
  );
  const at = needed.findIndex((one) => one.name === focus.profile.name);
  if (state === "choose") {
    return (
      <div className="proto-rsi-queue">
        <CardTitle as="h4">Sign in what&apos;s needed</CardTitle>
        <p className="ui-setting-help">
          {needed.length === 1
            ? "1 profile on this pool is not signed in on its runner."
            : `${needed.length} profiles on this pool are not signed in on its runner.`}{" "}
          purlis walks you through each in turn, in your browser.
        </p>
        <Button variant="primary">{`Sign in what's needed (${needed.length})`}</Button>
      </div>
    );
  }
  return (
    <div className="proto-rsi-queue">
      <CardTitle as="h4">Sign in what&apos;s needed</CardTitle>
      <ol className="proto-rsi-queue-list">
        {needed.map((one, index) => {
          const now = index === at;
          const standing = now
            ? standingAt(pool.name, one.name, state, focus)
            : index < at
              ? "signed-in"
              : "signed-out";
          return (
            <li key={one.name} className="proto-rsi-queue-item" data-now={now ? "" : undefined}>
              <div className="proto-rsi-profile-line">
                <HarnessIcon kind={one.harness} />
                <span className="ui-machine-name">{one.name}</span>
                <Badge variant={standing === "signed-in" ? "ok" : now ? "accent" : "neutral"}>
                  {now || standing === "signed-in" ? STANDING_SAID[standing] : "Next"}
                </Badge>
                {now && state !== "signed-in" && (
                  <Button size="sm" variant="ghost">
                    Skip for now
                  </Button>
                )}
              </div>
              {now && <SignInStep state={state} focus={focus} />}
            </li>
          );
        })}
      </ol>
    </div>
  );
}

/** "Profiles on this pool": a checkbox per profile beside the queue. */
function Panel({
  pool,
  state,
  moment,
}: {
  pool: PrototypePool;
  state: SignInState;
  moment: PrototypeMoment;
}) {
  const focus = focusOf(state, moment);
  const kept = profilesOn(pool.name, focus, moment).map((one) => one.name);
  return (
    <Card as="section" className="proto-rsi-panel" aria-label={`Profiles on ${pool.name}`}>
      <div className="proto-rsi-panel-boxes">
        <CardTitle as="h4">Profiles on {pool.name}</CardTitle>
        <p className="ui-setting-help">Chats on this pool can use only these.</p>
        <div className="proto-rsi-boxes" role="group" aria-label={`Profiles on ${pool.name}`}>
          {PROTOTYPE_PROFILES.map((one) => {
            const on =
              kept.includes(one.name) ||
              (state === "choose" && one.name === PROTOTYPE_DEFAULT_PROFILE);
            const standing = standingAt(pool.name, one.name, state, focus);
            const notes = [
              on ? STANDING_SAID[standing] : "Not on this pool",
              one.name === PROTOTYPE_DEFAULT_PROFILE
                ? `default for new chats in ${PROTOTYPE_PROJECT}`
                : undefined,
              moment === "move" &&
              one.name === PROTOTYPE_MOVE.profile &&
              pool.name === PROTOTYPE_MOVE.to
                ? "added for the move"
                : undefined,
            ].filter(Boolean);
            return (
              <Checkbox key={one.name} defaultSelected={on} description={notes.join(", ")}>
                {one.name}
              </Checkbox>
            );
          })}
        </div>
        {pool.freshClaim && <FreshClaimNote pool={pool.name} />}
      </div>
      <Queue pool={pool} state={state} moment={moment} />
    </Card>
  );
}

/** A pool's row: a count of what is signed in, and its panel's toggle. */
function PoolRow({
  pool,
  state,
  moment,
  open,
}: {
  pool: PrototypePool;
  state: SignInState;
  moment: PrototypeMoment;
  open: boolean;
}) {
  const focus = focusOf(state, moment);
  const kept = profilesOn(pool.name, focus, moment);
  const signedIn = kept.filter(
    (one) => standingAt(pool.name, one.name, state, focus) === "signed-in",
  ).length;
  return (
    <li className="ui-machine-entry proto-rsi-pool">
      <span className="ui-machine-name">{pool.name}</span>
      <span className="ui-machine-path">{pool.where}</span>
      <Badge variant={signedIn === kept.length ? "ok" : "waiting"}>
        {`${signedIn} of ${kept.length} signed in`}
      </Badge>
      <Button size="sm" aria-expanded={open}>
        Profiles on this pool
      </Button>
      <Button size="sm">Remove</Button>
      {open && (
        <div className="proto-rsi-under">
          <Panel pool={pool} state={state} moment={moment} />
        </div>
      )}
    </li>
  );
}

export function VariantC({ state, moment }: { state: SignInState; moment: PrototypeMoment }) {
  const focus = focusOf(state, moment);
  if (moment === "move") {
    const to = PROTOTYPE_POOLS.find((one) => one.name === PROTOTYPE_MOVE.to) ?? PROTOTYPE_POOLS[1];
    return (
      <PrototypeChatPane
        notices={
          <Notice
            cause="proto-move-waits"
            at="pane"
            label="Move"
            tone={state === "failed" ? "trouble" : "attention"}
            fixes={[{ label: "Cancel move", onPress: () => undefined }]}
            under={<Panel pool={to} state={state} moment={moment} />}
          >
            {state === "signed-in"
              ? `Moves to pool ${to.name} at its next pause.`
              : `Moves to pool ${to.name} once what it needs is signed in there: ${PROTOTYPE_MOVE.profile} is added to the pool for it.`}
          </Notice>
        }
      />
    );
  }
  if (moment === "add-pool") {
    const { added, other } = addedPool(state);
    return (
      <PrototypeSettingsFrame>
        <ul className="ui-machine-list">
          <PoolRow pool={other} state="signed-in" moment="settings" open={false} />
        </ul>
        <PrototypeScrim>
          <DialogInPlace
            onClose={() => undefined}
            size="wide"
            title={added.kind === "cluster" ? "Add a cluster pool" : "Add a Docker pool"}
            subtitle={`Pool ${added.name} is added. Last, its profiles: pick them, then sign in what's needed.`}
            main={{ children: state === "signed-in" ? "Done" : "Finish later" }}
            cancel={false}
          >
            <Panel pool={added} state={state} moment={moment} />
          </DialogInPlace>
        </PrototypeScrim>
      </PrototypeSettingsFrame>
    );
  }
  return (
    <PrototypeSettingsFrame>
      <ul className="ui-machine-list">
        {PROTOTYPE_POOLS.map((pool) => (
          <PoolRow
            key={pool.name}
            pool={pool}
            state={state}
            moment={moment}
            open={pool.name === focus.pool}
          />
        ))}
      </ul>
      <Button>Add a pool…</Button>
    </PrototypeSettingsFrame>
  );
}
