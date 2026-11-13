// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, onTestFinished, vi } from "vitest";
import { act, cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetStudio, openSkill, renderStudio, renderStudioAt } from "./SkillStudioTab.testkit";
import {
  AT_THE_ROOT,
  skillRowNamed,
  skillRowId,
  SKILL_STUDIO_TITLE,
  asPersona,
  inWorkspace,
  skillStudioAs,
  skillStudioAt,
  skillStudioTitle,
  skillStudioView,
  studioOpening,
} from "./skillStudio";
import { catalogue, SKILL_STUDIO_SHOW, type Now } from "./actions";
import { skill, studio } from "./skillsListing.testkit";
import { noTabs } from "./tabs";

afterEach(async () => {
  // Frames a test held and did not let go: run, so the next test's frames are its own.
  await letHeldFramesGo?.();
  letHeldFramesGo = undefined;
  cleanup();
  clearMocks();
  forgetStudio();
});

function answering(studio: unknown) {
  mockIPC((cmd) => {
    if (cmd === "skill_studio") return studio;
    if (cmd === "skills_deleted") return [];
    throw new Error(`unexpected ${cmd}`);
  });
}

/** Holds every animation frame asked for from here; the answer lets them go, and later ones run. */
function holdTheFrames(): () => Promise<void> {
  return stepTheFrames().all;
}

/** `count` skills' names, `crowd-000` on, in the Library's order. */
const crowd = (count: number) =>
  Array.from({ length: count }, (_, n) => `crowd-${String(n).padStart(3, "0")}`);

/** How many skills' rows the Library has drawn: what a screen reader counts in it. */
const skillRows = () =>
  within(screen.getByRole("grid", { name: "Skills" }))
    .getAllByRole("row")
    .filter((one) => one.getAttribute("data-testid")?.startsWith("skill-crowd-")).length;

/**
 * Holds every animation frame asked for from here, for the test to let go one at a time: each
 * `next` runs the frames asked for so far and what they leave for after their paint.
 */
function stepTheFrames(): { next: () => Promise<void>; all: () => Promise<void> } {
  let held: FrameRequestCallback[] = [];
  const spy = vi.spyOn(window, "requestAnimationFrame").mockImplementation((run) => {
    held.push(run);
    return held.length;
  });
  const next = () =>
    act(async () => {
      const now = held;
      held = [];
      for (const run of now) run(performance.now());
      await new Promise((done) => setTimeout(done, 0));
    });
  const all = async () => {
    for (let n = 0; n < 50 && held.length > 0; n++) await next();
    spy.mockRestore();
    for (const run of held.splice(0)) run(performance.now());
  };
  letHeldFramesGo = all;
  return { next, all };
}

/** The window this test draws in is `height` px tall, until it ends. */
function windowOfHeight(height: number): void {
  const tall = vi.spyOn(window, "innerHeight", "get").mockReturnValue(height);
  onTestFinished(() => tall.mockRestore());
}

/** The frames held by the test running, for `afterEach` to let go if it did not. */
let letHeldFramesGo: (() => Promise<void>) | undefined;

