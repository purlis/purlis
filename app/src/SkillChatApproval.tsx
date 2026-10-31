// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { StartRefused } from "./bindings";
import type { Said } from "./actions";
import type { SkillChatAsk } from "./skillDraftAsks";
import { startRefused, type StartApproving } from "./StartApproval";

/**
 * **Approve and start, in place, for the Skill studio's starts** (#3942): "Ask a chat to draft a
 * skill", Try it and a draft's redo start their chats in the core, without the new-chat picker.
 * Refused on a profile nobody approved, their notice offers Approve and start…
 * (`StartApproval.tsx`), which starts the same chat again, with its request typed and not sent.
 */

/** What `wanted` starts, as the question names it. */
export function studioStartNamed(wanted: SkillChatAsk): string {
  switch (wanted.kind) {
    case "draft":
      return "Ask a chat to draft a skill";
    case "try":
      return "Try it";
    case "redo":
      return `Ask a chat to redo ${wanted.name}`;
    case "triggers":
      return `Ask a chat to write the trigger tests of ${wanted.name}`;
    case "evals":
      return `Ask a chat to write the outcome evals of ${wanted.name}`;
    case "improve":
      return `Improve ${wanted.name} from runs`;
  }
}

/** The same start again, on the profile just approved: Try it's, which picks one, on that one. */
export function startedAgain(wanted: SkillChatAsk, profile: string): SkillChatAsk {
  return wanted.kind === "try" ? { ...wanted, profile } : wanted;
}

/**
 * **What the window says of a studio start the core refused**: its words, with Approve and start…
 * where the core said the profile it was refused on waits for an approval, which asks with `ask`
 * and, once approved, starts it again with `again`.
 */
export function skillChatRefused(
  wanted: SkillChatAsk,
  refused: StartRefused | string,
  ask: (approving: StartApproving) => void,
  again: (wanted: SkillChatAsk) => void,
): Said {
  const said = startRefused(
    refused,
    {
      named: studioStartNamed(wanted),
      typed: "its request",
      again: (profile) => again(startedAgain(wanted, profile)),
    },
    ask,
  );
  return { from: "skill-chat", refused: true, ...said };
}
