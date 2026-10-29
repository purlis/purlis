// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's page inside the Skill studio** (#3682, spec #3679, the amendment's S3): its header
 * and "…" menu, the banner that says why it isn't plainly on with its one fix, and its tabs
 * (Overview, Text, History, Grades, Runs), each with its count. Drawn at the window seam, the core
 * faked at its command boundary.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { chooseMore, forgetStudio, renderStudioAt } from "./SkillStudioTab.testkit";
import {
  AT_THE_ROOT,
  ChatToUseHere,
  asPersona,
  inWorkspace,
  skillRowId,
  skillRowNamed,
  type StudioPlace,
  type StudioScope,
} from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { UseSkillOpener } from "./UseSkill";
import { bannerOf, skillFileOf, whoGetsIt } from "./SkillPage";
import { diffLineKind, lifecycleSaid } from "./SkillDetails";
import { groupSaid } from "./SkillPanels";
import { PLANE_ROOT } from "./pieceViews";
import type { Offer } from "./actions";
import type { SkillStats, StudioSkill } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const NEW = "a".repeat(40);
const OLD = "b".repeat(40);
const RUN = "01JB0000000000000000000001";

const NO_TOTALS: SkillStats = { by_harness: [], by_persona: [], by_workspace: [] };

function totals(runs: number) {
  return {
    group: { kind: "none" },
    runs,
    ended: runs,
    tool_calls: runs * 2,
    tool_errors: 0,
    refusals: 0,
    asks: 0,
    tokens: null,
    ms: 100,
  };
}

function version(commit: string, subject: string, runs: number) {
  return {
    commit,
    at: "2026-10-20T10:00:00+00:00",
    author: "Ana",
    subject,
    runs: totals(runs),
    grade: null,
  };
}

/** What the core keeps about deploy: one copy at the project's skills, with two versions. */
function details(lifecycle: unknown = { state: "active" }, unused: number | null = null) {
  return {
    copies: [
      {
        level: { at: "project", local: false },
        label: "project",
        lifecycle,
        unread: false,
        owners: ["alice"],
        unused_days: unused,
        versions: [version(NEW, "skill edit deploy", 2), version(OLD, "skill new deploy", 0)],
        uncommitted: totals(0),
      },
    ],
    uses: [
      { said: "skills:ship", name: "ship", project: true, broken: false },
      { said: "skills:gone", name: "gone", project: true, broken: true },
    ],
    used_by: ["release"],
    uses_cut: false,
    pinned_by: [] as unknown[],
    pins_refused: [] as unknown[],
  };
}

/** purlis's Runs panel, as the core answers it: the table of runs. */
function runsPanel(rows: { key: string; cells: string[] }[], offer: string | null = null) {
  return {
    key: { kind: "runs" },
    title: "Runs",
    by: null,
    refused: null,
    blocks: [
      {
        kind: "table",
        caption: "Runs of deploy",
        columns: ["Started", "Started by", "Ended", "Tool calls"],
        rows,
        empty: {
          headline: "No runs of deploy yet",
          body: "A run starts when a chat uses it.",
          offer,
        },
      },
    ],
  };
}

const DEPLOY = skill("deploy", { description: "Deploys the site." });

/** A core answering the studio with `skills`, `more` by command, and recording each ask. */
function core(
  skills: StudioSkill[] = [DEPLOY],
  more: Record<string, (args: Record<string, unknown>) => unknown> = {},
  over: Partial<ReturnType<typeof studio>> = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd in more) return more[cmd](given);
    switch (cmd) {
      case "skill_studio":
        return studio(skills, {
          workspace: (given.workspace as string | null) ?? null,
          persona: (given.persona as string | null) ?? null,
          ...over,
        });
      case "skill_details":
        return details();
      case "skill_stats":
        return NO_TOTALS;
      case "skill_panels":
        return [runsPanel([])];
      case "skill_text":
        return "---\nname: deploy\ndescription: Deploys the site.\n---\n\nShip it.\n";
      case "skill_grade_plan":
        return { running: null };
      case "start_options":
        return { profiles: [] };
      case "skill_updates":
        return { offers: [], unconfirmed: [], unasked: [] };
      case "skill_lifecycle":
      case "skill_turn":
      case "skill_pin":
      case "skill_revert":
      case "skill_level_accept":
      case "skill_owners":
        return { notes: [] };
      default:
        return [];
    }
  });
  return asked;
}

