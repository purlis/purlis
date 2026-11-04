// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { SkillDraft, StudioLink } from "./bindings";
import {
  chatsOfSkill,
  chipSaid,
  doneByDrafts,
  isDone,
  jobSaid,
  linkOfAsk,
  scopeOfLevel,
  studioPlaceOf,
  type StudioChat,
} from "./studioChats";

const DEMO = { at: "workspace", name: "purlis-demo", local: false } as const;
const EVALS: StudioLink = {
  skill: "test-design",
  level: DEMO,
  job: "outcome-evals",
  done: false,
};

const chat = (session: number, link: StudioLink): StudioChat => ({
  session,
  name: `chat ${session}`,
  link,
});

const draft = (chat: number | null): SkillDraft => ({
  id: "d1",
  name: "test-design",
  level: DEMO,
  chat,
  chat_open: false,
  new: false,
  stale: false,
  files: [],
});

describe("what a studio chat says of its job (#5185)", () => {
  it("says the job while it runs, and what it did once it is done", () => {
    expect(jobSaid("outcome-evals", false)).toBe("writing outcome evals");
    expect(jobSaid("outcome-evals", true)).toBe("wrote outcome evals");
    expect(jobSaid("trigger-tests", false)).toBe("writing trigger tests");
    expect(jobSaid("checks", true)).toBe("wrote checks");
    expect(jobSaid("improve", false)).toBe("drafting an edit from its runs");
    expect(jobSaid("draft", false)).toBe("drafting a skill");
    expect(jobSaid("redo", true)).toBe("redid its draft");
  });

  it("names the skill and the job on the chat's chip", () => {
    expect(chipSaid(EVALS, false)).toBe("test-design · writing outcome evals");
    expect(chipSaid({ skill: null, level: DEMO, job: "draft", done: false }, true)).toBe(
      "New skill · drafted a skill",
    );
  });
});

describe("which chats are a skill's (#5185)", () => {
  it("lists the chats started for this skill at this level, and no other's", () => {
    const chats = [
      chat(3, EVALS),
      chat(4, { ...EVALS, level: { at: "project", local: false } }),
      chat(5, { ...EVALS, skill: "deploy" }),
      chat(6, { skill: null, level: DEMO, job: "draft", done: false }),
    ];

    expect(chatsOfSkill(chats, "test-design", DEMO).map((one) => one.session)).toEqual([3]);
  });
});

describe("where a studio chat's link opens (#5185)", () => {
  it("opens the skill's page on the chats of the scope where it lives", () => {
    expect(scopeOfLevel(DEMO)).toEqual({ at: "workspace", name: "purlis-demo" });
    expect(scopeOfLevel({ at: "persona", name: "steward", local: true })).toEqual({
      at: "persona",
      name: "steward",
    });
    expect(scopeOfLevel({ at: "project", local: false })).toEqual({ at: "root", name: "" });
  });

  it("opens the Grades tab for checks, Runs for an edit, and Drafts for a new skill", () => {
    expect(studioPlaceOf(EVALS)).toEqual({ page: "library", skill: "test-design", tab: "grades" });
    expect(studioPlaceOf({ ...EVALS, job: "improve" })).toEqual({
      page: "library",
      skill: "test-design",
      tab: "runs",
    });
    expect(studioPlaceOf({ skill: null, level: DEMO, job: "draft", done: false })).toEqual({
      page: "drafts",
    });
  });
});

describe("the link a studio start leaves on its chat (#5185)", () => {
  it("is the ask's skill, level and job; a Try it leaves none", () => {
    expect(linkOfAsk({ plane: "p", kind: "evals", level: DEMO, name: "test-design" })).toEqual(
      EVALS,
    );
    expect(linkOfAsk({ plane: "p", kind: "draft", level: DEMO, what: "" })).toEqual({
      skill: null,
      level: DEMO,
      job: "draft",
      done: false,
    });
    expect(
      linkOfAsk({ plane: "p", kind: "redo", id: "d1", name: "test-design", level: DEMO, chat: 3 }),
    ).toEqual({ skill: "test-design", level: DEMO, job: "redo", done: false });
    expect(linkOfAsk({ plane: "p", kind: "try", id: "d1", profile: null })).toBeUndefined();
  });

  it("is done once a draft its chat wrote waits, or once its program has ended", () => {
    expect(isDone(chat(3, EVALS), [draft(3)], false)).toBe(true);
    expect(isDone(chat(3, EVALS), [draft(4), draft(null)], false)).toBe(false);
    expect(isDone(chat(3, EVALS), [], true)).toBe(true);
  });

  it("stays done once its draft came, after the draft is accepted or discarded", () => {
    const links = { 3: EVALS, 4: { ...EVALS, job: "trigger-tests" as const } };

    const read = doneByDrafts(links, [draft(3)]);
    expect(read[3].done).toBe(true);
    expect(read[4].done).toBe(false);
    // The draft is gone (accepted, or discarded): the chat is still done.
    expect(isDone(chat(3, read[3]), [], false)).toBe(true);
    // A read that changes nothing is the same value, so nothing is drawn anew.
    expect(doneByDrafts(read, [draft(3)])).toBe(read);
  });
});
