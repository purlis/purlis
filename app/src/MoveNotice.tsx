// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatMove, PlaneId } from "./bindings";
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
 * **A move that waits on its profile's sign-in on the runner** (#3477, spec #4023 stories 21-24)
 * says so, and opens the same sign-in strip as a pool's row under its line ({@link PoolSignIn}):
 * once the strip says signed in, the core is told, asks the runner itself, and the move goes
 * ahead by itself. The strip's Cancel, like Cancel move, cancels the move, and the chat stays
 * here. A harness whose sign-in the window does not drive yet offers its own Sign in, in a shell
 * tab on the runner, and Try the move once that is done.
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
  const cancel = { label: "Cancel move", onPress: () => putAwayMove(plane, session) };
  if (one.stage === "signing-in" && one.sign_in !== null && one.to !== null)
    return <WaitsOnASignIn plane={plane} session={session} one={one} words={said.words} />;
  return (
    <Notice cause={`chat-move:${session}`} at="pane" label="Move" fixes={[cancel]}>
      {said.words}
    </Notice>
  );
}

/** A move waiting on its profile's sign-in on pool `one.to`'s runner, with what signs it in. */
function WaitsOnASignIn({
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
  const pool = one.to ?? "";
  const asked = one.sign_in;
  if (asked === null) return null;
  const cancel = { label: "Cancel move", onPress: () => putAwayMove(plane, session) };
  if (!DRIVEN.has(asked.kind))
    return (
      <Notice
        cause={`chat-move:${session}`}
        at="pane"
        tone="attention"
        label="Move"
        fixes={[
          {
            label: "Sign in",
            onPress: () => askProfileSignIn(plane, asked.profile, undefined, pool),
          },
          { label: "Try the move", onPress: () => signedInForMove(plane, session) },
          cancel,
        ]}
      >
        {words}
      </Notice>
    );
  return (
    <Notice
      cause={`chat-move:${session}`}
      at="pane"
      tone="attention"
      label="Move"
      fixes={[cancel]}
      under={
        <PoolSignIn
          pool={pool}
          profile={asked.profile}
          harness={asked.harness}
          onSignedIn={() => signedInForMove(plane, session)}
          // Closed without signing in: the move cannot go ahead, so it is cancelled.
          onEnd={(signedIn) => {
            if (!signedIn) putAwayMove(plane, session);
          }}
          onTerminal={() => askProfileSignIn(plane, asked.profile, undefined, pool)}
        />
      }
    >
      {words}
    </Notice>
  );
}
