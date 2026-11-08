// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import {
  commands,
  type PlaneId,
  type SigningLane,
  type SigningRetried,
  type SigningWaiting,
} from "./bindings";
import { useChatsHere, useChatsSelect } from "./chatState";
import { ACCOUNTS_CHANGED } from "./ForgeSignInEnded";
import { Notice } from "./Notice";
import { SigningSetupDialog } from "./SigningSetupDialog";

/** Told on `window` whenever work waiting on signing may have moved: set up, signed or sent. */
export const SIGNING_WAITING_CHANGED = "purlis:signing-waiting-changed";

/** Each chat's state, as one string: a turn that pushed and ended changes it. */
const chatsMoved = (states: { bySession: Readonly<Record<number, unknown>> }) =>
  Object.entries(states.bySession)
    .map(([session, state]) => `${session}:${JSON.stringify(state)}`)
    .join(",");

/** The lane's key: its workspace, repo and piece. */
export const laneOf = (one: SigningLane): SigningLane => ({
  ws: one.ws,
  repo: one.repo,
  piece: one.piece,
});

const sameLane = (one: SigningLane, other: SigningLane) =>
  one.ws === other.ws && one.repo === other.repo && one.piece === other.piece;

/**
 * **The work in `plane` waiting on signing, and whether signing is on** (#5630): read as it
 * mounts, again as the project's chats move (a push refused ends a turn), and whenever setup,
 * Sign and retry or an account changed it.
 */
