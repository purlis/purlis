// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type NeedsApproval, type StudioRefused } from "./bindings";
import type { Said } from "./actions";
import { ChatAsk } from "./ChatAsk";
import { ApprovalSentence, ProfileMeta } from "./ProfileApproval";
import type { SkillChatAsk } from "./skillDraftAsks";

/**
 * **Approve and start, in place** (#3942): the Skill studio's "Ask a chat to draft a skill", Try
 * it and a draft's redo start their chats in the core, without the new-chat picker, and the core
 * refuses a profile nobody approved on this machine. Where that is the refusal, its notice offers
 * the picker's own approval (ADR 0022, ruling V69): a question that shows the profile's command in
 * the picker's sentence, whose answer records the approval of that line and starts the same chat
 * again, with its prompt typed and not sent. Nothing is approved without that answer.
 *
 * What needs approving is what the core read of the profile as it refused the start
 * (`StudioRefused::approval`), never the refusal's words, as a waiting chat's Review and approve…
 * reads it.
 */

/** The answer that approves the profile's command and starts the chat. */
export const APPROVE_AND_START = "Approve and start";

/** A studio start refused for its profile's approval, and the approval it needs. */
export type SkillChatApproving = { wanted: SkillChatAsk; approval: NeedsApproval };

/** What `wanted` starts, as the question names it. */
export function studioStartNamed(wanted: SkillChatAsk): string {
  switch (wanted.kind) {
    case "draft":
      return "Ask a chat to draft a skill";
    case "try":
      return "Try it";
    case "redo":
      return `Ask a chat to redo ${wanted.name}`;
  }
}

/** The same start again, on the profile just approved: Try it's, which picks one, on that one. */
export function startedAgain(wanted: SkillChatAsk, profile: string): SkillChatAsk {
  return wanted.kind === "try" ? { ...wanted, profile } : wanted;
}

/**
 * **What the window says of a studio start the core refused**: its words, with Approve and start…
 * where the core said the profile it was refused on waits for an approval, which asks with `ask`.
 * A refusal the window only has words for (the core could not be reached) is said alone.
 */
export function skillChatRefused(
  wanted: SkillChatAsk,
  refused: StudioRefused | string,
  ask: (approving: SkillChatApproving) => void,
): Said {
  const { words, approval } =
    typeof refused === "string" ? { words: refused, approval: null } : refused;
  const said: Said = { from: "skill-chat", refused: true, words };
  if (approval === null) return said;
  return {
    ...said,
    ways: [
      { label: `${APPROVE_AND_START}…`, primary: true, onPress: () => ask({ wanted, approval }) },
    ],
  };
}

/**
 * **The question Approve and start… asks**: the picker's sentence, line and mark for the profile
 * (`ProfileApproval.tsx`), and an answer that records the approval of the line on screen, which
 * the core checks against the file again (`approve_profile`), and then hands the start to
 * `onApproved`. A refusal stays in the question, and nothing starts.
 */
export function SkillChatApproveAsk({
  plane,
  asked,
  onCancel,
  onApproved,
}: {
  plane: string;
  asked: SkillChatApproving;
  onCancel: () => void;
  /** The approval is recorded: start this. */
  onApproved: (again: SkillChatAsk) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [trouble, setTrouble] = useState<string>();
  const { wanted, approval } = asked;
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
    onApproved(startedAgain(wanted, approval.profile));
  };
  return (
    <ChatAsk
      title={`Approve ${approval.profile}?`}
      says={`${studioStartNamed(wanted)} starts a chat on the profile ${approval.profile}. Approving records that purlis may run its command on this machine, then starts the chat with its request typed and not sent.`}
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