describe("the Skill studio", () => {
  it("lists each skill with its level, and why one is left out", async () => {
    answering({
      on: true,
      workspace: null,
      first: { starts: false, unmoved: null },
      places: [],
      workspaces: [],
      levels: [],
      harnesses: [],
      clashes: [],
      skills: [
        {
          name: "deploy",
          level: "project",
          label: "project",
          local: false,
          grant: "root",
          granted: { to: "root" },
          turned_off: false,
          on: true,
          left_out: null,
          description: "Deploys the site.",
          not_granted: null,
          written: "deploy",
          named: false,
          unchecked: null,
          folder: "skills/deploy",
          compat: [],
          lint: [],
          hidden: [],
          pinned: false,
          pinned_by: [],
        },
        {
          name: "secrets",
          level: "built-in",
          label: "built-in",
          local: false,
          grant: null,
          granted: null,
          turned_off: false,
          on: true,
          left_out: null,
          description: "Keeps secrets.",
          not_granted: null,
          written: "secrets",
          named: false,
          unchecked: null,
          folder: "/app/plugin/skills/purlis-secrets",
          compat: [],
          lint: [],
          hidden: [],
          pinned: false,
          pinned_by: [],
        },
        {
          name: "review",
          level: "project",
          label: "project",
          local: false,
          grant: "root",
          granted: { to: "root" },
          turned_off: false,
          on: false,
          left_out: "a built-in skill has this name, and a built-in skill is never replaced",
          description: "",
          not_granted: null,
          written: "review",
          named: false,
          unchecked: null,
          folder: "skills/review",
          compat: [],
          lint: [],
          hidden: [],
          pinned: false,
          pinned_by: [],
        },
      ],
    });

    renderStudio();

    const deploy = await screen.findByTestId("skill-deploy");
    expect(within(deploy).getByText("project")).toBeInTheDocument();
    // One line of what it does under its name, and no mark for a skill a chat simply gets.
    expect(within(deploy).getByText("Deploys the site.")).toBeInTheDocument();
    expect(within(deploy).queryByText("Left out")).toBeNull();
    expect(within(screen.getByTestId("skill-review")).getByText("Left out")).toBeInTheDocument();
    expect(
      within(await openSkill("skill-deploy")).getByTestId("skill-in-a-chat"),
    ).toHaveTextContent("On: a chat at the project's root gets it.");
    expect(within(screen.getByTestId("skill-secrets")).getByText("built-in")).toBeInTheDocument();
    expect(
      within(screen.getByTestId("skill-review")).getByText(
        "Left out: a built-in skill has this name, and a built-in skill is never replaced",
      ),
    ).toBeInTheDocument();
  });

  it("draws the Library's first screen of rows at once and the rest straight after (#4351)", async () => {
    const names = Array.from({ length: 60 }, (_, n) => `crowd-${String(n).padStart(2, "0")}`);
    answering(studio(names.map((name) => skill(name))));
    const letTheFrameGo = holdTheFrames();
    renderStudio();

    await screen.findByTestId("skill-crowd-00");
    expect(screen.queryByTestId("skill-crowd-59")).toBeNull();

    await letTheFrameGo();
    expect(await screen.findByTestId("skill-crowd-59")).toBeTruthy();
  });

  it("builds the rest of the Library's rows a screen a frame, so no frame waits on them all (#4351)", async () => {
    windowOfHeight(900);
    answering(studio(crowd(200).map((name) => skill(name))));
    const frames = stepTheFrames();
    renderStudio();
    await screen.findByTestId("skill-crowd-000");
    expect(skillRows()).toBe(23);

    await frames.next();
    expect(skillRows()).toBe(46);
    await frames.next();
    expect(skillRows()).toBe(69);

    await frames.all();
    expect(skillRows()).toBe(200);
  });

  it("draws no more than 40 rows on a very tall window's first frame, and still fills the Library (#4351)", async () => {
    windowOfHeight(2160);
    answering(studio(crowd(200).map((name) => skill(name))));
    const frames = stepTheFrames();
    renderStudio();
    await screen.findByTestId("skill-crowd-000");
    expect(skillRows()).toBe(40);

    await frames.all();
    expect(skillRows()).toBe(200);
  });

  it("still fills the Library in a window that says it has no height (#4351)", async () => {
    windowOfHeight(0);
    answering(studio(crowd(200).map((name) => skill(name))));
    const frames = stepTheFrames();
    renderStudio();
    await screen.findByTestId("skill-crowd-000");
    expect(skillRows()).toBe(20);

    await frames.all();
    expect(skillRows()).toBe(200);
  });

  it("keeps a press on an early row made while the Library's rows are still coming (#4351)", async () => {
    answering(studio(crowd(200).map((name) => skill(name))));
    const frames = stepTheFrames();
    renderStudio();
    await screen.findByTestId("skill-crowd-000");
    await frames.next();

    const user = userEvent.setup();
    await user.click(screen.getByTestId("skill-crowd-001"));
    await frames.all();

    expect(screen.getByTestId("skill-studio-skill").getAttribute("data-row")).toBe(
      screen.getByTestId("skill-crowd-001").getAttribute("data-row"),
    );
  });

  it("keeps the keyboard on the row it went to while the Library's rows are still coming (#4351)", async () => {
    answering(studio(crowd(200).map((name) => skill(name))));
    const frames = stepTheFrames();
    renderStudio();
    const first = await screen.findByTestId("skill-crowd-000");
    await frames.next();

    const user = userEvent.setup();
    await act(async () => first.focus());
    await user.keyboard("{ArrowDown}");
    await frames.all();

    expect(document.activeElement).toBe(screen.getByTestId("skill-crowd-001"));
    expect(skillRows()).toBe(200);
  });

  it("finds what is typed in the search box while the Library's rows are still coming (#4351)", async () => {
    answering(studio(crowd(200).map((name) => skill(name))));
    const frames = stepTheFrames();
    renderStudio();
    await screen.findByTestId("skill-crowd-000");

    const user = userEvent.setup();
    await user.type(screen.getByRole("searchbox", { name: "Search skills" }), "crowd-19");
    await frames.all();

    expect(skillRows()).toBe(10);
    expect(screen.getByTestId("skill-crowd-199")).toBeTruthy();
  });

  it("keeps a chosen row past the first screen drawn once an early row is pressed (#4351, #5276)", async () => {
    const skills = crowd(200).map((name) => skill(name));
    answering(studio(skills));
    const frames = stepTheFrames();
    renderStudioAt({ page: "library", scope: AT_THE_ROOT, skill: skillRowId(skills[150]) });
    await screen.findByTestId("skill-crowd-150");
    const drawnForIt = skillRows();

    await userEvent.setup().click(screen.getByTestId("skill-crowd-005"));

    expect(skillRows()).toBeGreaterThanOrEqual(drawnForIt);
    expect(screen.getByTestId("skill-crowd-150")).toBeTruthy();
    await frames.all();
    expect(skillRows()).toBe(200);
  });

  // The first frame's budget is what an opening with nothing chosen builds, a screen of rows, and
  // the chosen row: opening at row 199 of 200 with every row before it took 140 ms to the row in
  // jsdom, against 28 ms for a screen (#5276), and L10 is judged on the screen's opening.
  it("opens at a chosen row deep in the Library with one screen of rows and that row on its first frame (#5276)", async () => {
    windowOfHeight(900);
    const skills = crowd(200).map((name) => skill(name));
    answering(studio(skills));
    const frames = stepTheFrames();
    renderStudioAt({ page: "library", scope: AT_THE_ROOT, skill: skillRowId(skills[199]) });

    const chosen = await screen.findByTestId("skill-crowd-199");
    expect(chosen.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByTestId("skill-crowd-022")).toBeTruthy();
    expect(screen.queryByTestId("skill-crowd-100")).toBeNull();
    expect(skillRows()).toBe(24);

    await frames.all();
    const drawn = within(screen.getByRole("grid", { name: "Skills" }))
      .getAllByRole("row")
      .map((one) => one.getAttribute("data-testid"));
    expect(drawn).toEqual(crowd(200).map((name) => `skill-${name}`));
  });

  it("keeps the keyboard on a deep chosen row while the rows before it come (#5276)", async () => {
    windowOfHeight(900);
    const skills = crowd(200).map((name) => skill(name));
    answering(studio(skills));
    const frames = stepTheFrames();
    renderStudioAt({ page: "library", scope: AT_THE_ROOT, skill: skillRowId(skills[150]) });
    const chosen = await screen.findByTestId("skill-crowd-150");
    await act(async () => chosen.focus());

    await frames.all();

    expect(skillRows()).toBe(200);
    expect(screen.getByTestId("skill-crowd-150")).toBe(chosen);
    expect(document.activeElement).toBe(chosen);
  });

  it("draws a chosen row past the Library's first screen on the tab's first frame (#4351)", async () => {
    const skills = Array.from({ length: 60 }, (_, n) =>
      skill(`crowd-${String(n).padStart(2, "0")}`),
    );
    answering(studio(skills));
    renderStudioAt({ page: "library", scope: AT_THE_ROOT, skill: skillRowId(skills[50]) });

    await screen.findByTestId("skill-crowd-00");
    expect(screen.queryByTestId("skill-crowd-50")).toBeTruthy();
  });

  it("draws a chosen row known only by its name on the tab's first frame (#4351)", async () => {
    const skills = Array.from({ length: 60 }, (_, n) =>
      skill(`crowd-${String(n).padStart(2, "0")}`),
    );
    answering(studio(skills));
    renderStudioAt({ page: "library", scope: AT_THE_ROOT, skill: skillRowNamed("crowd-50") });

    await screen.findByTestId("skill-crowd-00");
    expect(screen.queryByTestId("skill-crowd-50")).toBeTruthy();
  });

  it("draws the first screen first each time the Library comes back to a tab (#4351)", async () => {
    const skills = Array.from({ length: 60 }, (_, n) =>
      skill(`crowd-${String(n).padStart(2, "0")}`),
    );
    answering(studio(skills));
    renderStudio();
    await screen.findByTestId("skill-crowd-59");

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Sources" }));
    const library = await screen.findByRole("button", { name: "Library" });
    const letTheFrameGo = holdTheFrames();
    await user.click(library);

    await screen.findByTestId("skill-crowd-00");
    expect(screen.queryByTestId("skill-crowd-59")).toBeNull();

    await letTheFrameGo();
    expect(await screen.findByTestId("skill-crowd-59")).toBeTruthy();
  });

  it("says why under the name of a skill left out, not what it says it does (#4352)", async () => {
    answering(
      studio([
        skill("respelled", {
          on: false,
          description: "Widens.",
          left_out: "purlis hands it to no chat: its frontmatter could not be read",
        }),
      ]),
    );

    renderStudio();

    const row = await screen.findByTestId("skill-respelled");
    expect(
      within(row).getByText(
        "Left out: purlis hands it to no chat: its frontmatter could not be read",
      ),
    ).toBeInTheDocument();
    expect(within(row).queryByText("Widens.")).toBeNull();
  });

  it("says the whole of a reason its line cuts short under the pointer, and none of one that fits (#4711)", async () => {
    const long =
      "Left out: a name starting with purlis- is purlis's own, so this one is never handed on";
    // jsdom draws nothing: the long reason's line is wider than its box, as a browser cuts it.
    vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function (
      this: HTMLElement,
    ) {
      return this.textContent === long ? 400 : 100;
    });
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(() => 100);
    answering(
      studio([
        skill("purlis-own", {
          on: false,
          left_out: "a name starting with purlis- is purlis's own, so this one is never handed on",
        }),
        skill("off-here", { on: false, left_out: "off here" }),
      ]),
    );
    try {
      renderStudio();
      const user = userEvent.setup();

      await user.hover(await screen.findByTestId("skill-off-here"));
      await new Promise((done) => setTimeout(done, 600));
      expect(screen.queryByRole("tooltip")).toBeNull();

      await user.hover(screen.getByTestId("skill-purlis-own"));
      expect(await screen.findByRole("tooltip")).toHaveTextContent(long);
    } finally {
      vi.restoreAllMocks();
    }
  });

  it("keeps the Library's tools to search and chips, with New skill… the one way in (#4041)", async () => {
    answering(studio([skill("deploy")]));

    renderStudio();

    await screen.findByTestId("skill-deploy");
    const tools = screen.getByTestId("skill-studio").querySelector(".skill-library-tools");
    if (!(tools instanceof HTMLElement)) throw new Error("the Library draws no tools");
    expect(within(tools).getByRole("searchbox", { name: "Search skills" })).toBeInTheDocument();
    // Search and chips, and no button: New skill… on the bar is the way a skill is started,
    // its Ask a chat to draft it (#3684) included.
    expect(within(tools).queryAllByRole("button")).toEqual([]);
    expect(screen.getByRole("button", { name: "New skill…" })).toBeInTheDocument();
  });

  it("says why the router hid a skill, and which are pinned (SK-15)", async () => {
    const row = (name: string, more: object) => ({
      name,
      level: "project",
      label: "project",
      local: false,
      grant: "root",
      granted: { to: "root" },
      turned_off: false,
      on: true,
      left_out: null,
      description: `Does ${name}.`,
      not_granted: null,
      written: name,
      named: false,
      unchecked: null,
      folder: `skills/${name}`,
      compat: [],
      lint: [],
      hidden: [],
      pinned: false,
      pinned_by: [],
      ...more,
    });
    answering({
      on: true,
      workspace: null,
      first: { starts: false, unmoved: null },
      places: [],
      workspaces: [],
      levels: [],
      harnesses: [],
      clashes: [],
      skills: [
        row("deploy", {
          hidden: [
            {
              harness: "codex",
              title: "Codex",
              hidden: {
                router: "built-in",
                why: "over-budget",
                said: "hidden by the built-in router: over this chat's skill budget, so it is in the index skill",
              },
            },
          ],
        }),
        row("review", { pinned: true }),
      ],
    });

    renderStudio();

    const user = userEvent.setup();
    // The row's one mark says the router hid it; its page says how, on which program.
    expect(
      within(await screen.findByTestId("skill-deploy")).getByText("Hidden by the router"),
    ).toBeInTheDocument();
    expect(within(screen.getByTestId("skill-review")).getByText("Pinned")).toBeInTheDocument();
    const deploy = await openSkill("skill-deploy", user);
    expect(within(deploy).getByTestId("skill-in-a-chat")).toHaveTextContent(
      "On, hidden by the built-in router: over this chat's skill budget, so it is in the index skill (on Codex)",
    );
    // Its banner says so too, and the chat can still load it.
    expect(within(deploy).getByTestId("skill-banner-hidden")).toHaveTextContent(
      "Hidden by the router on Codex",
    );
    expect(
      within(await openSkill("skill-review", user)).getByTestId("skill-in-a-chat"),
    ).toHaveTextContent(/^On, pinned/);
  });

  it("shows a workspace's chats, its own copy over the project's, and both copies", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "skills_deleted") return [];
      if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
      asked.push(args);
      return {
        on: true,
        workspace: "alpha",
        first: { starts: false, unmoved: null },
        places: [],
        workspaces: ["alpha", "beta"],
        levels: [],
        harnesses: [],
        clashes: [],
        skills: [
          {
            name: "lint",
            level: "workspace",
            label: "workspace",
            local: false,
            grant: null,
            granted: null,
            turned_off: false,
            on: true,
            left_out: null,
            description: "Lints here.",
            not_granted: null,
            written: "lint",
            named: false,
            unchecked: null,
            folder: "workspaces/alpha/skills/lint",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
          },
          {
            name: "lint",
            level: "project",
            label: "project",
            local: false,
            grant: "all",
            granted: { to: "all" },
            turned_off: false,
            on: false,
            left_out: "hidden by the workspace skill of this name",
            description: "",
            not_granted: null,
            written: "lint",
            named: false,
            unchecked: null,
            folder: "skills/lint",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
          },
        ],
      };
    });

    renderStudio(inWorkspace("alpha"));

    const mine = await screen.findByTestId("skill-lint");
    expect(within(mine).getByText("workspace")).toBeInTheDocument();
    expect(
      within(screen.getByTestId("skill-lint-hidden-1")).getByText(
        "Left out: hidden by the workspace skill of this name",
      ),
    ).toBeInTheDocument();
    const hidden = await openSkill("skill-lint-hidden-1");
    expect(within(hidden).getByText("Every workspace")).toBeInTheDocument();
    expect(
      within(hidden).getByText("Left out: hidden by the workspace skill of this name"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Every skill a chat in alpha can get/)).toBeInTheDocument();
    expect(asked[0]).toMatchObject({ plane: "p", workspace: "alpha" });
  });

  it("shares a local skill and reads the project again", async () => {
    let shared = false;
    const calls: string[] = [];
    mockIPC((cmd, args) => {
      calls.push(cmd);
      if (cmd === "skill_share") {
        expect(args).toMatchObject({
          plane: "p",
          whose: { at: "project" },
          name: "scratch",
        });
        shared = true;
        return null;
      }
      if (cmd === "skills_deleted") return [];
      if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
      return {
        on: true,
        workspace: null,
        first: { starts: false, unmoved: null },
        places: [],
        workspaces: [],
        levels: [],
        harnesses: [],
        clashes: [],
        skills: [
          {
            name: "scratch",
            level: "project",
            label: shared ? "project" : "local project",
            local: !shared,
            grant: "root",
            granted: { to: "root" },
            turned_off: false,
            on: true,
            left_out: null,
            description: "Tries it.",
            not_granted: null,
            written: "scratch",
            named: false,
            unchecked: null,
            folder: shared ? "skills/scratch" : "skills.local/scratch",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
          },
        ],
      };
    });

    renderStudio();
    const row = await screen.findByTestId("skill-scratch");
    expect(within(row).getByText(/local project/)).toBeInTheDocument();
    within(await openSkill("skill-scratch"))
      .getByRole("button", { name: "Share" })
      .click();

    await waitFor(() =>
      expect(within(screen.getByTestId("skill-scratch")).getByText("project")).toBeInTheDocument(),
    );
    expect(calls.filter((cmd) => cmd === "skill_studio")).toHaveLength(2);
  });

  /** A studio with no skills, whose core answers the deletes cut short with `ended`, and
   *  records every command asked. */
  function cutShort(ended: { settled: string[]; unsettled: string[] }) {
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      if (cmd === "skill_studio")
        return {
          on: true,
          workspace: null,
          persona: null,
          first: { starts: false, unmoved: null },
          places: [],
          workspaces: [],
          levels: [],
          skills: [],
          harnesses: [],
          clashes: [],
        };
      if (cmd === "skill_deletes_cut_short") return ended;
      return [];
    });
    return calls;
  }

  it("says a skill delete a crash cut short, finished as it opens, and reads its skills again", async () => {
    // SK-35 (#3013): the core finishes or undoes it, and the studio says which in one line.
    const line = "release's delete was cut short; it is finished now, and Restore brings it back";
    const calls = cutShort({ settled: [line], unsettled: [] });
    renderStudio();

    const notice = await screen.findByRole("status", { name: /Skill delete cut short/ });
    expect(notice).toHaveTextContent(line);
    // The notice family's box says its tone (#2912).
    expect(notice.closest('[data-scope="notice"][data-part="root"]')).toHaveAttribute(
      "data-variant",
      "news",
    );
    await waitFor(() =>
      expect(calls.filter((cmd) => cmd === "skill_studio").length).toBeGreaterThanOrEqual(2),
    );
    expect(calls.filter((cmd) => cmd === "skill_deletes_cut_short")).toHaveLength(1);
  });

  it("shows a skill delete it could neither finish nor put back as trouble", async () => {
    const failed =
      "release's delete was cut short and could not be finished or put back: the project's " +
      "last commit still has it, and purlis tries again next time";
    const finished =
      "deploy's delete was cut short; it is finished now, and Restore brings it back";
    cutShort({ settled: [finished], unsettled: [failed] });
    renderStudio();

    const notice = await screen.findByRole("status", {
      name: /A skill delete could not be finished/,
    });
    expect(notice.closest('[data-scope="notice"][data-part="root"]')).toHaveAttribute(
      "data-variant",
      "trouble",
    );
    expect(notice).toHaveTextContent(`${failed} ${finished}`);
  });

  it("says it is not on while its flag is off", async () => {
    answering({
      on: false,
      workspace: null,
      first: { starts: false, unmoved: null },
      places: [],
      workspaces: [],
      skills: [],
      harnesses: [],
      clashes: [],
    });
    renderStudio();
    expect(await screen.findByTestId("skill-studio-off")).toBeInTheDocument();
  });

  it("shows a persona's chats, with the persona's own skill and one it names", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
      asked.push(args);
      return {
        on: true,
        workspace: null,
        persona: "ops",
        first: { starts: false, unmoved: null },
        places: [],
        workspaces: ["alpha"],
        levels: [],
        harnesses: [],
        clashes: [],
        skills: [
          {
            name: "triage",
            level: "persona",
            label: "persona",
            local: false,
            grant: null,
            on: true,
            left_out: null,
            description: "Triages.",
            granted: null,
            turned_off: false,
            not_granted: null,
            folder: "personas/ops/skills/triage",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            written: "triage",
            named: false,
            unchecked: null,
          },
          {
            name: "deploy",
            level: "project",
            label: "project",
            local: false,
            grant: "root",
            on: true,
            left_out: null,
            description: "Deploys.",
            granted: null,
            turned_off: false,
            not_granted: null,
            folder: "skills/deploy",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            written: "deploy",
            named: true,
            unchecked: null,
          },
          {
            name: "superpowers:tdd",
            level: "installed",
            label: "installed",
            local: false,
            grant: null,
            on: true,
            left_out: null,
            description: "",
            granted: null,
            turned_off: false,
            not_granted: null,
            folder: "",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            written: "superpowers:tdd",
            named: true,
            unchecked: "purlis can't check whether this machine has it",
          },
        ],
      };
    });

    renderStudio(asPersona("ops"));

    const user = userEvent.setup();
    expect(
      within(await screen.findByTestId("skill-triage")).getByText("persona"),
    ).toBeInTheDocument();
    const triage = await openSkill("skill-triage", user);
    expect(within(triage).queryByRole("button", { name: /Take .* out of/ })).toBeNull();
    expect(
      within(await openSkill("skill-deploy", user)).getByRole("button", {
        name: "Take deploy out of ops",
      }),
    ).toBeInTheDocument();
    expect(
      within(await openSkill("skill-superpowers:tdd", user)).getByText(/purlis can't check/),
    ).toBeInTheDocument();
    expect(screen.getByRole("form", { name: "Add a skill to ops" })).toBeInTheDocument();
    expect(screen.getByText(/Every skill a chat as ops can get/)).toBeInTheDocument();
    expect(asked[0]).toMatchObject({ plane: "p", workspace: null, persona: "ops" });
  });

  it("opens one tab per project, at the page and scope each way in names (#3681)", () => {
    const free = () => false;
    for (const scope of [AT_THE_ROOT, inWorkspace("ops"), asPersona("ops")])
      expect(studioOpening(skillStudioView(scope), free)).toEqual({
        tab: { from: null, view: "skill-studio", key: "" },
        place: { page: "library", scope },
      });
    expect(studioOpening(skillStudioAt(inWorkspace("ops"), "sources"), free).place).toEqual({
      page: "sources",
      scope: inWorkspace("ops"),
    });
    // A tab put back from an older launch, keyed by its scope, is the one tab on its Library.
    expect(
      studioOpening({ from: null, view: "skill-studio", key: "persona/ops" }, () => true),
    ).toEqual({
      tab: { from: null, view: "skill-studio", key: "" },
      place: { page: "library", scope: asPersona("ops") },
    });
    // ⌘-click: a tab of its own, beside the ones open.
    const open = new Set(["", "#2"]);
    expect(
      studioOpening(
        skillStudioAt(AT_THE_ROOT, "library", { newTab: true }),
        (key) => !open.has(key),
      ).tab.key,
    ).toBe("#3");
    // A studio tab itself opens as itself.
    expect(studioOpening({ from: null, view: "skill-studio", key: "#2" }, free)).toEqual({
      tab: { from: null, view: "skill-studio", key: "#2" },
    });
    expect(skillStudioTitle(asPersona("ops"))).toBe("Skill studio · ops");
  });

  it("offers each persona's studio in the palette only while its flag is on", () => {
    const now = (skillStudio: boolean): Now => ({
      tabs: noTabs(),
      workspaces: [],
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
      personas: ["ops"],
      skillStudio,
    });
    const row = (on: boolean) =>
      catalogue(now(on)).find((offer) => offer.id === skillStudioAs("ops"));
    expect(row(true)?.does).toEqual({
      verb: "openView",
      view: skillStudioView(asPersona("ops")),
      title: SKILL_STUDIO_TITLE,
    });
    expect(row(false)).toBeUndefined();
  });

  it("is offered in the palette only while its flag is on", () => {
    const now = (skillStudio: boolean): Now => ({
      tabs: noTabs(),
      workspaces: [],
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
      skillStudio,
    });
    const ids = (on: boolean) => catalogue(now(on)).map((offer) => offer.id);
    expect(ids(true)).toContain(SKILL_STUDIO_SHOW);
    expect(ids(false)).not.toContain(SKILL_STUDIO_SHOW);
  });
});

