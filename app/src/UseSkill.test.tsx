// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { renderHook, waitFor } from "@testing-library/react";
import {
  ChatSkillSetsHere,
  SHOW_THIS_CHATS_SKILLS,
  SkillsChip,
  UseSkill,
  UseSkillOpener,
  skillsMatching,
  useChatSkillSets,
} from "./UseSkill";
import type { ChatSkill, ChatSkills, PlaneUpdated, SkillBudget } from "./bindings";
import { CHAT_SKILLS_ASKED, skillsMissingPutAway, type ChatSkillsAsk } from "./chatSkillsAsk";
import { budgetSaid, pinsTogetherSaid, warnsSaid, type ChatSkillsSaid } from "./chatSkillNeeds";

afterEach(() => {
  cleanup();
  clearMocks();
});

function skill(name: string, more: Partial<ChatSkill> = {}): ChatSkill {
  return {
    name,
    level: "project",
    label: "project",
    description: `Does ${name}.`,
    manual: false,
    arguments: [],
    hint: null,
    hidden: null,
    ...more,
    takes_arguments:
      (more.arguments !== undefined && more.arguments.length > 0) ||
      (more.hint !== undefined && more.hint !== null),
  };
}

const BUDGET: SkillBudget = {
  skills: 3,
  chars: 1234,
  manual: 1,
  most_skills: 30,
  most_chars: 6000,
  hidden: 0,
  still_over: null,
  pinned_by: [],
  pins_together: null,
  warns: null,
};

/** #3391: forty of the workspace's pins and thirty of the persona's, past the 64 a chat reads. */
const TOGETHER: SkillBudget = {
  ...BUDGET,
  pinned_by: [
    { at: "workspace", name: "ops" },
    { at: "persona", name: "reviewer" },
  ],
  pins_together: {
    held: [
      { by: { at: "workspace", name: "ops" }, pins: 40 },
      { by: { at: "persona", name: "reviewer" }, pins: 30 },
    ],
    most: 64,
  },
};

function skills(listed: ChatSkill[], budget: SkillBudget | null = BUDGET): ChatSkills {
  return {
    on: true,
    known: true,
    skills: listed,
    budget,
    repos: [],
    not_supported: null,
    routing: null,
    missing: null,
  };
}

function opened(listed: ChatSkills | undefined, onUse = vi.fn()) {
  render(
    <UseSkill chat="steward 3" skills={listed} busy={false} onUse={onUse} onCancel={vi.fn()} />,
  );
  return onUse;
}

