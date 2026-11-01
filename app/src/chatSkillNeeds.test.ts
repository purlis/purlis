// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The chat in front's needs as a whole (S8, #3694): which it has, in what order, and the one
 *  fix each offers. */
import { describe, expect, it } from "vitest";
import type { PlaneUpdated, SkillBudget } from "./bindings";
import { changedSays, chatNeedsOf, chatSetSaid, type ChatSkillsSaid } from "./chatSkillNeeds";
import { AT_THE_ROOT, asPersona, inWorkspace } from "./skillStudio";

const BUDGET: SkillBudget = {
  skills: 3,
  chars: 1234,
  manual: 0,
  most_skills: 30,
  most_chars: 6000,
  hidden: 0,
  still_over: null,
  pinned_by: [],
  pins_together: null,
  warns: null,
};

const NOTHING: ChatSkillsSaid = { budget: BUDGET, routing: null, missing: null, changed: null };

const CHANGED: PlaneUpdated = {
  session: 4,
  files: [],
  skills: true,
  skills_added: ["triage"],
  skills_gone: [],
  skills_edited: [],
};

describe("a chat's needs as a whole", () => {
  it("are none where its set is well, and its header says its budget", () => {
    expect(chatNeedsOf(NOTHING, inWorkspace("ops"))).toEqual([]);
    expect(chatSetSaid(NOTHING)).toEqual(["3 skills of 30 · 1,234 of 6,000 characters"]);
    expect(chatSetSaid({ ...NOTHING, budget: null })).toEqual([]);
  });

  it("says its program's own catalogue after the budget, and not where it has no budget (#3324)", () => {
    const own = "Codex's own, not counted: 4 skills, 900 characters";
    expect(chatSetSaid({ ...NOTHING, own })).toEqual([
      "3 skills of 30 · 1,234 of 6,000 characters",
      own,
    ]);
    expect(chatSetSaid({ ...NOTHING, budget: null, own })).toEqual([]);
    expect(chatSetSaid({ ...NOTHING, own: null })).toEqual([
      "3 skills of 30 · 1,234 of 6,000 characters",
    ]);
  });

  it("go budget, router, started without, then changed, each with its fix", () => {
    const needs = chatNeedsOf(
      {
        budget: { ...BUDGET, warns: "chars" },
        routing: { router: "acme", ms: 9, fell_back: true, said: "acme fell back." },
        missing: {
          persona: null,
          missing: [],
          no_built_in: [],
          broken: [{ name: "half", level: "project", local: false, label: "project", why: "x" }],
          by_a_repo: [],
          beside_a_built_in: [],
        },
        changed: CHANGED,
      },
      inWorkspace("ops"),
    );
    expect(needs.map((one) => one.id)).toEqual(["budget", "routing", "missing", "changed"]);
    expect(needs[0].fix).toEqual({
      act: "studio",
      label: "Open routing",
      scope: inWorkspace("ops"),
      page: "routing",
    });
    // A chat on no persona's missing skills are seen at the project's root.
    expect(needs[2].fix).toMatchObject({ scope: AT_THE_ROOT, page: "library" });
    expect(needs[2].title).toBe("Started without 1 skill");
    expect(needs[3].fix).toEqual({ act: "restart", label: "Restart" });
    // A fallen-back router is a need, so the header leaves it out.
    expect(
      chatSetSaid({ ...NOTHING, routing: { router: "a", ms: 1, fell_back: true, said: "x" } }),
    ).toEqual(["3 skills of 30 · 1,234 of 6,000 characters"]);
  });

  it("make a prompt the router fell back on a need, and a prompt it answered a line of the header (#4297)", () => {
    const start = { router: "acme", ms: 9, fell_back: false, said: "Routed in 9 ms." };
    const slow = {
      ms: 101,
      fell_back: true,
      said: "Its last prompt went on with no pointer: 'acme' did not answer within 100 ms.",
    };
    const [need] = chatNeedsOf(
      { ...NOTHING, routing: { ...start, last_prompt: slow } },
      AT_THE_ROOT,
    );
    expect(need.id).toBe("routing");
    expect(need.title).toBe("Its router fell back on a prompt");
    expect(need.says).toBe(slow.said);
    expect(need.fix).toMatchObject({ act: "studio", page: "routing" });
    expect(chatSetSaid({ ...NOTHING, routing: { ...start, last_prompt: slow } })).toEqual([
      "3 skills of 30 · 1,234 of 6,000 characters",
      "Routed in 9 ms.",
    ]);

    const quick = { ms: 4, fell_back: false, said: "Its last prompt took 4 ms." };
    expect(
      chatNeedsOf({ ...NOTHING, routing: { ...start, last_prompt: quick } }, AT_THE_ROOT),
    ).toEqual([]);
    expect(chatSetSaid({ ...NOTHING, routing: { ...start, last_prompt: quick } })).toEqual([
      "3 skills of 30 · 1,234 of 6,000 characters",
      "Routed in 9 ms.",
      "Its last prompt took 4 ms.",
    ]);
  });

  it("say both where a chat's start and its last prompt fell back, in one need (#4297)", () => {
    const needs = chatNeedsOf(
      {
        ...NOTHING,
        routing: {
          router: "acme",
          ms: 9,
          fell_back: true,
          said: "acme fell back.",
          last_prompt: { ms: 101, fell_back: true, said: "Its last prompt went on." },
        },
      },
      AT_THE_ROOT,
    );
    expect(needs).toHaveLength(1);
    expect(needs[0].title).toBe("Its router fell back");
    expect(needs[0].says).toBe("acme fell back. Its last prompt went on.");
  });

  it("send a persona's missing skills to that persona's studio", () => {
    const [need] = chatNeedsOf(
      {
        ...NOTHING,
        missing: {
          persona: "ops",
          missing: ["tdd"],
          no_built_in: [],
          broken: [],
          by_a_repo: [],
          beside_a_built_in: [],
        },
      },
      AT_THE_ROOT,
    );
    expect(need.fix).toMatchObject({ scope: asPersona("ops") });
  });

  it("say what changed by name, and only what did", () => {
    expect(changedSays({ ...CHANGED, skills_gone: ["old"], skills_edited: ["deploy"] })).toBe(
      "This chat's skills changed since it started (new: triage; gone: old; changed: deploy). It keeps the skills it started with until it restarts on the same conversation.",
    );
    expect(changedSays({ ...CHANGED, skills_added: [] })).toBe(
      "This chat's skills changed since it started. It keeps the skills it started with until it restarts on the same conversation.",
    );
  });

  it("send a budget past its most where its sentence says it is fixed (review of #3694)", () => {
    const pins = chatNeedsOf(
      {
        ...NOTHING,
        budget: {
          ...BUDGET,
          warns: "skills",
          still_over: "pins",
          pinned_by: [{ at: "persona", name: "reviewer" }],
        },
      },
      inWorkspace("ops"),
    );
    expect(pins[0].fix).toMatchObject({ scope: asPersona("reviewer"), page: "library" });
    const own = chatNeedsOf(
      { ...NOTHING, budget: { ...BUDGET, warns: "skills", still_over: "not-hideable" } },
      inWorkspace("ops"),
    );
    expect(own[0].fix).toMatchObject({ label: "Open Harness skills", page: "harness" });
  });

  it("count only what the chat started without, not a repo's copy of a built-in skill", () => {
    const [need] = chatNeedsOf(
      {
        ...NOTHING,
        missing: {
          persona: null,
          missing: [],
          no_built_in: [],
          broken: [],
          by_a_repo: [],
          beside_a_built_in: ["secrets", "browser"],
        },
      },
      AT_THE_ROOT,
    );
    expect(need.title).toBe("A repo copies a built-in skill");
  });
});