const ROW = {
  local: false,
  grant: null,
  granted: null,
  turned_off: false,
  not_granted: null,
  on: true,
  left_out: null,
  owner: null,
  switch: null,
};

/** A studio with one project skill, `review`, and Claude Code's own skills and plugins. */
function withClaudeCode(offLangfuse: boolean) {
  return {
    on: true,
    workspace: null,
    first: { starts: false, unmoved: null },
    places: [],
    workspaces: [],
    levels: [],
    skills: [
      {
        ...ROW,
        name: "review",
        level: "project",
        label: "project",
        grant: "root",
        description: "Reviews.",
        folder: "skills/review",
        compat: [],
        lint: [],
        hidden: [],
        pinned: false,
        pinned_by: [],
      },
      {
        ...ROW,
        name: "secrets",
        level: "built-in",
        label: "built-in",
        description: "Keeps secrets.",
        folder: "/app/plugin/skills/purlis-secrets",
        compat: [],
        lint: [],
        hidden: [],
        pinned: false,
        pinned_by: [],
      },
    ],
    clashes: [
      {
        name: "review",
        level: "project",
        local: false,
        harness: "Claude Code",
        outside: "repo",
        owner: { kind: "repo", name: "api" },
        outcome: "both-load",
        said: "the repo api has a skill of this name too, and a chat there gets both",
      },
    ],
    harnesses: [
      {
        harness: "claude",
        title: "Claude Code",
        repo_left_out: [],
        same_left_out: [],
        own: null,
        skills: [
          {
            ...ROW,
            name: "langfuse",
            level: "installed",
            label: "installed",
            on: !offLangfuse,
            left_out: offLangfuse ? "turned off for this project's chats" : null,
            description: "Reads traces.",
            folder: "/home/.claude/skills/langfuse",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            owner: { kind: "you" },
            switch: { kind: "alone" },
          },
          {
            ...ROW,
            name: "figma:frame",
            level: "installed",
            label: "installed",
            description: "Frames.",
            folder: "/cache/figma/skills/frame",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            owner: { kind: "plugin", name: "figma" },
            switch: {
              kind: "not",
              why: "it comes with the plugin figma, which Claude Code turns on or off itself, so its skills are shown read-only",
            },
          },
          {
            ...ROW,
            name: "review",
            level: "repo",
            label: "repo",
            description: "The repo's review.",
            folder: "workspaces/alpha/api/.claude/skills/review",
            compat: [],
            lint: [],
            hidden: [],
            pinned: false,
            pinned_by: [],
            owner: { kind: "repo", name: "api" },
            switch: { kind: "alone" },
          },
        ],
      },
      {
        harness: "opencode",
        title: "opencode",
        repo_left_out: [],
        same_left_out: [],
        own: null,
        skills: [],
      },
    ],
  };
}