export function useSigningWaiting(plane: PlaneId): {
  waiting?: readonly SigningWaiting[];
  on?: boolean;
} {
  const moved = useChatsSelect(useChatsHere(), chatsMoved);
  const [waiting, setWaiting] = useState<readonly SigningWaiting[]>();
  const [on, setOn] = useState<boolean>();
  const [asked, setAsked] = useState(0);
  useEffect(() => {
    const again = () => setAsked((was) => was + 1);
    window.addEventListener(SIGNING_WAITING_CHANGED, again);
    window.addEventListener(ACCOUNTS_CHANGED, again);
    return () => {
      window.removeEventListener(SIGNING_WAITING_CHANGED, again);
      window.removeEventListener(ACCOUNTS_CHANGED, again);
    };
  }, []);
  useEffect(() => {
    let live = true;
    void commands
      .signingWaiting(plane)
      .then((read) => {
        if (live && read.status === "ok" && Array.isArray(read.data)) setWaiting(read.data);
      })
      // Nothing read is nothing waiting.
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, moved, asked]);
  const anyWaiting = (waiting?.length ?? 0) > 0;
  useEffect(() => {
    // The Keychain is not asked: only whether this Mac has a key.
    if (!anyWaiting) return;
    let live = true;
    void commands
      .signingState()
      .then((read) => {
        if (live && read.status === "ok" && read.data) setOn(read.data.on);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [anyWaiting, asked]);
  return { waiting, on };
}

/**
 * **Sign and retry `lanes`** (ADR 0116 §4): each signed with this Mac's key and sent again, and
 * every reader told. What each came to, or the one sentence why none was tried.
 */
export async function signAndRetry(
  plane: PlaneId,
  lanes: readonly SigningLane[],
): Promise<readonly SigningRetried[] | string> {
  const done = await commands.signingSignAndRetry(plane, lanes.map(laneOf));
  window.dispatchEvent(new Event(SIGNING_WAITING_CHANGED));
  return done.status === "ok" ? done.data : done.error;
}

/** How one lane's Sign and retry is going, as its prompt says it. */
type Retry =
  | { at: "idle" }
  | { at: "running" }
  | { at: "sent"; signed: number }
  | { at: "failed"; why: string };

/** What a lane's prompt says once its commits went again. */
export function sentSaid(signed: number | undefined, branch: string): string {
  if (signed === undefined || signed === 0) return `${branch} was signed and sent again.`;
  return `${signed} ${signed === 1 ? "commit" : "commits"} re-signed, and ${branch} sent again.`;
}

/**
 * **A forge's refusal of unsigned commits, where it happened** (#5630, ADR 0116 §3): one
 * component, on the lane's row in Tasks and in the chat. It says *<Forge> only accepts signed
 * commits here* and offers **Set up signing**, the same dialog as everywhere, whose last step is
 * **Sign and retry**; with signing on already, Sign and retry itself. Then *Signed and sent ✓*.
 * In the chat it says too that the chat can't set signing up: it's the person's, here.
 */
export function SigningRefusal({
  plane,
  lane,
  on,
  where,
}: {
  plane: PlaneId;
  lane: SigningWaiting;
  /** Whether this Mac has a signing key: Sign and retry is offered at once. */
  on: boolean | undefined;
  where: "lane" | "chat";
}) {
  const [retry, setRetry] = useState<Retry>({ at: "idle" });
  const [setUp, setSetUp] = useState(false);
  const go = useCallback(() => {
    setRetry({ at: "running" });
    void signAndRetry(plane, [lane]).then((done) => {
      const mine = typeof done === "string" ? undefined : done.find((one) => sameLane(one, lane));
      if (typeof done === "string") setRetry({ at: "failed", why: done });
      else if (mine?.why) setRetry({ at: "failed", why: mine.why });
      else setRetry({ at: "sent", signed: mine?.signed ?? 0 });
    });
  }, [plane, lane]);
  const cause = `signing-refused:${lane.ws}/${lane.repo}/${lane.piece}:${where}`;
  const dialog = setUp && (
    <SigningSetupDialog
      forge={{ kind: lane.kind, host: lane.host }}
      repo={lane.path}
      commit={lane.commit}
      finish={{ label: "Sign and retry", onFinish: go }}
      onClose={() => {
        setSetUp(false);
        window.dispatchEvent(new Event(SIGNING_WAITING_CHANGED));
      }}
    />
  );
  if (retry.at === "sent" || (lane.sent && retry.at === "idle"))
    return (
      <Notice
        cause={cause}
        at="pane"
        tone="news"
        label="Signed and sent ✓"
        onDismiss={() =>
          void commands
            .signingForgetSent(plane)
            .then(() => window.dispatchEvent(new Event(SIGNING_WAITING_CHANGED)))
        }
      >
        {sentSaid(retry.at === "sent" ? retry.signed : undefined, lane.branch)}
      </Notice>
    );
  const theirs = where === "chat" ? "this chat's" : "this lane's";
  return (
    <>
      <Notice
        cause={cause}
        at="pane"
        tone={retry.at === "failed" ? "trouble" : "attention"}
        label={`${lane.forge} only accepts signed commits here`}
        running={retry.at === "running"}
        fixes={
          on
            ? [
                {
                  label: "Sign and retry",
                  onPress: go,
                  busy: retry.at === "running",
                  primary: true,
                },
              ]
            : [{ label: "Set up signing", onPress: () => setSetUp(true), primary: true }]
        }
      >
        {on
          ? `Signing is set up. Sign and retry signs ${theirs} own commits on ${lane.branch} and sends them again; nothing already pushed is rewritten.`
          : `${lane.path} requires signed commits, so ${lane.forge} refused ${lane.branch}. Set up signing on this Mac, then purlis signs ${theirs} commits and sends them again.`}
        {where === "chat" &&
          !on &&
          " The chat can't set signing up itself: it's yours to do, here."}
        {retry.at === "failed" && ` It wasn't sent: ${retry.why}`}
      </Notice>
      {dialog}
    </>
  );
}

/** **This chat's work waiting on signing** (#5630): its prompt, in the chat's notices. */
export function SigningRefusalNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const { waiting, on } = useSigningWaiting(plane);
  return (
    <>
      {(waiting ?? [])
        .filter((one) => one.chat === session)
        .map((one) => (
          <SigningRefusal
            key={`${one.ws}/${one.repo}/${one.piece}`}
            plane={plane}
            lane={one}
            on={on}
            where="chat"
          />
        ))}
    </>
  );
}
