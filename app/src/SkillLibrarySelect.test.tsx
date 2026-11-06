// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Library's Select mode and the Deleted page's** (#5332), at the window seam: what is
 * chosen and how it is counted, ⌘A on the data and not on the rows drawn, each action's review in
 * the selection page's place, the one change it asks of the core, Select ending after it with an
 * Undo, and Deleted's Bring back and Delete now. The core is faked at its command boundary; what
 * each change does is the core's seam test (`verbs::set`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { StudioSkill } from "./bindings";
import { forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT, inWorkspace, type StudioScope } from "./skillStudio";
import { skill } from "./skillsListing.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const builtIn = (name: string) =>
  skill(name, { level: "built-in", label: "built-in", grant: null, granted: null });

/** A core that lists `skills` and `deleted`, recording every ask and answering each change. */
function core(
  skills: StudioSkill[],
  { deleted = [] as unknown[], workspace = null as string | null } = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    switch (cmd) {
      case "skill_studio":
        return {
          on: true,
          workspace,
          persona: null,
          first: { starts: false, unmoved: null },
          places: [],
          workspaces: ["ops"],
          levels: [
            { level: { at: "project", local: false }, label: "project" },
            { level: { at: "workspace", name: "ops", local: false }, label: "workspace ops" },
          ],
          harnesses: [],
          clashes: [],
          skills,
        };
      case "skills_deleted":
        return deleted;
      case "skill_drafts":
        return [];
      case "skill_deletes_cut_short":
        return { settled: [], unsettled: [] };
      case "skill_set_review": {
        const members = given.members as { level: unknown; name: string; label: string }[];
        return {
          skills: members.map((member) => ({ member, changed: false, cannot: null })),
          names: [],
          open_chats: 0,
        };
      }
      case "skill_set_delete":
        return { notes: [], deleted: ["d1", "d2"] };
      case "skill_set_pin":
      case "skill_set_turn":
        return { notes: [], changed: given.names };
      case "skill_set_move":
        return { notes: [], back: [] };
      case "skill_set_export":
        // The person closed the save dialog without choosing a place.
        return null;
      case "skill_set_delete_now":
        return { off_the_list: 1, removed: 1, failed: [] };
      default:
        return { notes: [] };
    }
  });
  return asked;
}

function asks(asked: ReturnType<typeof core>, cmd: string) {
  return asked.filter((one) => one.cmd === cmd);
}

async function select(user: ReturnType<typeof userEvent.setup>, testid = "skill-library-select") {
  await user.click(await screen.findByTestId(testid));
  return screen.findByTestId("select-none");
}

const page = () => screen.getByTestId("select-page");

function open(scope: StudioScope = AT_THE_ROOT) {
  return renderStudio(scope);
}

