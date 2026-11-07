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
  finishedSaid,
  linkOfAsk,
  runningSaid,
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
  started: null,
  finished: null,
};
const UNTIMED = { started: null, finished: null } as const;

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
    expect(
      chipSaid({ skill: null, level: DEMO, job: "draft", done: false, ...UNTIMED }, true),
    ).toBe("New skill · drafted a skill");
  });
});

describe("which chats are a skill's (#5185)", () => {
  it("lists the chats started for this skill at this level, and no other's", () => {
    const chats = [
      chat(3, EVALS),
      chat(4, { ...EVALS, level: { at: "project", local: false } }),
      chat(5, { ...EVALS, skill: "deploy" }),
      chat(6, { skill: null, level: DEMO, job: "draft", done: false, ...UNTIMED }),
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
    expect(
      studioPlaceOf({ skill: null, level: DEMO, job: "draft", done: false, ...UNTIMED }),
    ).toEqual({
      page: "drafts",
    });
  });
});

describe("the link a studio start leaves on its chat (#5185)", () => {
  it("is the ask's skill, level and job, started now; a Try it leaves none", () => {
    const now = new Date("2026-11-04T17:31:00.250Z");
    const started = { started: "2026-11-04T17:31:00Z", finished: null };
    expect(linkOfAsk({ plane: "p", kind: "evals", level: DEMO, name: "test-design" }, now)).toEqual(
      { ...EVALS, ...started },
    );
    expect(linkOfAsk({ plane: "p", kind: "draft", level: DEMO, what: "" }, now)).toEqual({
      skill: null,
      level: DEMO,
      job: "draft",
      done: false,
      ...started,
    });
    expect(
      linkOfAsk(
        { plane: "p", kind: "redo", id: "d1", name: "test-design", level: DEMO, chat: 3 },
        now,
      ),
    ).toEqual({ skill: "test-design", level: DEMO, job: "redo", done: false, ...started });
    expect(linkOfAsk({ plane: "p", kind: "try", id: "d1", profile: null }, now)).toBeUndefined();
  });

  it("is done once a draft its chat wrote waits, or once its program has ended", () => {
    expect(isDone(chat(3, EVALS), [draft(3)], false)).toBe(true);
    expect(isDone(chat(3, EVALS), [draft(4), draft(null)], false)).toBe(false);
    expect(isDone(chat(3, EVALS), [], true)).toBe(true);
  });

  it("stays done once its draft came, after the draft is accepted or discarded", () => {
    const links = { 3: EVALS, 4: { ...EVALS, job: "trigger-tests" as const } };

    const read = doneByDrafts(links, [draft(3)], new Date("2026-11-04T17:42:00Z"));
    expect(read[3].done).toBe(true);
    expect(read[3].finished).toBe("2026-11-04T17:42:00Z");
    expect(read[4].done).toBe(false);
    // The draft is gone (accepted, or discarded): the chat is still done.
    expect(isDone(chat(3, read[3]), [], false)).toBe(true);
    // A read that changes nothing is the same value, so nothing is drawn anew.
    expect(doneByDrafts(read, [draft(3)])).toBe(read);
  });
});

describe("how a studio chat's times are said (#5248)", () => {
  const now = new Date(2026, 10, 5, 9, 30);

  it("says when it finished: its clock time today, and its day before", () => {
    expect(finishedSaid(new Date(2026, 10, 5, 9, 4).toISOString(), now)).toBe("finished 09:04");
    expect(finishedSaid(new Date(2026, 10, 4, 17, 42).toISOString(), now)).toBe(
      "finished 11-04 17:42",
    );
    expect(finishedSaid(null, now)).toBeUndefined();
    expect(finishedSaid("soon", now)).toBeUndefined();
  });

  it("says how long it has run, in whole minutes", () => {
    const at = (ms: number) => new Date(now.getTime() - ms).toISOString();
    expect(runningSaid(at(20_000), now.getTime())).toBe("running under a minute");
    expect(runningSaid(at(2 * 60_000 + 59_000), now.getTime())).toBe("running 2 min");
    expect(runningSaid(at(65 * 60_000), now.getTime())).toBe("running 1 h 5 min");
    expect(runningSaid(null, now.getTime())).toBe("running");
  });
});