/** The studio at `skill`'s page on `scope`'s chats, on `tab`. */
function at(
  one: StudioSkill,
  scope: StudioScope = AT_THE_ROOT,
  more: Partial<StudioPlace> = {},
): StudioPlace {
  return { page: "library", scope, skill: skillRowId(one), ...more };
}

/** The skill's page, once drawn. */
const page = () => screen.findByTestId("skill-studio-skill");

/** The tab of the page called `name`. */
const tab = (name: string | RegExp) => screen.getByRole("tab", { name });

describe("a skill's page", () => {
  it("opens beside the Library's list, with its name, level, actions and tabs", async () => {
    core();
    renderStudioAt(at(DEPLOY));

    const shown = await page();
    expect(within(shown).getByRole("heading", { name: /deploy/ })).toBeInTheDocument();
    expect(within(shown).getByText("project")).toBeInTheDocument();
    expect(within(shown).getByText("Deploys the site.")).toBeInTheDocument();
    for (const name of ["Use in this chat", "Edit deploy", "Grade deploy", "More for deploy"])
      expect(within(shown).getByRole("button", { name })).toBeInTheDocument();
    expect(
      within(screen.getByRole("tablist", { name: "deploy's sections" }))
        .getAllByRole("tab")
        .map((one) => one.querySelector('[data-part="label"]')?.textContent),
    ).toEqual(["Overview", "Text", "History", "Grades", "Runs"]);
    expect(tab(/Overview/)).toHaveAttribute("aria-selected", "true");
    // The Library's list is still beside it, with the skill chosen.
    expect(screen.getByTestId("skill-deploy")).toHaveAttribute("aria-selected", "true");
  });

  it("opens alone with a way back where the tab is narrow", async () => {
    core();
    renderStudioAt(at(DEPLOY), { narrow: true });

    await page();
    expect(screen.getByRole("button", { name: "Back to Library" })).toBeInTheDocument();
    expect(screen.getByLabelText("Skill studio pages")).toBeInTheDocument();
  });

  it("counts History's versions and Runs' runs on their tabs", async () => {
    core([DEPLOY], {
      skill_stats: () => ({
        by_harness: [{ ...totals(3), group: { kind: "named", name: "Claude Code" } }],
        by_persona: [totals(3)],
        by_workspace: [totals(3)],
      }),
    });
    renderStudioAt(at(DEPLOY));

    await page();
    await waitFor(() => expect(tab(/History/)).toHaveTextContent("2"));
    expect(tab(/History/)).toHaveAccessibleDescription("2 versions");
    await waitFor(() => expect(tab(/Runs/)).toHaveAccessibleDescription("3 runs"));
  });

  it("goes to a tab as a place of its own, which the path names", async () => {
    core();
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY));

    await page();
    await user.click(tab(/Text/));
    expect(await screen.findByTestId("skill-text")).toHaveTextContent("Ship it.");
    const path = within(screen.getByRole("list", { name: "Path" }))
      .getAllByRole("listitem")
      .map((one) => (one.textContent ?? "").replace("›", "").trim());
    expect(path).toEqual(["Library", "deploy", "Text"]);
  });

  it("opens on the tab and the run a run's chip names", async () => {
    core([DEPLOY], {
      skill_panels: () => [
        runsPanel([
          { key: RUN, cells: ["2026-10-20 10:00 UTC", "The chat read it", "Ended", "7"] },
        ]),
      ],
    });
    renderStudioAt({
      page: "library",
      scope: AT_THE_ROOT,
      skill: skillRowNamed("deploy"),
      tab: "runs",
      run: RUN,
    });

    const row = await screen.findByTestId(`table-row-${RUN}`);
    expect(row.getAttribute("aria-current")).toBe("true");
    expect(screen.getByTestId("skill-runs").textContent).not.toMatch(/pass|fail/i);
  });

  it("offers a new chat on Runs where there are none, as the catalogue's own row", async () => {
    core([DEPLOY], { skill_panels: () => [runsPanel([], "chat.new")] });
    const pressed: string[] = [];
    const newTab = { id: "chat.new", title: "New tab", available: true } as unknown as Offer;
    renderStudioAt(at(DEPLOY, AT_THE_ROOT, { tab: "runs" }), {
      offerFor: (id) => (id === "chat.new" ? newTab : undefined),
      onPress: (offer) => pressed.push(offer.id),
    });

    const empty = await screen.findByTestId("table-empty");
    within(empty).getByRole("button", { name: "New tab" }).click();
    expect(pressed).toEqual(["chat.new"]);
  });

  it("adds up its runs on Runs by what they ran on, with their signals", async () => {
    core([DEPLOY], {
      skill_stats: () => ({
        by_harness: [{ ...totals(2), group: { kind: "named", name: "Claude Code" } }],
        by_persona: [{ ...totals(2), group: { kind: "named", name: "ops" } }],
        by_workspace: [totals(2)],
      }),
    });
    renderStudioAt(at(DEPLOY, AT_THE_ROOT, { tab: "runs" }));

    const harness = await screen.findByTestId("skill-totals-by_harness");
    expect(harness).toHaveTextContent("Claude Code");
    expect(harness).toHaveTextContent("2 runs");
    expect(screen.getByTestId("skill-totals-by_workspace")).toHaveTextContent("Project root");
  });

  it("opens a grade's scorecard from Grades, in one click", async () => {
    core([DEPLOY], {
      skill_panels: () => [
        {
          key: { kind: "grades" },
          title: "Grades",
          by: null,
          refused: null,
          blocks: [
            {
              kind: "table",
              caption: "Grades of deploy",
              columns: ["Graded", "Version"],
              rows: [{ key: "01GRADE", cells: ["2026-10-28 10:00 UTC", "0123456789ab"] }],
              empty: { headline: "No grades of deploy yet", body: null, offer: null },
            },
          ],
        },
        runsPanel([]),
      ],
    });
    const onOpenView = vi.fn();
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY), { onOpenView });

    await page();
    await user.click(screen.getByRole("button", { name: "Grade deploy" }));
    expect(tab(/Grades/)).toHaveAttribute("aria-selected", "true");
    fireEvent.click(await screen.findByRole("button", { name: "Open scorecard" }));
    expect(onOpenView).toHaveBeenCalledWith(
      { from: null, view: "skill-grade", key: "deploy@01GRADE" },
      "Grade · deploy",
    );
  });

  it("draws an extension's panel on Overview, under its title and the extension's name", async () => {
    core([DEPLOY], {
      skill_panels: () => [
        runsPanel([]),
        {
          key: { kind: "extension", extension: "house", panel: "checks" },
          title: "Checks",
          by: "House rules",
          refused: null,
          blocks: [{ kind: "note", text: "Two checks passed review.", tone: "plain" }],
        },
      ],
    });
    renderStudioAt(at(DEPLOY));

    const checks = await screen.findByTestId("skill-panel-house/checks");
    expect(within(checks).getByRole("heading")).toHaveTextContent("Checks · House rules");
    expect(checks).toHaveTextContent("Two checks passed review.");
  });
});