describe("the Library's Select mode", () => {
  it("is off until Select is pressed, and then puts a box on each row", async () => {
    core([skill("deploy"), skill("tdd")]);
    open();
    const row = await screen.findByTestId("skill-deploy");
    expect(within(row).queryByRole("checkbox")).toBeNull();
    const user = userEvent.setup();
    await select(user);
    expect(within(screen.getByTestId("skill-deploy")).getByRole("checkbox")).toBeTruthy();
  });

  it("counts what is chosen and says how many of them each action reaches", async () => {
    core([skill("deploy"), skill("tdd", { archived: true }), builtIn("browser")]);
    open();
    const user = userEvent.setup();
    await select(user);
    await user.click(screen.getByTestId("skill-deploy"));
    await user.click(screen.getByTestId("skill-tdd"));
    await waitFor(() => expect(page().textContent).toContain("2 selected"));
    const acts = screen.getByTestId("select-acts");
    expect(within(acts).getByTestId("select-act-archive").textContent).toContain("1 of 2");
    expect(within(acts).getByTestId("select-act-unarchive").textContent).toContain("1 of 2");
    expect(within(acts).getByTestId("select-act-delete").textContent).toContain("2 of 2");
    // A turn needs a workspace's chats, and a pin a workspace or a persona: at the root, neither.
    expect(within(acts).queryByTestId("select-act-turn-off")).toBeNull();
    expect(within(acts).queryByTestId("select-act-pin")).toBeNull();
  });

  it("takes everything the search and filters keep on ⌘A, drawn or not, and a folded built-in is not shown", async () => {
    const many = Array.from({ length: 60 }, (_, at) => skill(`skill-${at}`));
    core([...many, builtIn("browser"), builtIn("secrets")]);
    open();
    const user = userEvent.setup();
    await select(user);
    await user.click(screen.getByTestId("skill-skill-0"));
    await user.keyboard("{Meta>}a{/Meta}");
    await waitFor(() => expect(page().textContent).toContain("62 selected, 2 not shown"));
    const hidden = screen.getByTestId("select-names-not-shown");
    expect(within(hidden).getByTestId("select-name-browser")).toBeTruthy();
  });

  it("keeps what is chosen across a search, counted as not shown", async () => {
    core([skill("deploy"), skill("tdd")]);
    open();
    const user = userEvent.setup();
    await select(user);
    await user.click(screen.getByTestId("skill-deploy"));
    await user.type(screen.getByRole("searchbox", { name: "Search skills" }), "tdd");
    await waitFor(() => expect(page().textContent).toContain("1 selected, 1 not shown"));
    await user.click(screen.getByTestId("select-not-shown"));
    await waitFor(() => expect(screen.getByTestId("skill-deploy")).toBeTruthy());
    expect(screen.queryByTestId("skill-tdd")).toBeNull();
  });

  it("deletes only the project's own, says the built-in stays, and ends Select with an Undo", async () => {
    const asked = core([skill("deploy"), skill("tdd"), builtIn("browser")]);
    open();
    const user = userEvent.setup();
    await select(user);
    await user.keyboard("{Meta>}a{/Meta}");
    await user.click(await within(page()).findByRole("button", { name: "Delete…" }));
    const review = await screen.findByTestId("set-review");
    expect(within(review).getByRole("heading", { name: "Delete 2 skills?" })).toBeTruthy();
    expect(within(review).getByTestId("set-stays-browser").textContent).toContain(
      "Built-in skills can't be changed.",
    );
    expect(within(review).getByTestId("set-review-lead").textContent).toMatch(
      /^2 will be deleted, 1 is built-in and stays\./,
    );
    const go = within(review).getByRole("button", { name: "Delete" });
    await waitFor(() => expect(go).not.toHaveAttribute("aria-disabled", "true"));
    await user.click(go);
    await vi.waitFor(() => expect(asks(asked, "skill_set_delete")).toHaveLength(1));
    const members = asks(asked, "skill_set_delete")[0]?.args.members as { name: string }[];
    expect(members.map((one) => one.name)).toEqual(["deploy", "tdd"]);
    await waitFor(() => expect(screen.queryByTestId("select-page")).toBeNull());
    expect(within(screen.getByTestId("skill-deploy")).queryByRole("checkbox")).toBeNull();
    await user.click(await screen.findByRole("button", { name: "Undo" }));
    await vi.waitFor(() => expect(asks(asked, "skill_set_restore")).toHaveLength(1));
    expect(asks(asked, "skill_set_restore")[0]?.args.ids).toEqual(["d1", "d2"]);
  });

  it("pins for the workspace's chats in one change, leaving one pinned already as it is", async () => {
    const ops = { at: "workspace" as const, name: "ops" };
    const asked = core([skill("deploy"), skill("tdd", { pinned: true, pinned_by: [ops] })], {
      workspace: "ops",
    });
    open(inWorkspace("ops"));
    const user = userEvent.setup();
    await select(user);
    await user.keyboard("{Meta>}a{/Meta}");
    await user.click(await within(page()).findByRole("button", { name: "Pin…" }));
    const review = await screen.findByTestId("set-review");
    expect(within(review).getByTestId("set-stays-tdd").textContent).toContain("Pinned already.");
    const go = within(review).getByRole("button", { name: "Pin" });
    await waitFor(() => expect(go).not.toHaveAttribute("aria-disabled", "true"));
    await user.click(go);
    await vi.waitFor(() => expect(asks(asked, "skill_set_pin")).toHaveLength(1));
    expect(asks(asked, "skill_set_pin")[0]?.args).toMatchObject({
      by: ops,
      names: ["deploy"],
      on: true,
    });
  });

  it("moves to the level picked, which the review asks first", async () => {
    const asked = core([skill("deploy"), skill("tdd")]);
    open();
    const user = userEvent.setup();
    await select(user);
    await user.keyboard("{Meta>}a{/Meta}");
    await user.click(await within(page()).findByRole("button", { name: "Move to another level…" }));
    const review = await screen.findByTestId("set-review");
    const go = within(review).getByRole("button", { name: "Move" });
    expect(go).toHaveAttribute("aria-disabled", "true");
    const ops = within(review).getByRole("radio", { name: /workspace ops/ });
    await user.click(ops);
    // Picked, the level is the review's own: it is read again for where they go.
    const moving = () =>
      within(screen.getByTestId("set-review")).getByRole("button", { name: "Move" });
    await waitFor(() => expect(moving()).not.toHaveAttribute("aria-disabled", "true"));
    // The review takes the level in place: the keyboard stays on the choice it made.
    expect(document.activeElement).toBe(ops);
    await user.click(moving());
    await vi.waitFor(() => expect(asks(asked, "skill_set_move")).toHaveLength(1));
    const moves = asks(asked, "skill_set_move")[0]?.args.moves as { to: unknown }[];
    expect(moves.map((one) => one.to)).toEqual([
      { at: "workspace", name: "ops", local: false },
      { at: "workspace", name: "ops", local: false },
    ]);
  });

  it("keeps Select on when an export's save dialog is closed without a place", async () => {
    const asked = core([skill("deploy"), builtIn("browser")]);
    open();
    const user = userEvent.setup();
    await select(user);
    await user.keyboard("{Meta>}a{/Meta}");
    await user.click(await within(page()).findByRole("button", { name: "Export…" }));
    const review = await screen.findByTestId("set-review");
    expect(within(review).getByTestId("set-stays-browser").textContent).toContain(
      "nothing to export",
    );
    const go = within(review).getByRole("button", { name: "Choose where to save…" });
    await waitFor(() => expect(go).not.toHaveAttribute("aria-disabled", "true"));
    await user.click(go);
    await vi.waitFor(() => expect(asks(asked, "skill_set_export")).toHaveLength(1));
    await waitFor(() => expect(screen.getByTestId("set-review")).toBeTruthy());
    expect(screen.queryByRole("button", { name: "Undo" })).toBeNull();
  });

  it("goes back to the selection from a review, and Esc on the list leaves Select", async () => {
    core([skill("deploy"), skill("tdd")]);
    open();
    const user = userEvent.setup();
    await select(user);
    await user.click(screen.getByTestId("skill-deploy"));
    await user.click(await within(page()).findByRole("button", { name: "Delete…" }));
    await user.click(
      within(await screen.findByTestId("set-review")).getByRole("button", { name: "Cancel" }),
    );
    await waitFor(() => expect(page().textContent).toContain("1 selected"));
    screen.getByTestId("skill-deploy").focus();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByTestId("select-page")).toBeNull());
  });
});

