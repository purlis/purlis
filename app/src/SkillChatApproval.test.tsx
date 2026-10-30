// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Approve and start, in place, for the Skill studio's starts** (#3942): what a studio start the
 * core refused offers, and the start it runs again once the profile is approved. The question
 * itself is `StartApproval.test.tsx`'s.
 */
import { describe, expect, it, vi } from "vitest";
import type { NeedsApproval } from "./bindings";
import type { SkillChatAsk } from "./skillDraftAsks";
import { skillChatRefused, startedAgain, studioStartNamed } from "./SkillChatApproval";

const PLANE = "/home/dev/plane";
const REFUSED =
  "profile 'work' is new, and nobody has approved it — nothing was started. Approve it once so purlis can run it.";

const WORK: NeedsApproval = {
  profile: "work",
  kind: "claude",
  source: "purlis.local.toml",
  approval: "new",
  shown: "CLAUDE_CONFIG_DIR=~/.claude-work claude",
};

const TRY: SkillChatAsk = { plane: PLANE, kind: "try", id: "d1", profile: null };
const DRAFT: SkillChatAsk = { plane: PLANE, kind: "draft", level: { at: "project", local: false } };

describe("a studio start refused for its profile's approval", () => {
  it("offers Approve and start…, which asks first and then starts the same chat again", () => {
    const ask = vi.fn();
    const again = vi.fn();
    const said = skillChatRefused(TRY, { words: REFUSED, approval: WORK }, ask, again);

    expect(said).toMatchObject({ from: "skill-chat", refused: true, words: REFUSED });
    expect(said.ways?.map((one) => one.label)).toEqual(["Approve and start…"]);
    said.ways?.[0].onPress();
    expect(ask).toHaveBeenCalledWith(
      expect.objectContaining({ named: "Try it", typed: "its request", approval: WORK }),
    );
    expect(again).not.toHaveBeenCalled();

    // The question's answer starts it again: Try it on the profile just approved.
    const [[asked]] = ask.mock.calls as [[{ again: (profile: string) => void }]];
    asked.again("work");
    expect(again).toHaveBeenCalledWith({ ...TRY, profile: "work" });
  });

  it("says the refusal alone where there is nothing to approve, or only its words came", () => {
    expect(
      skillChatRefused(DRAFT, { words: "no profile", approval: null }, vi.fn(), vi.fn()),
    ).toEqual({ from: "skill-chat", refused: true, words: "no profile" });
    // A refusal that names approval in its words alone offers nothing: the words are not read.
    expect(skillChatRefused(DRAFT, REFUSED, vi.fn(), vi.fn())).toEqual({
      from: "skill-chat",
      refused: true,
      words: REFUSED,
    });
  });
});

describe("what it names and starts again", () => {
  it("names each studio start, and starts Try it again on the approved profile", () => {
    expect(studioStartNamed(DRAFT)).toBe("Ask a chat to draft a skill");
    expect(studioStartNamed(TRY)).toBe("Try it");
    expect(
      studioStartNamed({ plane: PLANE, kind: "redo", id: "d1", name: "deploy", chat: null }),
    ).toBe("Ask a chat to redo deploy");
    expect(startedAgain(TRY, "work")).toEqual({ ...TRY, profile: "work" });
    expect(startedAgain(DRAFT, "work")).toBe(DRAFT);
  });
});