describe("a skill's Overview", () => {
  it("says who gets it, where it lives, who looks after it, its grade, runs and uses", async () => {
    core();
    renderStudioAt(at(DEPLOY));

    await page();
    const facts = screen.getByLabelText("About deploy");
    expect(within(facts).getByTestId("skill-in-a-chat")).toHaveTextContent(
      "On: a chat at the project's root gets it.",
    );
    expect(facts).toHaveTextContent("skills/deploy · project");
    expect(await within(facts).findByTestId("skill-owners")).toHaveTextContent("alice");
    expect(facts).toHaveTextContent("Not graded");
    const uses = await screen.findByTestId("skill-uses");
    expect(uses).toHaveTextContent("Uses ship");
    expect(uses.querySelector("[data-broken]")).toHaveTextContent("gone");
    expect(uses).toHaveTextContent("Used by release");
  });

  it("changes who looks after it in a dialog", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY));

    await page();
    await user.click(await screen.findByRole("button", { name: "Change owners…" }));
    const dialog = await screen.findByRole("dialog", { name: "Who looks after deploy" });
    const box = within(dialog).getByLabelText("Owners");
    await user.clear(box);
    await user.type(box, "alice, bob");
    await user.click(within(dialog).getByRole("button", { name: "Save owners" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_owners")?.args).toMatchObject({
        name: "deploy",
        owners: ["alice", "bob"],
      }),
    );
  });

  it("suggests deprecating a skill no chat ran for 60 days", async () => {
    core([DEPLOY], { skill_details: () => details({ state: "active" }, 61) });
    renderStudioAt(at(DEPLOY));

    expect(await screen.findByTestId("skill-unused")).toHaveTextContent("61 days");
  });
});