describe("Use skill…", () => {
  it("lists exactly the chat's skills, built-in ones last, the manual-only ones marked, and nothing above (S8)", () => {
    opened(
      skills([
        skill("deploy"),
        skill("handoff", { level: "built-in", label: "built-in" }),
        skill("release", { manual: true }),
      ]),
    );
    const rows = screen.getAllByTestId("use-skill-row");
    expect(rows.map((row) => row.querySelector(".use-skill-name")?.textContent)).toEqual([
      "deploy",
      "release",
      "handoff",
    ]);
    expect(within(rows[1]).getByText("· manual only")).toBeTruthy();
    expect(within(rows[0]).queryByText("· manual only")).toBeNull();
    // Each skill is named by its name alone, and its level is its description (#2911).
    expect(screen.getByRole("option", { name: "deploy" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "release" })).toHaveAccessibleDescription(
      /manual only/,
    );
    // Its budget, routing and pins are the Skills view's now: none is said here.
    expect(screen.queryByTestId("use-skill-budget")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("types a skill that takes nothing at once", () => {
    const onUse = opened(skills([skill("deploy")]));
    fireEvent.click(screen.getByTestId("use-skill-row"));
    expect(onUse).toHaveBeenCalledWith("deploy", []);
  });

  it("asks for each declared argument in order before it types the skill", () => {
    const onUse = opened(
      skills([skill("review", { arguments: ["pr", "focus"], hint: "<pr> [focus]" })]),
    );
    fireEvent.click(screen.getByTestId("use-skill-row"));
    expect(screen.getByRole("heading").textContent).toBe("Use review");
    const boxes = screen.getAllByRole("textbox");
    expect(boxes).toHaveLength(2);
    expect(boxes[0].getAttribute("placeholder")).toBe("<pr> [focus]");
    fireEvent.change(boxes[0], { target: { value: "42" } });
    fireEvent.change(boxes[1], { target: { value: "the tests" } });
    fireEvent.click(screen.getByRole("button", { name: "Type it" }));
    expect(onUse).toHaveBeenCalledWith("review", ["42", "the tests"]);
  });

  it("gives a skill that only says what to pass one box with that hint", () => {
    const onUse = opened(skills([skill("triage", { hint: "[issue number]" })]));
    fireEvent.click(screen.getByTestId("use-skill-row"));
    expect(screen.getAllByRole("textbox")).toHaveLength(1);
    expect(screen.getByText("[issue number]")).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "12" } });
    fireEvent.click(screen.getByRole("button", { name: "Type it" }));
    expect(onUse).toHaveBeenCalledWith("triage", ["12"]);
  });

  it("finds a skill by any words of its name or description", () => {
    const listed = [skill("deploy", { description: "Ships the app." }), skill("review")];
    expect(skillsMatching(listed, "SHIPS app").map((one) => one.name)).toEqual(["deploy"]);
    expect(skillsMatching(listed, "").map((one) => one.name)).toEqual(["deploy", "review"]);
    // A list over seven gets the family's filter (#2910), its total in its placeholder.
    const many = [...listed, ...["a", "b", "c", "d", "e", "f"].map((name) => skill(name))];
    opened(skills(many));
    const filter = screen.getByRole("searchbox", { name: "Filter skills" });
    expect(filter.getAttribute("placeholder")).toBe("Filter 8 skills");
    fireEvent.change(filter, { target: { value: "nothing like it" } });
    expect(screen.getByText("No skill is called “nothing like it”.")).toBeTruthy();
  });

  it("is a quick picker: its box takes the keys on any list, and Enter takes the first match", () => {
    const onUse = opened(skills([skill("deploy"), skill("review"), skill("triage")]));
    const filter = screen.getByRole("searchbox", { name: "Filter skills" });
    expect(document.activeElement).toBe(filter);
    fireEvent.change(filter, { target: { value: "rev" } });
    expect(screen.getAllByTestId("use-skill-row")).toHaveLength(1);
    fireEvent.keyDown(filter, { key: "Enter" });
    expect(onUse).toHaveBeenCalledWith("review", []);
  });

  it("asks a skill's arguments in a second step when Enter takes it", () => {
    const onUse = opened(skills([skill("review", { arguments: ["pr"] }), skill("deploy")]));
    fireEvent.keyDown(screen.getByRole("searchbox", { name: "Filter skills" }), { key: "Enter" });
    expect(onUse).not.toHaveBeenCalled();
    expect(screen.getByRole("heading").textContent).toBe("Use review");
    expect(screen.getAllByRole("textbox")).toHaveLength(1);
  });

  it("warns of a set near or past the budget, saying which part", () => {
    expect(warnsSaid(BUDGET)).toBeUndefined();
    expect(warnsSaid({ ...BUDGET, warns: "skills" })).toMatch(
      /This chat's skills are near or past purlis's skill budget/,
    );
    expect(warnsSaid({ ...BUDGET, warns: "chars" })).toMatch(/descriptions are near or past/);
    expect(warnsSaid({ ...BUDGET, warns: "both" })).toMatch(
      /This chat's skills and their descriptions are near or past/,
    );
    // Said in the Skills view's Needs you (S8), not in the picker.
    opened(skills([skill("deploy")], { ...BUDGET, warns: "both" }));
    expect(screen.queryByRole("status")).toBeNull();
    expect(budgetSaid({ ...BUDGET, manual: 0 })).toBe("3 skills of 30 · 1,234 of 6,000 characters");
  });

  it("says a workspace's and a persona's pins pass 64 together, in the Skills view and not the picker (#3391)", () => {
    expect(pinsTogetherSaid(BUDGET)).toBeUndefined();
    expect(pinsTogetherSaid(TOGETHER)).toBe(
      "This chat reads none of its pins: workspace ops pins 40 skills and persona reviewer pins 30 skills, more than the 64 a chat reads together. Unpin some in Skill studio · ops or Skill studio · reviewer.",
    );
    opened(skills([skill("deploy")], TOGETHER));
    expect(screen.queryByTestId("use-skill-pins-together")).toBeNull();
  });

  it("says nothing of the chat's router: that is the Skills view's (S8)", () => {
    opened({
      ...skills([skill("deploy")]),
      routing: { router: "relevance", ms: 100, fell_back: true, said: "It fell back." },
    });
    expect(screen.queryByTestId("use-skill-routing")).toBeNull();
    expect(screen.queryByText("It fell back.")).toBeNull();
  });

  it("says how many skills the router hid behind the index skill, and marks each (SK-15)", () => {
    const over = { ...BUDGET, warns: "skills" as const, hidden: 2 };
    expect(budgetSaid({ ...over, manual: 0 })).toBe(
      "3 skills of 30 · 1,234 of 6,000 characters · 2 skills in the index skill",
    );
    expect(warnsSaid(over)).toMatch(/so 2 skills are hidden behind the index skill/);
    expect(
      warnsSaid({ ...over, still_over: "pins", pinned_by: [{ at: "workspace", name: "ops" }] }),
    ).toBe(
      "This chat's pinned skills are past purlis's skill budget, so all of them are listed. Unpin some in Skill studio · ops, or make some manual-only.",
    );
    // Both pin: the way out names each (SK-15d).
    expect(
      warnsSaid({
        ...over,
        still_over: "pins",
        pinned_by: [
          { at: "workspace", name: "ops" },
          { at: "persona", name: "reviewer" },
        ],
      }),
    ).toMatch(/Unpin some in Skill studio · ops or Skill studio · reviewer, or make some/);
    expect(warnsSaid({ ...over, still_over: "not-hideable" })).toMatch(
      /purlis's own skills and the ones this chat's program and repos bring are past/,
    );
    opened(
      skills(
        [
          skill("deploy"),
          skill("review", {
            hidden: {
              router: "built-in",
              why: "over-budget",
              said: "hidden by the built-in router",
            },
          }),
        ],
        over,
      ),
    );
    const rows = screen.getAllByTestId("use-skill-row");
    expect(within(rows[1]).getByText("· in the index skill")).toBeTruthy();
    expect(within(rows[0]).queryByText("· in the index skill")).toBeNull();
  });

  it("says why a chat has nothing to offer, and that it is still reading", () => {
    opened(undefined);
    expect(screen.getByText("Reading this chat's skills…")).toBeTruthy();
    cleanup();
    opened({
      on: true,
      known: false,
      skills: [],
      budget: null,
      repos: [],
      not_supported: null,
      routing: null,
      missing: null,
    });
    expect(screen.getByTestId("use-skill-none").textContent).toMatch(/runs no tool purlis knows/);
    cleanup();
    // A declared harness skills are not supported on says so, in the core's one line (SK-30).
    const goose =
      "Skills are not supported on Goose: it has no skill format purlis can hand skills by, and its declaration does not say it takes MCP servers, so its chats get no skills.";
    opened({
      on: true,
      known: false,
      skills: [],
      budget: null,
      repos: [],
      not_supported: goose,
      routing: null,
      missing: null,
    });
    expect(screen.getByTestId("use-skill-none").textContent).toBe(goose);
    cleanup();
    opened(skills([]));
    expect(screen.getByTestId("use-skill-none").textContent).toMatch(/skills chip shows why/);
  });
});

