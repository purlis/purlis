// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { MoveSignIn, PlaneId } from "./bindings";
import {
  forgetMove,
  moveCancellable,
  moveSaid,
  putAwayMove,
  signedInForMove,
  useChatMoves,
} from "./chatMoves";
import { Notice } from "./Notice";
import { askProfileSignIn } from "./settings/links";
import { DRIVEN } from "./settings/PoolProfiles";
import { PoolSignIn } from "./settings/PoolSignIn";

/**
 * **This chat's move, on its pane** (#2048): that it is moving and moves at its next pause, with
 * Cancel move while it has not; or, where it did not move, why, until it is dismissed. The chat
 * keeps running where it is in every case.
 *
 * **A move that waits on its profile's sign-in on the runner** (#3477, spec #4023 stories 21-24,
 * prototype Variant A) says so, with Sign in, Try the move and Cancel move. Sign in opens the same
 * sign-in strip as a pool's row under its line ({@link PoolSignIn}); a harness whose sign-in the
 * window does not drive yet opens its own in a shell tab on the runner instead. Once the strip
 * says signed in, the core is told, asks the runner itself, and the move goes ahead by itself;
 * Try the move tells it the same after a sign-in elsewhere (Open in terminal). The strip's Cancel,
 * like Cancel move, cancels the move, and the chat stays here.
 */
export function MoveNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const one = useChatMoves(plane)[session];
  if (one === undefined) return null;
  const said = moveSaid(one);
  if (said.trouble)
    return (
      <Notice
        cause={`chat-move-failed:${session}`}
        at="pane"
        tone="trouble"
        label="Move"
        onDismiss={() => putAwayMove(plane, session)}
      >
        {said.words}
      </Notice>
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
  const [open, setOpen] = useState(false);
  const inTerminal = () => askProfileSignIn(plane, asked.profile, undefined, pool);
  const signIn = {
    label: "Sign in",
    onPress: DRIVEN.has(asked.kind) ? () => setOpen(true) : inTerminal,
  };
  const tryIt = { label: "Try the move", onPress: () => signedInForMove(plane, session) };
  const cancel = { label: "Cancel move", onPress: () => putAwayMove(plane, session) };
  return (
    <Notice
      cause={`chat-move:${session}`}
      at="pane"
      tone="attention"
      label="Move"
      fixes={open ? [tryIt, cancel] : [signIn, tryIt, cancel]}
      under={
        open ? (
          <PoolSignIn
            pool={pool}
            profile={asked.profile}
            harness={asked.harness}
            onSignedIn={() => signedInForMove(plane, session)}
            // Closed without signing in: the move cannot go ahead, so it is cancelled.
            onEnd={(signedIn) => {
              if (!signedIn) putAwayMove(plane, session);
            }}
            onTerminal={inTerminal}
          />
        ) : undefined
      }
    >
      {words}
    </Notice>
  );
}