describe("a skill's banner", () => {
  it("says a skill is off in the workspace, and turns it on", async () => {
    const off = skill("lint", {
      on: false,
      turned_off: true,
      left_out: "turned off in this workspace",
    });
    const asked = core([off]);
    const user = userEvent.setup();
    renderStudioAt(at(off, inWorkspace("ops")));

    expect(await screen.findByTestId("skill-banner-off")).toHaveTextContent(
      "It is off in ops, so a chat in ops doesn't get it.",
    );
    // The banner's fix, and the same switch on Overview.
    expect(screen.getAllByRole("button", { name: "Turn on in ops" })).toHaveLength(2);
    await user.click(screen.getAllByRole("button", { name: "Turn on in ops" })[0]);
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_turn")?.args).toMatchObject({
        workspace: "ops",
        name: "lint",
        on: true,
      }),
    );
  });

  it("says why a skill is left out, with the grant that would reach it", async () => {
    const out = skill("ship", {
      on: false,
      left_out: "its grant does not reach ops",
      not_granted: "Grant it to ops to turn it on here.",
    });
    core([out]);
    renderStudioAt(at(out, inWorkspace("ops")));

    expect(await screen.findByTestId("skill-banner-left-out")).toHaveTextContent(
      "Left out: its grant does not reach ops.",
    );
    expect(screen.getByTestId("not-granted-ship")).toHaveTextContent("Grant it to ops");
    // A skill left out is not offered to a chat.
    expect(screen.getByRole("button", { name: "Use in this chat" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("accepts the folder a chat made, where that is why it is left out", async () => {
    const out = skill("scratch", {
      on: false,
      left_out: "a chat made its folder",
      accept: "skills/scratch",
    });
    const asked = core([out]);
    const user = userEvent.setup();
    renderStudioAt(at(out));

    await screen.findByTestId("skill-banner-left-out");
    await user.click(screen.getAllByRole("button", { name: "Accept its folder" })[0]);
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_level_accept")?.args).toMatchObject({
        folder: "skills/scratch",
      }),
    );
  });

  it("says a change waits for review, and goes to it", async () => {
    core([DEPLOY], {
      skill_drafts: () => [
        {
          id: "d1",
          name: "deploy",
          level: { at: "project", local: false },
          chat: 3,
          new: false,
          stale: false,
          files: [],
        },
      ],
    });
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY));

    expect(await screen.findByTestId("skill-banner-review")).toHaveTextContent(
      "A chat changed deploy.",
    );
    await user.click(screen.getByRole("button", { name: "Review the change" }));
    await waitFor(() =>
      expect(
        within(screen.getByRole("navigation", { name: "Skill studio pages" }))
          .getByRole("button", { name: /Drafts/ })
          .getAttribute("aria-current"),
      ).toBe("page"),
    );
  });

  it("names the waiting change on History too", async () => {
    core([DEPLOY], {
      skill_drafts: () => [
        {
          id: "d1",
          name: "deploy",
          level: { at: "project", local: false },
          chat: 3,
          new: false,
          stale: false,
          files: [],
        },
      ],
    });
    renderStudioAt(at(DEPLOY, AT_THE_ROOT, { tab: "history" }));

    expect(await screen.findByTestId("skill-history-review")).toHaveTextContent(
      "Chat 3 changed deploy, and the change is not accepted yet.",
    );
  });

  it("says an installed skill's source has a newer version, and goes to Sources", async () => {
    const installed = skill("brainstorming", { folder: "" });
    core([installed], {
      skill_updates: () => ({
        offers: [
          {
            name: "brainstorming",
            level: { at: "project", local: false },
            label: "project",
            remote: { url: "https://example.org/s.git", branch: null, label: "Superpowers" },
            pinned: { id: "1".repeat(40), short: "1111111" },
            newer: { id: "3".repeat(40), short: "3f2a1c9" },
            edited: false,
          },
        ],
        unconfirmed: [],
        unasked: [],
      }),
    });
    const user = userEvent.setup();
    renderStudioAt(at(installed));

    expect(await screen.findByTestId("skill-banner-update")).toHaveTextContent("3f2a1c9");
    await user.click(screen.getByRole("button", { name: "Review the update" }));
    await screen.findByRole("heading", { name: "Sources" });
  });

  it("says the router hid a skill, and pins it for the workspace", async () => {
    const hidden = skill("figma", {
      hidden: [
        {
          harness: "codex",
          title: "Codex",
          hidden: {
            router: "built-in",
            why: "over-budget",
            said: "over this chat's skill budget",
          },
        },
      ],
    });
    const asked = core([hidden]);
    const user = userEvent.setup();
    renderStudioAt(at(hidden, inWorkspace("ops")));

    expect(await screen.findByTestId("skill-banner-hidden")).toHaveTextContent(
      "Hidden by the router on Codex: over this chat's skill budget.",
    );
    await user.click(screen.getByRole("button", { name: "Pin it to ops" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_pin")?.args).toMatchObject({
        by: { at: "workspace", name: "ops" },
        name: "figma",
        on: true,
      }),
    );
  });

  it("says an archived skill never loads, and unarchives it", async () => {
    const asked = core([DEPLOY], { skill_details: () => details({ state: "archived" }) });
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY));

    expect(await screen.findByTestId("skill-banner-archived")).toHaveTextContent("never loads");
    await user.click(screen.getByRole("button", { name: "Unarchive" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_lifecycle")?.args).toMatchObject({
        lifecycle: { state: "active" },
      }),
    );
  });

  it("is not drawn for a skill a chat simply gets", async () => {
    core();
    renderStudioAt(at(DEPLOY));

    await page();
    await screen.findByTestId("skill-owners");
    expect(document.querySelector('[data-cause^="skill-banner:"]')).toBeNull();
  });
});