describe("each program's own skills in the Skill studio (SK-6)", () => {
  it("marks a project skill a repo also has, and says a built-in skill is always on", async () => {
    answering(withClaudeCode(false));
    renderStudio();

    const user = userEvent.setup();
    await openSkill("skill-review", user);
    expect(await screen.findByTestId("skill-banner-clash")).toHaveTextContent(
      "the repo api has a skill of this name too, and a chat there gets both",
    );
    expect(
      within(await openSkill("skill-secrets", user)).getByTestId("skill-built-in"),
    ).toHaveTextContent(/can't be edited, moved or turned off/);
  });

  it("marks no clash on a copy left out, which no chat gets (SK-46)", async () => {
    const studio = withClaudeCode(false);
    answering({
      ...studio,
      skills: [
        {
          ...studio.skills[0],
          on: false,
          left_out: "hidden by another project skill of this name",
        },
        ...studio.skills.slice(1),
      ],
    });
    renderStudio();

    await openSkill("skill-review");
    expect(screen.queryByTestId("skill-banner-clash")).toBeNull();
  });

  it("marks a clash in the core's words, as `purlis skill list` says it (SK-46)", async () => {
    const studio = withClaudeCode(false);
    studio.clashes = [
      {
        ...studio.clashes[0],
        harness: "opencode",
        outcome: "repo-wins",
        said: "the repo api has a skill of this name too, and opencode chats there get the repo's copy alone",
      },
    ];
    answering(studio);
    renderStudio();

    await openSkill("skill-review");
    expect(await screen.findByTestId("skill-banner-clash")).toHaveTextContent(
      "the repo api has a skill of this name too, and opencode chats there get the repo's copy alone",
    );
  });

  it("renames the project's copy from the clash mark, through the studio's own Rename", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "skill_rename") {
        asked.push(args);
        return { notes: [] };
      }
      if (cmd === "skill_studio") return withClaudeCode(false);
      if (cmd === "skills_deleted") return [];
      throw new Error(`unexpected ${cmd}`);
    });
    renderStudio();

    await openSkill("skill-review");
    await screen.findByTestId("skill-banner-clash");
    screen.getByRole("button", { name: "Rename the project's copy" }).click();
    const dialog = await screen.findByRole("dialog", { name: "Rename review" });
    expect(within(dialog).getByLabelText("New name")).toBeInTheDocument();
  });
});
