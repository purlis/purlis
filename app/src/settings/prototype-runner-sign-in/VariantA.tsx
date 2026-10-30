// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * **Variant A: inline in each pool's row.** Each pool's row lists the profiles it keeps; a
 * profile's line expands in place into a sign-in strip. Choosing profiles is chips on the row.
 * At Move to runner the strip opens under the chat's move Notice, in its pane.
 */
import { Badge } from "../../ui/badge/Badge";
import { Button } from "../../ui/button/Button";
import { Card } from "../../ui/card/Card";
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
  standingSaid,
  STANDING_SAID,
  type Focus,
  type PrototypePool,
  type PrototypeMoment,
  type SignInState,
} from "./data";
import { PrototypeChatPane, PrototypeScrim, PrototypeSettingsFrame } from "./Frame";
import { SignInStep } from "./SignInStep";

export const VARIANT_A_NAME = "Inline in the pool's row";

/** The strip a profile's line opens into. */
function Strip({ state, focus }: { state: SignInState; focus: Focus }) {
  return (
    <Card className="proto-rsi-strip">
      <div className="proto-rsi-strip-head">
        <strong>
          Signing in {focus.profile.name} on {focus.pool}
        </strong>
        <Button size="sm" variant="ghost">
          {state === "signed-in" ? "Close" : "Cancel"}
        </Button>
      </div>
      <SignInStep state={state} focus={focus} />
    </Card>
  );
}

/** The chips a row offers while profiles are chosen for it. */
function ChooseChips({ pool }: { pool: PrototypePool }) {
  return (
    <div className="proto-rsi-choose">
      <p className="ui-setting-help">
        Which of your profiles should chats on {pool.name} use? Each one you pick signs in on the
        runner once.
      </p>
      <div className="proto-rsi-chips" role="group" aria-label={`Profiles on ${pool.name}`}>
        {PROTOTYPE_PROFILES.map((one) => (
          <Checkbox
            key={one.name}
            variant="chip"
            defaultSelected={
              pool.profiles.includes(one.name) || one.name === PROTOTYPE_DEFAULT_PROFILE
            }
            tooltip={
              one.name === PROTOTYPE_DEFAULT_PROFILE
                ? `The profile a new chat in ${PROTOTYPE_PROJECT} gets`
                : one.harnessSaid
            }
          >
            {one.name === PROTOTYPE_DEFAULT_PROFILE ? `${one.name} (default)` : one.name}
          </Checkbox>
        ))}
      </div>
      <div className="proto-rsi-actions">
        <Button variant="primary">Sign in what&apos;s picked</Button>
        <Button variant="ghost">Later</Button>
      </div>
    </div>
  );
}

/** A pool's profile lines, the focused one opened into its strip. */
function ProfileLines({
  pool,
  state,
  moment,
}: {
  pool: PrototypePool;
  state: SignInState;
  moment: PrototypeMoment;
}) {
  const focus = focusOf(state, moment);
  if (state === "choose" && focus.pool === pool.name) return <ChooseChips pool={pool} />;
  return (
    <ul className="proto-rsi-profiles" aria-label={`Profiles on ${pool.name}`}>
      {profilesOn(pool.name, focus, moment).map((one) => {
        const standing = standingAt(pool.name, one.name, state, focus);
        const open = focus.pool === pool.name && focus.profile.name === one.name;
        return (
          <li key={one.name} className="proto-rsi-profile">
            <div className="proto-rsi-profile-line">
              <HarnessIcon kind={one.harness} />
              <span className="ui-machine-name">{one.name}</span>
              <span className="ui-setting-help">
                {standing === "signed-in" || standing === "signed-out"
                  ? standingSaid(pool.name, standing)
                  : STANDING_SAID[standing]}
                {standing === "signed-in" && pool.freshClaim ? " For this claim only." : ""}
              </span>
              {(standing === "signed-out" || standing === "failed") && !open && (
                <Button size="sm">Sign in</Button>
              )}
              <Button
                size="sm"
                variant="ghost"
                tooltip={`Takes ${one.name} off ${pool.name}; its sign-in there goes with it`}
              >
                Remove
              </Button>
            </div>
            {open && standing !== "signed-out" && <Strip state={state} focus={focus} />}
          </li>
        );
      })}
      <li>
        <Button size="sm" variant="ghost">
          Add a profile…
        </Button>
      </li>
    </ul>
  );
}

/** A pool's row, as the real group draws it, with its profile lines under it. */
function PoolRow({
  pool,
  state,
  moment,
}: {
  pool: PrototypePool;
  state: SignInState;
  moment: PrototypeMoment;
}) {
  return (
    <li className="ui-machine-entry proto-rsi-pool">
      <span className="ui-machine-name">{pool.name}</span>
      <span className="ui-machine-path">{pool.where}</span>
      <Badge variant="accent">linked</Badge>
      <Button size="sm">Remove</Button>
      <div className="proto-rsi-under">
        <ProfileLines pool={pool} state={state} moment={moment} />
      </div>
    </li>
  );
}

export function VariantA({ state, moment }: { state: SignInState; moment: PrototypeMoment }) {
  if (moment === "move") {
    const focus = focusOf(state, moment);
    const done = state === "signed-in";
    return (
      <PrototypeChatPane
        notices={
          <Notice
            cause="proto-move-waits"
            at="pane"
            label="Move"
            tone={state === "failed" ? "trouble" : "attention"}
            fixes={
              state === "choose"
                ? [
                    { label: "Sign in", onPress: () => undefined },
                    { label: "Cancel move", onPress: () => undefined },
                  ]
                : [{ label: "Cancel move", onPress: () => undefined }]
            }
            under={state === "choose" ? undefined : <Strip state={state} focus={focus} />}
          >
            {done
              ? `Moves to pool ${PROTOTYPE_MOVE.to} at its next pause.`
              : `Moves to pool ${PROTOTYPE_MOVE.to} once ${PROTOTYPE_MOVE.profile} is signed in there. It keeps working here meanwhile.`}
          </Notice>
        }
      />
    );
  }
  const pools = (
    <ul className="ui-machine-list">
      {PROTOTYPE_POOLS.map((pool) => (
        <PoolRow key={pool.name} pool={pool} state={state} moment={moment} />
      ))}
    </ul>
  );
  if (moment === "add-pool") {
    const { added, other } = addedPool(state);
    return (
      <PrototypeSettingsFrame>
        <ul className="ui-machine-list">
          <PoolRow pool={other} state="signed-in" moment="settings" />
        </ul>
        <PrototypeScrim>
          <DialogInPlace
            onClose={() => undefined}
            title={`Pool ${added.name} is added`}
            subtitle="Sign in the profiles its chats use. Each signs in on the runner itself, once."
            main={{ children: "Done" }}
            cancel={false}
          >
            <ProfileLines pool={added} state={state} moment={moment} />
          </DialogInPlace>
        </PrototypeScrim>
      </PrototypeSettingsFrame>
    );
  }
  return (
    <PrototypeSettingsFrame>
      {pools}
      <Button>Add a pool…</Button>
    </PrototypeSettingsFrame>
  );
}