describe('a skill\'s "…" menu', () => {
  it("deprecates a skill for a replacement, in a dialog", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY));

    await page();
    await chooseMore("deploy", /^Deprecate/, user);
    const dialog = await screen.findByRole("dialog", { name: "Deprecate deploy?" });
    await user.type(within(dialog).getByLabelText("Use instead (optional)"), "ship");
    await user.click(within(dialog).getByRole("button", { name: "Deprecate" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_lifecycle")?.args).toMatchObject({
        name: "deploy",
        lifecycle: { state: "deprecated", replacement: "ship" },
      }),
    );
  });

  it("archives a skill once the dialog says what that means", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY));

    await page();
    await chooseMore("deploy", /^Archive/, user);
    const dialog = await screen.findByRole("dialog", { name: "Archive deploy?" });
    expect(dialog).toHaveTextContent("no chat loads it until you unarchive it");
    expect(asked.some((one) => one.cmd === "skill_lifecycle")).toBe(false);
    await user.click(within(dialog).getByRole("button", { name: "Archive" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_lifecycle")?.args).toMatchObject({
        lifecycle: { state: "archived" },
      }),
    );
  });

  it("undoes a deprecation from the menu, whatever the banner says first", async () => {
    const off = skill("deploy", { on: false, turned_off: true, left_out: "turned off in ops" });
    const asked = core([off], {
      skill_details: () => details({ state: "deprecated", replacement: null }),
    });
    const user = userEvent.setup();
    renderStudioAt(at(off, inWorkspace("ops")));

    // Off outranks deprecated on the banner, so the menu holds the way back.
    await screen.findByTestId("skill-banner-off");
    await chooseMore("deploy", "Undo deprecation", user);
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_lifecycle")?.args).toMatchObject({
        lifecycle: { state: "active" },
      }),
    );
  });

  it("opens the skill's file in a tab of its own", async () => {
    core();
    const onOpenView = vi.fn();
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY), { onOpenView });

    await page();
    await chooseMore("deploy", "Open the file", user);
    expect(onOpenView).toHaveBeenCalledWith(
      { from: null, view: "plane-file", key: "skills/deploy/SKILL.md" },
      "SKILL.md · Plane root",
    );
  });
});

