// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { LoaderCircle, Sparkles } from "lucide-react";
import { EmptyState } from "./ui/empty-state/EmptyState";

/** New skill's words, on the studio's toolbar and the Skills view alike. */
export const NEW_SKILL = "New skill";

/** **New skill…** (#3684, S4): a dialog that asks how first, so its words take an ellipsis, in
 *  the studio's top bar and the Skills view alike. */
export const NEW_SKILL_MENU = `${NEW_SKILL}…`;

/** The words of the act that opens a chat to draft a skill, wherever it is offered. */
export const DRAFT_WITH_A_CHAT = "Ask a chat to draft a skill";

/**
 * **What a project with no skill of its own is told** (SK-22, decision 42): the claim that it
 * has none, that its chats still get the skills listed, and where a skill is started: New
 * skill…, which asks how (#3684, S4). The three starts it once drew as buttons are that dialog's
 * ways now.
 */
export function SkillStarts({ testid = "skill-studio-empty" }: { testid?: string }) {
  return (
    <EmptyState
      headline="This project has no skills of its own yet"
      body={`Every chat still gets the skills listed here. Start one of the project's own from ${NEW_SKILL_MENU}`}
      mark={Sparkles}
      testid={testid}
    />
  );
}

/** **The project's skills could not be read**: the core's sentence, under the claim. The Skill
 *  studio and the Skills view say it alike. */
export function SkillsNotRead({ refused, testid }: { refused: string; testid: string }) {
  return (
    <EmptyState
      headline="purlis could not read this project's skills"
      body={refused}
      testid={testid}
    />
  );
}

/** **The project's skills, being read**: what the Skill studio and the Skills view say meanwhile. */
export function SkillsReading() {
  return (
    <p className="pending" aria-busy="true">
      <LoaderCircle className="node-icon spinning" />
      Reading the project&apos;s skills…
    </p>
  );
}
