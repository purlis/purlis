// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Deleted page and a deleted skill's page** (S9, #3691), at the window seam: the skills
 * deleted in the last 30 days with the days left, Bring back asked of the core, the last text read
 * only when it is opened (and said in place where it could not be), and the page's empty state.
 * The core is faked at its command boundary; what it lists and restores is the core's seam test.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT } from "./skillStudio";
import { skill } from "./skillsListing.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
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
  clash: { held_by: `the name ${name} belongs to purlis`, offered, taken: ["vault"] },
});

/** The built-in skills the studio lists, whose names a skill cannot come back under. */
const BUILT_INS = ["browser", "secrets"].map((name) =>
  skill(name, { level: "built-in", label: "built-in", grant: null, granted: null }),
);

/** A core with `deleted` listed, answering `text` for a last text, recording every ask. */
function core(
  deleted: unknown[],
  text: () => unknown = () => "---\nname: x\n---\n\nbody\n",
  skills: unknown[] = [],
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "skill_studio")
      return {
        workspace: null,
        persona: null,
        first: { starts: false, unmoved: null },
        places: [],
        workspaces: [],
        levels: [{ level: { at: "project", local: false }, label: "project" }],
        harnesses: [],
        clashes: [],
        skills,
      };
    if (cmd === "skills_deleted") return deleted;
    if (cmd === "skill_drafts") return [];
    if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
    if (cmd === "skill_deleted_text") return text();
    return { notes: [] };
  });
  return asked;
}

async function openDeleted(name: string, user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByTestId(`deleted-${name}`));
  return screen.findByTestId("skill-studio-deleted");
}

describe("the Deleted page", () => {
  it("lists each skill deleted in the last 30 days with the days left to bring it back", async () => {
    core([gone("browser-old"), gone("scratch-notes", { days_left: 1, label: "local" })]);
    renderStudio(AT_THE_ROOT, "deleted");
    expect(await screen.findByTestId("deleted-browser-old")).toHaveTextContent(
      "27 days left to bring it back",
    );
    expect(screen.getByTestId("deleted-scratch-notes")).toHaveTextContent(
      "1 day left to bring it back",
    );
    expect(screen.getByTestId("skill-deleted-unpicked")).toBeInTheDocument();
  });

  it("says what bringing one back does, and asks the core to bring it back", async () => {
    const asked = core([gone("browser-old")]);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const page = await openDeleted("browser-old", user);
    expect(page).toHaveTextContent(
      "It can be brought back for 27 more days, with its history and grades.",
    );
    expect(page).toHaveTextContent("the project's history still holds it");
    await user.click(within(page).getByRole("button", { name: "Bring back browser-old" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_restore")?.args).toMatchObject({
        id: "browser-old-id",
      }),
    );
  });

  it("brings back one whose name became purlis's under a new name, filled in as an install fills one", async () => {
    // #3729: its name starts with `purlis-` now; #5457: the name offered is the core's.
    const asked = core(
      [gone("purlis-secrets", held("purlis-secrets", "acme-purlis-secrets"))],
      undefined,
      BUILT_INS,
    );
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const page = await openDeleted("purlis-secrets", user);
    expect(within(page).getByTestId("deleted-name-taken")).toHaveTextContent(
      "Its name belongs to purlis now, so it comes back under a new name.",
    );
    const box = within(page).getByRole("textbox", { name: "New name for purlis-secrets" });
    expect(box).toHaveValue("acme-purlis-secrets");
    await user.clear(box);
    await user.type(box, "purlis-browser");
    expect(page.textContent).toContain(
      "A name starting with purlis- belongs to purlis; pick another.",
    );
    const back = within(page).getByRole("button", {
      name: "Bring back purlis-secrets as purlis-browser",
    });
    expect(back).toBeDisabled();
    await user.click(back);
    await user.clear(box);
    await user.type(box, "vault");
    expect(page.textContent).toContain("There is already a skill named vault here.");
    expect(
      within(page).getByRole("button", { name: "Bring back purlis-secrets as vault" }),
    ).toBeDisabled();
    expect(asked.some((one) => one.cmd === "skill_restore")).toBe(false);
    await user.clear(box);
    await user.type(box, "vault-keys");
    await user.click(
      within(page).getByRole("button", { name: "Bring back purlis-secrets as vault-keys" }),
    );
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_restore")?.args).toMatchObject({
        id: "purlis-secrets-id",
        under: "vault-keys",
      }),
    );
  });

  it("says a skill this machine keeps is removed for good after its days", async () => {
    core([gone("scratch", { in_history: false, days_left: 3 })]);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const page = await openDeleted("scratch", user);
    expect(page).toHaveTextContent("removed from this machine for good");
  });

  it("reads its last text only when it is opened", async () => {
    const asked = core([gone("browser-old")], () => "---\nname: browser-old\n---\n\nOld steps.\n");
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const page = await openDeleted("browser-old", user);
    expect(asked.some((one) => one.cmd === "skill_deleted_text")).toBe(false);
    await user.click(within(page).getByRole("button", { name: "Show its last text" }));
    expect(await within(page).findByText(/Old steps\./)).toBeInTheDocument();
    expect(asked.find((one) => one.cmd === "skill_deleted_text")?.args).toMatchObject({
      id: "browser-old-id",
    });
  });

  it("says in place why its last text could not be read, with Try again", async () => {
    let fails = true;
    core([gone("browser-old")], () => {
      if (fails) throw "the commit is gone";
      return "the text";
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const page = await openDeleted("browser-old", user);
    await user.click(within(page).getByRole("button", { name: "Show its last text" }));
    expect(
      await within(page).findByText("purlis could not read its last text: the commit is gone"),
    ).toBeInTheDocument();
    fails = false;
    await user.click(within(page).getByRole("button", { name: "Try again" }));
    expect(await within(page).findByText("the text")).toBeInTheDocument();
  });

  it("says nothing was deleted in the last 30 days when nothing was", async () => {
    core([]);
    renderStudio(AT_THE_ROOT, "deleted");
    expect(await screen.findByTestId("skill-deleted-none")).toHaveTextContent(
      "Nothing deleted in the last 30 days.",
    );
  });
});