describe("a built-in skill's page", () => {
  const SECRETS = skill("secrets", {
    level: "built-in",
    label: "built-in",
    grant: null,
    granted: null,
    folder: "/app/plugin/skills/secrets",
  });

  it("says it can't be edited, moved or turned off, and offers only what it can do", async () => {
    core([SECRETS]);
    const user = userEvent.setup();
    renderStudioAt(at(SECRETS));

    const shown = await page();
    expect(within(shown).getByTestId("skill-built-in")).toHaveTextContent(
      "It ships with purlis, so it can't be edited, moved or turned off.",
    );
    expect(within(shown).queryByRole("button", { name: "Edit secrets" })).toBeNull();
    expect(within(shown).queryByRole("button", { name: "Grade secrets" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "More for secrets" }));
    const menu = await screen.findByRole("menu", { name: "More for secrets" });
    for (const row of within(menu).getAllByRole("menuitem"))
      expect(row).toHaveAttribute("aria-disabled", "true");
    expect(menu).toHaveTextContent("Built-in skills can't be changed.");
  });

  it("keeps no text or history of its own here", async () => {
    core([SECRETS]);
    renderStudioAt(at(SECRETS, AT_THE_ROOT, { tab: "text" }));
    expect(await screen.findByTestId("skill-text-none")).toHaveTextContent(
      "It ships with purlis, inside the app.",
    );
    cleanup();
    forgetStudio();
    core([SECRETS]);
    renderStudioAt(at(SECRETS, AT_THE_ROOT, { tab: "history" }));
    expect(await screen.findByTestId("skill-history-none")).toHaveTextContent(
      "Built-in skills change with purlis itself",
    );
  });
});

describe("a skill's History", () => {
  it("lists each version with its runs, shows its changes and reverts to an older one", async () => {
    const asked = core([DEPLOY], {
      skill_diff: () => ({
        text: "--- a\n+++ b\n-Step one.\n+Step two.\n",
        cut: false,
        unseen: [],
      }),
      skill_revert_widens: () => null,
    });
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY, AT_THE_ROOT, { tab: "history" }));

    const newest = await screen.findByTestId(`skill-version-${NEW}`);
    expect(newest).toHaveTextContent("2 runs, 4 tool calls, 0 errors");
    expect(newest).not.toHaveTextContent("Revert to this");
    await user.click(screen.getAllByRole("button", { name: "Show changes" })[0]);
    const diff = await screen.findByTestId("skill-diff");
    expect(diff.querySelector('[data-line="added"]')).toHaveTextContent("+Step two.");
    await user.click(screen.getByRole("button", { name: "Revert to this" }));
    await user.click(await screen.findByRole("button", { name: "Revert" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_revert")?.args).toMatchObject({
        commit: OLD,
      }),
    );
  });

  it("says in place what could not be read, and reads it again", async () => {
    let tries = 0;
    core([DEPLOY], {
      skill_details: () => {
        tries += 1;
        if (tries === 1) throw "the history could not be read: git is busy";
        return details();
      },
    });
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY, AT_THE_ROOT, { tab: "history" }));

    const failed = await screen.findByTestId("skill-page-not-read");
    expect(failed).toHaveTextContent("git is busy");
    // The rest of the page still stands.
    expect(screen.getByRole("button", { name: "Edit deploy" })).toBeInTheDocument();
    await user.click(within(failed).getByRole("button", { name: "Try again" }));
    expect(await screen.findByTestId(`skill-version-${NEW}`)).toBeInTheDocument();
  });
});

