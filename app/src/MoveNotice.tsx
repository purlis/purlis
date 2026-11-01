// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import {
  commands,
  type ChatMove,
  type MoveSignIn,
  type PlaneId,
  type PlanRow,
  type PoolLimitOffer,
} from "./bindings";
import {
  askMove,
  dismissMovedAfterSignIn,
  forgetMove,
  moveCancellable,
  moveSaid,
  placeSaid,
  putAwayMove,
  signedInForMove,
  useChatMoves,
  useMovedAfterSignIn,
} from "./chatMoves";
import { Notice, type NoticeAction } from "./Notice";
import { saveLimits } from "./pools";
import { RaiseLimit } from "./RaiseLimit";
import { askProfileSignIn } from "./settings/links";
import { DRIVEN } from "./settings/PoolProfiles";
import { PoolSignIn, approvesIn, drivenPlan } from "./settings/PoolSignIn";

/**
 * **This chat's move, on its pane** (#2048): that it is moving and moves at its next pause, with
 * Cancel move while it has not; or, where it did not move, why, until it is dismissed. The chat
 * keeps running where it is in every case.
 *
 * **A move that waits on its profile's sign-in on the runner** (#3477, spec #4023 stories 21-24,
 * prototype Variant A) says so, with Sign in, Try the move and Cancel move. Sign in opens the same
 * sign-in strip as a pool's row under its line ({@link PoolSignIn}); a harness whose sign-in the
 * window does not drive yet opens its own in a shell tab on the runner instead. A harness that
 * serves several plans is asked which, as a pool's line asks it (#4181). Once the strip
 * says signed in, the core is told, asks the runner itself, and the move goes ahead by itself;
 * Try the move tells it the same after a sign-in elsewhere (Open in terminal). The strip's Cancel,
 * like Cancel move, cancels the move, and the chat stays here. Having gone ahead by itself, it
 * says it moved, until that is dismissed (#4135).
 *
 * **A move refused by a pool's limit offers to raise it** (#4313), as Start a chat does: saving
 * the values asks for the move again.
 */
export function MoveNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const one = useChatMoves(plane)[session];
  const movedTo = useMovedAfterSignIn(plane, session);
  if (one === undefined)
    return movedTo === undefined ? null : (
      <Notice
        cause={`chat-moved:${session}`}
        at="pane"
        label="Move"
        onDismiss={() => dismissMovedAfterSignIn(plane, session)}
      >
        {`Moved to ${placeSaid(movedTo)}.`}
      </Notice>
    );
  const said = moveSaid(one);
  if (said.trouble)
    return (
      // Another refusal is a form afresh.
      <MoveFailed
        key={one.why ?? ""}
        plane={plane}
        session={session}
        one={one}
        words={said.words}
      />
    );
  // Being made now, past cancelling (#3814): it says so, and Dismiss only hides the line.
  if (!moveCancellable(one))
    return (
      <Notice
        cause={`chat-move:${session}`}
        at="pane"
        label="Move"
        onDismiss={() => forgetMove(plane, session)}
        dismissSays="Hides this line. The move is being made, and goes on."
      >
        {said.words}
      </Notice>
    );
  if (one.stage === "signing-in" && one.sign_in !== null && one.to !== null)
    return (
      <WaitsOnASignIn
        // A wait asked again is a strip afresh.
        key={`${one.to}:${one.sign_in.profile}`}
        plane={plane}
        session={session}
        pool={one.to}
        asked={one.sign_in}
        words={said.words}
      />
    );
  return (
    <Notice
      cause={`chat-move:${session}`}
      at="pane"
      label="Move"
      fixes={[{ label: "Cancel move", onPress: () => putAwayMove(plane, session) }]}
    >
      {said.words}
    </Notice>
  );
}