describe("a set deleted together (#5331)", () => {
  const SET = { id: "1762300800000", from: "acme-skills", at: 1762300800 };
  const three = [
    gone("grilling", { set: SET }),
    gone("tdd", { set: SET }),
    gone("review", { set: SET }),
  ];

  it("is one group with its count, source and day, and Bring all back asks for all of them", async () => {
    const asked = core(three);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const group = await screen.findByTestId(`deleted-group-${SET.id}`);
    expect(group).toHaveTextContent("3 skills");
    expect(group).toHaveTextContent("from acme-skills, 5 Nov");
    expect(
      within(group).getByRole("button", {
        name: "Bring all back: 3 skills from acme-skills, 5 Nov",
      }),
    ).toBeInTheDocument();
    await user.click(within(group).getByRole("button", { name: /Bring all back/ }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_set_restore")?.args).toMatchObject({
        ids: ["grilling-id", "tdd-id", "review-id"],
        source: null,
      }),
    );
  });

  it("lets one skill of it come back alone", async () => {
    const asked = core(three);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const page = await openDeleted("tdd", user);
    await user.click(within(page).getByRole("button", { name: "Bring back tdd" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_restore")?.args).toMatchObject({
        id: "tdd-id",
      }),
    );
    expect(asked.some((one) => one.cmd === "skill_set_restore")).toBe(false);
  });

  it("asks a new name of one whose name became purlis's in Bring all back's one review, and brings the rest back as named", async () => {
    // #5457: the group's Bring all back opens the review where each such skill gets its name.
    const asked = core([
      gone("grilling", { set: SET }),
      gone("tdd", { set: SET }),
      gone("purlis-review", { set: SET, ...held("purlis-review", "acme-skills-purlis-review") }),
    ]);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const group = await screen.findByTestId(`deleted-group-${SET.id}`);
    await user.click(within(group).getByRole("button", { name: /Bring all back/ }));
    const review = await screen.findByTestId("set-review");
    expect(asked.some((one) => one.cmd === "skill_set_restore")).toBe(false);
    await user.click(within(review).getByRole("radio", { name: "Bring it back under a new name" }));
    expect(within(review).getByRole("textbox", { name: "New name for purlis-review" })).toHaveValue(
      "acme-skills-purlis-review",
    );
    await user.click(within(review).getByRole("button", { name: "Bring back 3 skills" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_set_restore")?.args).toMatchObject({
        ids: ["grilling-id", "tdd-id", "purlis-review-id"],
        under: [{ id: "purlis-review-id", to: "acme-skills-purlis-review" }],
      }),
    );
    expect(await screen.findByRole("status", { name: /Skills brought back/ })).toHaveTextContent(
      "3 skills are back.",
    );
    // Its Undo deletes them again as one group: the renamed one under the name it came back as.
    await user.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_set_delete")?.args).toMatchObject({
        members: [{ name: "grilling" }, { name: "tdd" }, { name: "acme-skills-purlis-review" }],
        from: SET.from,
      }),
    );
  });

  it("brings a group none of whose names became purlis's back at once, with no review", async () => {
    const asked = core(three);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const group = await screen.findByTestId(`deleted-group-${SET.id}`);
    await user.click(within(group).getByRole("button", { name: /Bring all back/ }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_set_restore")?.args).toMatchObject({
        ids: ["grilling-id", "tdd-id", "review-id"],
        under: [],
      }),
    );
    expect(screen.queryByTestId("set-review")).toBeNull();
  });

  it("draws a skill on its own where one is left of the set", async () => {
    core([gone("grilling", { set: SET }), gone("other")]);
    renderStudio(AT_THE_ROOT, "deleted");
    await screen.findByTestId("deleted-grilling");
    expect(screen.queryByTestId(`deleted-group-${SET.id}`)).toBeNull();
  });
});