const gone = (name: string, over: Record<string, unknown> = {}) => ({
  id: `${name}-id`,
  name,
  level: { at: "project", local: false },
  label: "project",
  days_left: 27,
  in_history: true,
  reserved: false,
  set: null,
  clash: null,
  ...over,
});

/** What the core says of a deleted skill whose name belongs to purlis now (#5457). */
const held = (name: string, offered: string) => ({
  reserved: true,
  clash: { held_by: `the name ${name} belongs to purlis`, offered, taken: ["tdd"] },
});

describe("the Deleted page's Select mode", () => {
  const set = { id: "s1", from: "acme", at: 1_700_000_000 };
  const deleted = [
    gone("grilling", { set }),
    gone("tdd", { set }),
    gone("scratch", { in_history: false, label: "local" }),
    gone("purlis-notes", held("purlis-notes", "my-notes")),
  ];

  it("ticks every skill of a group with its row, and Delete now asks first and says what is gone for good", async () => {
    const asked = core([], { deleted });
    renderStudio(AT_THE_ROOT, "deleted");
    const user = userEvent.setup();
    await select(user, "skill-deleted-select");
    await user.click(screen.getByTestId("deleted-group-s1"));
    await user.click(screen.getByTestId("deleted-scratch"));
    await waitFor(() => expect(page().textContent).toContain("3 selected"));
    await user.click(within(page()).getByRole("button", { name: "Delete now…" }));
    const review = await screen.findByTestId("set-review");
    expect(review.textContent).toContain("This can't be undone.");
    expect(within(review).getByTestId("set-review-lead").textContent).toBe(
      "2 leave this list, and the project's history still holds them. 1 is removed from this machine for good.",
    );
    await user.click(within(review).getByRole("button", { name: "Delete 3 skills now" }));
    await vi.waitFor(() => expect(asks(asked, "skill_set_delete_now")).toHaveLength(1));
    expect(asks(asked, "skill_set_delete_now")[0]?.args.ids).toEqual([
      "grilling-id",
      "tdd-id",
      "scratch-id",
    ]);
    await waitFor(() => expect(screen.queryByTestId("select-page")).toBeNull());
    expect(screen.queryByRole("button", { name: "Undo" })).toBeNull();
  });

  it("brings one whose name belongs to purlis now back under the name the person confirms, and the rest as named", async () => {
    // #5457: filled in as an install fills a taken name, and nothing comes back until it is confirmed.
    const asked = core([], { deleted });
    renderStudio(AT_THE_ROOT, "deleted");
    const user = userEvent.setup();
    await select(user, "skill-deleted-select");
    await user.click(screen.getByTestId("deleted-scratch"));
    await user.click(screen.getByTestId("deleted-purlis-notes"));
    await user.click(await within(page()).findByRole("button", { name: "Bring back…" }));
    const review = await screen.findByTestId("set-review");
    expect(within(review).getByTestId("clash-purlis-notes").textContent).toContain(
      "The name purlis-notes belongs to purlis.",
    );
    const main = within(review).getByRole("button", { name: "Bring back 2 skills" });
    expect(main).toHaveAttribute("aria-disabled", "true");
    await user.click(within(review).getByRole("radio", { name: "Bring it back under a new name" }));
    const box = within(review).getByRole("textbox", { name: "New name for purlis-notes" });
    expect(box).toHaveValue("my-notes");
    await user.clear(box);
    await user.type(box, "tdd");
    await waitFor(() =>
      expect(review.textContent).toContain("There is already a skill named tdd here."),
    );
    expect(main).toHaveAttribute("aria-disabled", "true");
    await user.clear(box);
    await user.type(box, "notes");
    await waitFor(() => expect(main).not.toHaveAttribute("aria-disabled"));
    await user.click(main);
    await vi.waitFor(() => expect(asks(asked, "skill_set_restore")).toHaveLength(1));
    expect(asks(asked, "skill_set_restore")[0]?.args).toMatchObject({
      ids: ["scratch-id", "purlis-notes-id"],
      under: [{ id: "purlis-notes-id", to: "notes" }],
    });
    expect(await screen.findByRole("button", { name: "Undo" })).toBeTruthy();
  });

  it("leaves one whose name belongs to purlis now in Deleted where the person says so", async () => {
    const asked = core([], { deleted });
    renderStudio(AT_THE_ROOT, "deleted");
    const user = userEvent.setup();
    await select(user, "skill-deleted-select");
    await user.click(screen.getByTestId("deleted-scratch"));
    await user.click(screen.getByTestId("deleted-purlis-notes"));
    await user.click(await within(page()).findByRole("button", { name: "Bring back…" }));
    const review = await screen.findByTestId("set-review");
    await user.click(within(review).getByRole("radio", { name: "Leave it in Deleted" }));
    await user.click(within(review).getByRole("button", { name: "Bring back 1 skill" }));
    await vi.waitFor(() => expect(asks(asked, "skill_set_restore")).toHaveLength(1));
    expect(asks(asked, "skill_set_restore")[0]?.args).toMatchObject({
      ids: ["scratch-id"],
      under: [],
    });
  });
});
