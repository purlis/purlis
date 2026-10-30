// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type NeedsApproval, type StartRefused } from "./bindings";
import type { Ran, SaidWay } from "./actions";
import { ChatAsk } from "./ChatAsk";
import { ApprovalSentence, ProfileMeta } from "./ProfileApproval";

/**
 * **Approve and start, in place** (#3942, #3968): the starts the core makes on a profile without
 * the new-chat picker (the Skill studio's, a curation action, a theme chat, Start a chat here)
 * are refused on a profile nobody approved on this machine. Where that is the refusal, its notice
 * offers the picker's own approval (ADR 0022, ruling V69): a question that shows the profile's
 * command in the picker's sentence, whose answer records the approval of that line and starts the
 * same chat again. Nothing is approved without that answer.
 *
 * What needs approving is what the core read of the profile as it refused the start
 * (`StartRefused::approval`), never the refusal's words, as a waiting chat's Review and approve…
 * reads it.
 */

/** The answer that approves the profile's command and starts the chat. */
export const APPROVE_AND_START = "Approve and start";

/** A start, as Approve and start… names it and starts it again. */
export type StartAgain = {
  /** What starts, as the question names it: "Try it", "Start a chat here". */
  named: string;
  /** What the chat starts with, typed and not sent: "its request", "its prompt". */
  typed: string;
  /** The same start again, on the profile just approved. */
  again: (profile: string) => void;
};

/** A start refused for its profile's approval, and the approval it needs. */
export type StartApproving = StartAgain & { approval: NeedsApproval };

/**
 * **What the window says of a start the core refused**: its words, with Approve and start…
 * where the core said the profile it was refused on waits for an approval, which asks with `ask`.
 * A refusal the window only has words for (the core could not be reached) is said alone.
 */
export function startRefused(
  refused: StartRefused | string,
  start: StartAgain,
  ask: (approving: StartApproving) => void,
): { words: string; ways?: readonly SaidWay[] } {
  const { words, approval } =
    typeof refused === "string" ? { words: refused, approval: null } : refused;
  if (approval === null) return { words };
  return {
    words,
    ways: [
      {
        label: `${APPROVE_AND_START}…`,
        primary: true,
        onPress: () => ask({ ...start, approval }),
      },
    ],
  };
}

/** {@link startRefused}, as a row's answer, which its notice says with its ways (`saidOf`). */
export function startRefusedRan(
  refused: StartRefused | string,
  start: StartAgain,
  ask: (approving: StartApproving) => void,
): Ran {
  const { words, ways } = startRefused(refused, start, ask);
  return ways === undefined ? { ok: false, refused: words } : { ok: false, refused: words, ways };
}

/**
 * **The question Approve and start… asks**: the picker's sentence, line and mark for the profile
 * (`ProfileApproval.tsx`), and an answer that records the approval of the line on screen, which
 * the core checks against the file again (`approve_profile`), then puts the question away
 * (`onApproved`) and starts the same chat again. A refusal stays in the question, and nothing
 * starts.
 */
export function StartApproveAsk({
  plane,
  asked,
  onCancel,
  onApproved,
}: {
  plane: string;
  asked: StartApproving;
  onCancel: () => void;
  /** The approval is recorded, and the start is about to run again. */
  onApproved: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [trouble, setTrouble] = useState<string>();
  const { approval } = asked;
  const approve = async () => {
    setBusy(true);
    setTrouble(undefined);
    const said = await commands
      .approveProfile(plane, approval.profile, approval.shown)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setBusy(false);
    if (said.status === "error") {
      setTrouble(said.error);
      return;
    }
    onApproved();
    asked.again(approval.profile);
  };
  return (
    <ChatAsk
      title={`Approve ${approval.profile}?`}
      says={`${asked.named} starts a chat on the profile ${approval.profile}. Approving records that purlis may run its command on this machine, then starts the chat with ${asked.typed} typed and not sent.`}
      answer={APPROVE_AND_START}
      trouble={trouble}
      busy={busy}
      onAnswer={() => void approve()}
      onCancel={onCancel}
    >
      {/* The picker's own sentence, line and mark (`ProfileApproval.tsx`, ruling V69). */}
      <ApprovalSentence row={approval} />
      <ProfileMeta row={approval} />
    </ChatAsk>
  );
}