/** A move that did not happen, why, and the limit to raise where one refused it. */
function MoveFailed({
  plane,
  session,
  one,
  words,
}: {
  plane: PlaneId;
  session: number;
  one: ChatMove;
  words: string;
}) {
  const { from, to, why } = one;
  const [offer, setOffer] = useState<PoolLimitOffer | null>(null);
  const [saving, setSaving] = useState(false);
  const [refused, setRefused] = useState<string>();
  // Dismissed or gone while the limit saves: the move isn't asked for after all.
  const left = useRef(false);
  useEffect(() => {
    left.current = false;
    return () => {
      left.current = true;
    };
  }, []);
  useEffect(() => {
    // A move back here is refused on the pool it is leaving (#4397).
    const pool = to ?? from;
    if (pool === null || why === null) return;
    let gone = false;
    void commands
      .poolLimitOfferForMove(pool, why)
      .catch(() => null)
      .then((said) => {
        if (!gone) setOffer(said);
      });
    return () => {
      gone = true;
    };
  }, [to, from, why]);
  const apply = async (values: ReadonlyArray<readonly [string, number]>) => {
    if (offer === null) return;
    setSaving(true);
    setRefused(undefined);
    const saved = await saveLimits(offer.pool, values);
    if (left.current) return;
    if (saved === undefined) await askMove(plane, session, from, to);
    else setRefused(saved);
    setSaving(false);
  };
  return (
    <Notice
      cause={`chat-move-failed:${session}`}
      at="pane"
      tone="trouble"
      label="Move"
      onDismiss={() => {
        left.current = true;
        putAwayMove(plane, session);
      }}
      under={
        offer === null ? undefined : (
          <RaiseLimit
            offer={offer}
            action="Save to the pool and move"
            working={saving ? "Saving…" : undefined}
            refused={refused}
            onApply={(values) => void apply(values)}
          />
        )
      }
    >
      {words}
    </Notice>
  );
}

/** A move waiting on `asked`'s sign-in on pool `pool`'s runner, with what signs it in. */
function WaitsOnASignIn({
  plane,
  session,
  pool,
  asked,
  words,
}: {
  plane: PlaneId;
  session: number;
  pool: string;
  asked: MoveSignIn;
  words: string;
}) {
  // The strip, once asked for, and the plan it signs in to where the harness serves several.
  const [open, setOpen] = useState<{ plan?: string }>();
  // Choose another plan goes with the strip it was pressed under: the keyboard moves to the first
  // plan, once (#4278).
  const firstPlan = useRef<HTMLButtonElement>(null);
  const leftAPlan = useRef(false);
  useEffect(() => {
    if (open !== undefined || !leftAPlan.current) return;
    leftAPlan.current = false;
    firstPlan.current?.focus();
  }, [open]);
  // A login approved in the browser has no way back to the runner from a terminal: it runs its
  // device-code flow there (#4044).
  const inTerminal = (plan?: string) =>
    askProfileSignIn(plane, asked.profile, plan, pool, approvesIn(asked.kind, plan) || undefined);
  const toPlan = (plan: PlanRow): NoticeAction => ({
    label: `Sign in to ${plan.title}`,
    onPress: drivenPlan(asked.kind, plan.id)
      ? () => setOpen({ plan: plan.id })
      : () => inTerminal(plan.id),
  });
  const [onePlan, ...otherPlans] = asked.plans;
  const signIn: NoticeAction =
    onePlan !== undefined
      ? { ...toPlan(onePlan), button: firstPlan }
      : {
          label: "Sign in",
          onPress: DRIVEN.has(asked.kind) ? () => setOpen({}) : () => inTerminal(),
        };
  const anotherPlan = {
    label: "Choose another plan",
    onPress: () => {
      leftAPlan.current = true;
      setOpen(undefined);
    },
  };
  const tryIt = { label: "Try the move", onPress: () => signedInForMove(plane, session) };
  const cancel = { label: "Cancel move", onPress: () => putAwayMove(plane, session) };
  return (
    <Notice
      cause={`chat-move:${session}`}
      at="pane"
      tone="attention"
      label="Move"
      fixes={
        open === undefined
          ? [signIn, ...otherPlans.map(toPlan), tryIt, cancel]
          : open.plan === undefined
            ? [tryIt, cancel]
            : [anotherPlan, tryIt, cancel]
      }
      under={
        open ? (
          <PoolSignIn
            pool={pool}
            profile={asked.profile}
            kind={asked.kind}
            harness={asked.harness}
            provider={open.plan}
            onSignedIn={() => signedInForMove(plane, session)}
            // Closed without signing in: the move cannot go ahead, so it is cancelled.
            onEnd={(signedIn) => {
              if (!signedIn) putAwayMove(plane, session);
            }}
            // The strip's login ends as it closes, so only one runs.
            onTerminal={() => {
              setOpen(undefined);
              inTerminal(open.plan);
            }}
          />
        ) : undefined
      }
    >
      {words}
    </Notice>
  );
}