describe("Use in this chat", () => {
  it("opens Use skill… on the chat last in front, with the skill picked", async () => {
    core();
    const open = vi.fn();
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY), {
      wrap: (tab) => (
        <ChatToUseHere.Provider value={{ session: 7, name: "steward 5" }}>
          <UseSkillOpener value={open}>{tab}</UseSkillOpener>
        </ChatToUseHere.Provider>
      ),
    });

    await page();
    await user.click(screen.getByRole("button", { name: "Use in this chat" }));
    expect(open).toHaveBeenCalledWith(7, "deploy");
  });

  it("is offered with why not while no chat is open", async () => {
    core();
    renderStudioAt(at(DEPLOY));

    await page();
    expect(screen.getByRole("button", { name: "Use in this chat" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });
});

describe("Edit", () => {
  it("turns the Text tab into the editor", async () => {
    core();
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY));

    await page();
    await user.click(screen.getByRole("button", { name: "Edit deploy" }));
    await waitFor(() => expect(tab(/Text/)).toHaveAttribute("aria-selected", "true"));
    expect(await screen.findByRole("button", { name: "Save" })).toBeInTheDocument();
  });
});

describe("a persona's studio", () => {
  it("adds a skill to the persona's own list from its Overview", async () => {
    const asked = core([DEPLOY], { persona_skill_change: () => null });
    const user = userEvent.setup();
    renderStudioAt(at(DEPLOY, asPersona("ops")));

    await page();
    await user.click(screen.getByRole("button", { name: "Add deploy to ops" }));
    await waitFor(() => expect(asked.some((one) => one.cmd.startsWith("persona"))).toBe(true));
  });
});

describe("what a skill's page says, in words", () => {
  it("says who gets a skill", () => {
    expect(whoGetsIt(DEPLOY, inWorkspace("ops"))).toBe("On: a chat in ops gets it.");
    expect(whoGetsIt(skill("x", { on: false, left_out: "off" }), AT_THE_ROOT)).toBe(
      "Left out: off",
    );
  });

  it("finds the file a skill's text is in, or says why there is none", () => {
    expect(skillFileOf(DEPLOY)).toEqual({ place: PLANE_ROOT, path: "skills/deploy/SKILL.md" });
    expect(
      skillFileOf(skill("r", { level: "repo", folder: "workspaces/ops/api/.claude/skills/r" })),
    ).toEqual({
      place: { workspace: "ops", repo: "api", piece: null },
      path: ".claude/skills/r/SKILL.md",
    });
    expect(skillFileOf(skill("i", { folder: "" }))).toHaveProperty("reason");
  });

  it("puts archived first, and says nothing of a built-in skill", () => {
    const off = skill("lint", { on: false, turned_off: true });
    expect(bannerOf(off, { clashes: [] }, inWorkspace("ops"), {})?.cause).toBe("off");
    const archived = details({ state: "archived" }).copies[0] as never;
    expect(bannerOf(off, { clashes: [] }, inWorkspace("ops"), { copy: archived })?.cause).toBe(
      "archived",
    );
    expect(
      bannerOf(skill("s", { level: "built-in", on: false }), { clashes: [] }, AT_THE_ROOT, {}),
    ).toBeUndefined();
  });

  it("marks added and removed lines, names a replacement, and says each group", () => {
    expect(diffLineKind("+added")).toBe("added");
    expect(diffLineKind("+++ b")).toBeUndefined();
    expect(lifecycleSaid("old", { state: "deprecated", replacement: "new" })).toContain("new");
    expect(groupSaid({ kind: "none" }, "No persona")).toBe("No persona");
  });
});