describe("the Skills chip on a chat's pane", () => {
  function studioOn() {
    mockIPC((cmd) => {
      if (cmd === "skill_studio_on") return true;
      throw new Error(`unexpected ${cmd}`);
    });
  }

  /** A chat's set as the window holds it: well, unless `more` says otherwise. */
  const held = (more: Partial<Omit<ChatSkillsSaid, "budget">> & { budget?: SkillBudget } = {}) => ({
    budget: BUDGET,
    routing: null,
    missing: null,
    changed: null,
    ...more,
  });

  /** The chip of chat 4, its set `set`. */
  async function chipOf(set: ReturnType<typeof held>) {
    studioOn();
    render(
      <ChatSkillSetsHere.Provider value={{ 4: set }}>
        <SkillsChip plane="p" session={4} />
      </ChatSkillSetsHere.Provider>,
    );
    return screen.findByTestId("pane-skills");
  }

  it("shows the chat's skills at a glance and opens the Skills view on them, not Use skill… (S8)", async () => {
    studioOn();
    const open = vi.fn();
    const ran: ChatSkillsAsk[] = [];
    const hear = (event: Event) => ran.push((event as CustomEvent<ChatSkillsAsk>).detail);
    window.addEventListener(CHAT_SKILLS_ASKED, hear);
    try {
      render(
        <UseSkillOpener value={open}>
          <ChatSkillSetsHere.Provider value={{ 4: held() }}>
            <SkillsChip plane="p" session={4} />
            <SkillsChip plane="p" session={5} />
          </ChatSkillSetsHere.Provider>
        </UseSkillOpener>,
      );
      const chip = await screen.findByTestId("pane-skills");
      expect(screen.getAllByTestId("pane-skills")).toHaveLength(1);
      expect(chip.textContent).toBe("4 skills");
      expect(chip.hasAttribute("data-over")).toBe(false);
      expect(chip.getAttribute("aria-label")).toBe(`4 skills. ${SHOW_THIS_CHATS_SKILLS}.`);
      fireEvent.click(chip);
      expect(ran).toEqual([{ plane: "p", session: 4 }]);
      expect(open).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener(CHAT_SKILLS_ASKED, hear);
    }
  });

  it("is in the warning look, naming the need, for a set near or past the budget", async () => {
    const chip = await chipOf(held({ budget: { ...BUDGET, warns: "chars" } }));
    expect(chip.hasAttribute("data-over")).toBe(true);
    // A manual-only skill is in the set's count, free in the listing's.
    expect(chip.textContent).toBe("4 skills");
    expect(chip.getAttribute("aria-label")).toBe(
      `4 skills. Near the skill budget. ${SHOW_THIS_CHATS_SKILLS}.`,
    );
  });

  it("is in the warning look for pins past 64 together, under the budget or not (#3391)", async () => {
    const chip = await chipOf(held({ budget: TOGETHER }));
    expect(chip.hasAttribute("data-over")).toBe(true);
    expect(chip.getAttribute("aria-label")).toMatch(/^4 skills\. Its pins are not read\./);
  });

  it("is in the warning look for each other need of the chat's own the Skills view holds (#3953)", async () => {
    const changed: PlaneUpdated = {
      session: 4,
      files: [],
      skills: true,
      skills_added: ["triage"],
      skills_gone: [],
      skills_edited: [],
    };
    const missing = {
      persona: "reviewer",
      missing: ["lint"],
      no_built_in: [],
      broken: [],
      by_a_repo: [],
      beside_a_built_in: [],
    };
    const fellBack = { router: "acme", ms: 9, fell_back: true, said: "acme fell back." };
    for (const [set, title] of [
      [held({ routing: fellBack }), "Its router fell back"],
      [held({ missing }), "Started without 1 skill"],
      [held({ changed }), "Changed since it started"],
    ] as const) {
      const chip = await chipOf(set);
      expect(chip.hasAttribute("data-over"), title).toBe(true);
      expect(chip.getAttribute("aria-label")).toBe(
        `4 skills. ${title}. ${SHOW_THIS_CHATS_SKILLS}.`,
      );
      cleanup();
    }
    // A router that did not fall back needs nothing: the view's header says it.
    const quiet = await chipOf(held({ routing: { ...fellBack, fell_back: false } }));
    expect(quiet.hasAttribute("data-over")).toBe(false);
    cleanup();
    // Several, each named, in the view's order.
    const many = await chipOf(
      held({ budget: { ...BUDGET, warns: "both" }, routing: fellBack, changed }),
    );
    expect(many.getAttribute("aria-label")).toBe(
      `4 skills. Near the skill budget · Its router fell back · Changed since it started. ${SHOW_THIS_CHATS_SKILLS}.`,
    );
  });

  it("reads each open chat's set once, with what it started without, and no read of its own for what changed (#3953)", async () => {
    const asked: string[] = [];
    const missing = {
      persona: null,
      missing: ["lint"],
      no_built_in: [],
      broken: [],
      by_a_repo: [],
      beside_a_built_in: [],
    };
    mockIPC((cmd, args) => {
      const session = (args as { session: number }).session;
      asked.push(`${cmd}:${session}`);
      if (cmd !== "chat_skills") throw new Error(`unexpected ${cmd}`);
      return {
        ...skills([], { ...BUDGET, skills: session }),
        missing: session === 5 ? missing : null,
      };
    });
    const changed: PlaneUpdated = {
      session: 6,
      files: [],
      skills: true,
      skills_added: [],
      skills_gone: ["old"],
      skills_edited: [],
    };
    const { result, rerender } = renderHook(
      ({ sessions, now }) => useChatSkillSets("p", sessions, true, now),
      { initialProps: { sessions: [4, 5], now: {} as Record<number, PlaneUpdated> } },
    );
    await waitFor(() => expect(Object.keys(result.current[0])).toEqual(["4", "5"]));
    expect(result.current[0][5]?.missing).toEqual(missing);
    expect(result.current[0][4]?.changed).toBeNull();
    rerender({ sessions: [4, 5, 6], now: { 6: changed } });
    await waitFor(() => expect(result.current[0][6]?.budget.skills).toBe(6));
    expect(result.current[0][6]?.changed).toEqual(changed);
    expect(asked.sort()).toEqual(["chat_skills:4", "chat_skills:5", "chat_skills:6"]);
    // Its notice put away, the chip says it no more; another project's is not this one's.
    act(() => skillsMissingPutAway("q", 5));
    expect(result.current[0][5]?.missing).toEqual(missing);
    act(() => skillsMissingPutAway("p", 5));
    expect(result.current[0][5]?.missing).toBeNull();
    // An answer read before the put-away, landing after, does not bring it back.
    act(() => result.current[1](5, { ...skills([], BUDGET), missing }));
    expect(result.current[0][5]?.missing).toBeNull();
  });

  it("reads every open chat's set again when what it is made of moved, and only then (#4094)", async () => {
    const asked: number[] = [];
    let inTheSet = 3;
    mockIPC((cmd, args) => {
      if (cmd !== "chat_skills") throw new Error(`unexpected ${cmd}`);
      asked.push((args as { session: number }).session);
      return skills([], { ...BUDGET, skills: inTheSet });
    });
    const { result, rerender } = renderHook(
      ({ moved }) => useChatSkillSets("p", [4, 5], true, {}, moved),
      { initialProps: { moved: 0 } },
    );
    await waitFor(() => expect(result.current[0][5]?.budget.skills).toBe(3));
    expect(asked.sort()).toEqual([4, 5]);
    // A skill turned off on disk: both chats' budgets as the Skills view's header now says them.
    inTheSet = 2;
    rerender({ moved: 1 });
    await waitFor(() => expect(result.current[0][4]?.budget.skills).toBe(2));
    await waitFor(() => expect(result.current[0][5]?.budget.skills).toBe(2));
    expect(asked.sort()).toEqual([4, 4, 5, 5]);
    // Drawn again with nothing moved: nothing read again.
    rerender({ moved: 1 });
    await act(async () => {
      await Promise.resolve();
    });
    expect(asked).toHaveLength(4);
  });

  it("reads a restarted chat's set, which takes the old session's place, and lets the old one go (#4094)", async () => {
    mockIPC((cmd, args) => {
      if (cmd !== "chat_skills") throw new Error(`unexpected ${cmd}`);
      const session = (args as { session: number }).session;
      // The new run started with a router that fell back, which the old one's had not.
      return {
        ...skills([], { ...BUDGET, skills: session }),
        routing:
          session === 7 ? { router: "ext", ms: 900, fell_back: true, said: "Fell back." } : null,
      };
    });
    const { result, rerender } = renderHook(
      ({ sessions }) => useChatSkillSets("p", sessions, true),
      { initialProps: { sessions: [4] } },
    );
    await waitFor(() => expect(result.current[0][4]?.budget.skills).toBe(4));
    rerender({ sessions: [7] });
    await waitFor(() => expect(result.current[0][7]?.routing?.fell_back).toBe(true));
    expect(Object.keys(result.current[0])).toEqual(["7"]);
  });

  it("keeps the latest answer where two reads of one chat land out of order (#4094)", async () => {
    const answers: ((said: ChatSkills) => void)[] = [];
    mockIPC((cmd) => {
      if (cmd !== "chat_skills") throw new Error(`unexpected ${cmd}`);
      return new Promise<ChatSkills>((resolve) => answers.push(resolve));
    });
    const { result, rerender } = renderHook(
      ({ moved }) => useChatSkillSets("p", [4], true, {}, moved),
      { initialProps: { moved: 0 } },
    );
    await waitFor(() => expect(answers).toHaveLength(1));
    rerender({ moved: 1 });
    await waitFor(() => expect(answers).toHaveLength(2));
    await act(async () => answers[1]?.(skills([], { ...BUDGET, skills: 9 })));
    await waitFor(() => expect(result.current[0][4]?.budget.skills).toBe(9));
    await act(async () => answers[0]?.(skills([], { ...BUDGET, skills: 1 })));
    expect(result.current[0][4]?.budget.skills).toBe(9);
  });
});
