// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  commands,
  type Asking,
  type SkillDraft,
  type SkillLevel,
  type StudioLevel,
} from "./bindings";
import { levelFrom, levelsFor } from "./skillChanges";
import type { StudioScope } from "./skillStudio";
import { counted } from "./dispatches";
import { listen } from "./here";
import { ASKS_CHANGED } from "./permissionAsks";

/** No drafts: one value, so a read of none never draws anew. */
const NONE: readonly SkillDraft[] = [];

/**
 * **Draft skills** (SK-9, #2772; spec #2762 decisions 13, 38, 50 and 52): a chat's write to a
 * skill's files, which the core keeps on this machine for the person to try, accept or discard in
 * the Skill studio (`skill_drafts`). A draft is never an ask: the Inbox does not list it.
 *
 * The core says a draft came when the project's asks move (`asks-changed`), so the studio and a
 * chat's tab read the drafts again then, and after the person's own press. Nothing is asked while
 * `asked` is false: a chat's tab asks only while the studio's flag is on.
 */
export function useSkillDrafts(
  plane: string,
  changed: number,
  asked = true,
): readonly SkillDraft[] {
  return useSkillDraftsRead(plane, changed, asked) ?? NONE;
}

/** {@link useSkillDrafts}, with `undefined` until the drafts were first read: the studio's Drafts
 *  page holds its layout until then, rather than saying there are none (S11). */
export function useSkillDraftsRead(
  plane: string,
  changed: number,
  asked = true,
): readonly SkillDraft[] | undefined {
  const [drafts, setDrafts] = useState<readonly SkillDraft[]>();
  const [moved, setMoved] = useState(0);
  useEffect(() => {
    const listening = listen<Asking>(ASKS_CHANGED, (event) => {
      if (event.payload.plane === plane) setMoved((n) => n + 1);
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane]);
  useEffect(() => {
    if (!asked) return;
    let live = true;
    void commands
      .skillDrafts(plane)
      .then((answer) => {
        // A read that failed lists none, as before: the drafts are read again when the asks move.
        if (live)
          setDrafts(answer.status === "ok" && Array.isArray(answer.data) ? answer.data : NONE);
      })
      .catch(() => {
        if (live) setDrafts(NONE);
      });
    return () => {
      live = false;
    };
  }, [plane, changed, moved, asked]);
  return drafts;
}

/**
 * **"Ask a chat to draft a skill"** and **Try it**: the project's window opens the chat, with the
 * core's prompt typed into it and never sent (`skill_draft_chat`, `skill_draft_try`), and puts
 * its tab on the strip, in front. The studio sends it; `PlaneView` hears it.
 */
export const SKILL_CHAT = "purlis-skill-chat";

/** What the studio asked of a project's window. */
export type SkillChatAsk =
  /** `what` the skill should do, as New skill… asked it (#3684): `""` where it was not. */
  | { plane: string; kind: "draft"; level: SkillLevel; what: string }
  /** `profile` null: the project's default. */
  | { plane: string; kind: "try"; id: string; profile: string | null }
  /** "Ask the chat to redo it" (S9): typed into `chat`, the chat that drafted it, where it is
   *  open; else (`chat` null, or no tab holds it) a chat opened afresh. */
  | { plane: string; kind: "redo"; id: string; name: string; chat: number | null }
  /** "Ask a chat to write them" (#3925): the trigger tests of the skill `name` at `level`. */
  | { plane: string; kind: "triggers"; level: SkillLevel; name: string };

/** Asks the window of the project `ask.plane` to open a draft, a Try it or a trigger tests chat. */
export function askSkillChat(ask: SkillChatAsk): void {
  window.dispatchEvent(new CustomEvent<SkillChatAsk>(SKILL_CHAT, { detail: ask }));
}

/**
 * **Ask a chat to draft a skill** (decision 13), from where the person came: the studio's
 * toolbar and its start, and the Skills view's New skill… (#3266). The draft lands at the level
 * the scope's chats would get (decision 37), among the project's `levels`.
 */
export function askDraft(plane: string, scope: StudioScope, levels: readonly StudioLevel[]): void {
  askSkillChat({ plane, kind: "draft", level: levelFrom(scope, levelsFor(levels, scope)), what: "" });
}

/** What a chat's tab says of the drafts it wrote (decision 38): only that they are ready. */
export function draftReadySays(names: readonly string[], offScreen: boolean): string {
  const chat = offScreen ? "That chat" : "This chat";
  return `${chat} drafted ${counted(names.length, "skill", "skills")}: ${names.join(", ")}. Try, accept or discard each in the Skill studio.`;
}
