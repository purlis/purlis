// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * **Variant B: one focused dialog per sign-in.** The pool's row only says how its profiles
 * stand. Each sign-in is its own dialog with numbered steps, one after another; choosing the
 * profiles is a dialog of its own before them. Move to runner opens the same dialog over the
 * chat, and the move waits for it.
 */
import { Check } from "lucide-react";
import { Badge } from "../../ui/badge/Badge";
import { Button } from "../../ui/button/Button";
import { Checkbox } from "../../ui/choice/Checkbox";
import { DialogInPlace } from "../../ui/dialog/Dialog";
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
  type Focus,
  type PrototypeMoment,
  type PrototypePool,
  type SignInState,
} from "./data";
import { PrototypeChatPane, PrototypeScrim, PrototypeSettingsFrame } from "./Frame";
import { SignInStep } from "./SignInStep";

export const VARIANT_B_NAME = "One dialog per sign-in, in steps";

/** The vendor's own step, in the harness's words. */
function vendorStep(focus: Focus, state: SignInState): string {
  if (state === "terminal") return "Finish it in the runner's terminal";
  switch (focus.profile.harness) {
    case "claude":
      return "Paste the code the page shows";
    case "codex":
      return "Approve purlis in the browser";
    case "copilot":
      return "Enter the code on GitHub";
  }
}

/** Where each state stands in the steps: the step it is at, or past all of them. */
function stepAt(state: SignInState): number {
  switch (state) {
    case "choose":
      return 0;
    case "opening":
      return 1;
    case "signed-in":
      return 4;
    default:
      return 2;
  }
}

function Steps({ focus, state, first }: { focus: Focus; state: SignInState; first: string }) {
  const at = stepAt(state);
  const steps = [first, "Open the sign-in in your browser", vendorStep(focus, state), "Signed in"];
  return (
    <ol className="proto-rsi-steps" aria-label="Steps">
      {steps.map((words, index) => {
        const done = index < at || (index === 3 && at === 4);
        const now = index === at;
        const failed = now && state === "failed";
        return (
          <li
            key={words}
            className="proto-rsi-step-item"
            data-done={done ? "" : undefined}
            data-now={now ? "" : undefined}
            aria-current={now ? "step" : undefined}
          >
            <span className="proto-rsi-step-mark" aria-hidden="true">
              {done ? <Check size={12} /> : index + 1}
            </span>
            <span>{words}</span>
            {failed && <Badge variant="waiting">Did not finish</Badge>}
          </li>
        );
      })}
    </ol>
  );
}

/** One sign-in's dialog. */
function SignInDialog({
  focus,
  state,
  first,
  subtitle,
  count,
}: {
  focus: Focus;
  state: SignInState;
  /** What the first step was: picked at Add a pool, or added for the move. */
  first: string;
  subtitle: string;
  /** "1 of 2", where more sign-ins follow. */
  count?: string;
}) {
  const done = state === "signed-in";
  return (
    <DialogInPlace
      onClose={() => undefined}
      title={`Sign in ${focus.profile.name} on ${focus.pool}${count ? ` (${count})` : ""}`}
      subtitle={subtitle}
      cancel={done ? false : { children: "Cancel" }}
      main={done ? { children: count ? "Next sign-in" : "Done" } : undefined}
    >
      <Steps focus={focus} state={state} first={first} />
      <SignInStep state={state} focus={focus} />
    </DialogInPlace>
  );
}

/** The dialog that picks a pool's profiles, before their sign-ins. */
function ChooseDialog({ pool, title }: { pool: PrototypePool; title: string }) {
  return (
    <DialogInPlace
      onClose={() => undefined}
      title={title}
      subtitle="Chats on this pool can use only the profiles picked here. Each signs in on the runner once."
      main={{ children: "Next: sign in 2 profiles" }}
    >
      <div className="proto-rsi-boxes" role="group" aria-label={`Profiles on ${pool.name}`}>
        {PROTOTYPE_PROFILES.map((one) => (
          <Checkbox
            key={one.name}
            defaultSelected={
              pool.profiles.includes(one.name) || one.name === PROTOTYPE_DEFAULT_PROFILE
            }
            description={
              one.name === PROTOTYPE_DEFAULT_PROFILE
                ? `${one.harnessSaid}. The profile a new chat in ${PROTOTYPE_PROJECT} gets.`
                : one.harnessSaid
            }
          >
            {one.name}
          </Checkbox>
        ))}
      </div>
    </DialogInPlace>
  );
}

/** A pool's row: how its profiles stand, and one button to them. */
function PoolRow({
  pool,
  state,
  moment,
}: {
  pool: PrototypePool;
  state: SignInState;
  moment: PrototypeMoment;
}) {
  const focus = focusOf(state, moment);
  return (
    <li className="ui-machine-entry proto-rsi-pool">
      <span className="ui-machine-name">{pool.name}</span>
      <span className="ui-machine-path">{pool.where}</span>
      <Badge variant="accent">linked</Badge>
      <Button size="sm">Remove</Button>
      <div className="proto-rsi-under proto-rsi-badges">
        {profilesOn(pool.name, focus, moment).map((one) => {
          const standing = standingAt(pool.name, one.name, state, focus);
          return (
            <Badge key={one.name} variant={standing === "signed-in" ? "ok" : "waiting"}>
              {one.name}: {STANDING_SAID[standing]}
            </Badge>
          );
        })}
        <Button size="sm" variant="ghost">
          Profiles and sign-in…
        </Button>
      </div>
    </li>
  );
}

export function VariantB({ state, moment }: { state: SignInState; moment: PrototypeMoment }) {
  const focus = focusOf(state, moment);
  if (moment === "move") {
    return (
      <>
        <PrototypeChatPane
          notices={
            <Notice
              cause="proto-move-waits"
              at="pane"
              label="Move"
              tone={state === "failed" ? "trouble" : "attention"}
              fixes={[
                { label: "Sign in…", onPress: () => undefined },
                { label: "Cancel move", onPress: () => undefined },
              ]}
            >
              {state === "signed-in"
                ? `Moves to pool ${PROTOTYPE_MOVE.to} at its next pause.`
                : `Waiting to move to pool ${PROTOTYPE_MOVE.to}: ${PROTOTYPE_MOVE.profile} is not signed in there yet.`}
            </Notice>
          }
        />
        <PrototypeScrim>
          <SignInDialog
            focus={focus}
            state={state === "choose" ? "opening" : state}
            first={`Add ${PROTOTYPE_MOVE.profile} to ${PROTOTYPE_MOVE.to}`}
            subtitle={`So "${PROTOTYPE_MOVE.chat}" can move there. The move waits for this; the chat keeps working here.`}
          />
        </PrototypeScrim>
      </>
    );
  }
  const { added, other } = addedPool(state);
  const pools = moment === "add-pool" ? [other] : PROTOTYPE_POOLS;
  const choosing = PROTOTYPE_POOLS.find((one) => one.name === focus.pool) ?? PROTOTYPE_POOLS[0];
  return (
    <PrototypeSettingsFrame>
      <ul className="ui-machine-list">
        {pools.map((pool) => (
          <PoolRow key={pool.name} pool={pool} state={state} moment={moment} />
        ))}
      </ul>
      <Button>Add a pool…</Button>
      <PrototypeScrim>
        {state === "choose" ? (
          <ChooseDialog
            pool={moment === "add-pool" ? added : choosing}
            title={
              moment === "add-pool"
                ? `Pool ${added.name} is added: pick its profiles`
                : `Profiles on ${choosing.name}`
            }
          />
        ) : (
          <SignInDialog
            focus={focus}
            state={state}
            first={
              moment === "add-pool"
                ? "Pick the pool's profiles"
                : `Add ${focus.profile.name} to ${focus.pool}`
            }
            subtitle={`${focus.profile.harnessSaid}, on ${focus.pool}'s runner. The runner keeps the sign-in; nothing of this Mac's goes there.`}
            count={moment === "add-pool" ? "1 of 2" : undefined}
          />
        )}
      </PrototypeScrim>
    </PrototypeSettingsFrame>
  );
}
