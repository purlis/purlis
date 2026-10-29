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
  stays: "kept in the project's history",
  days_left: 27,
  in_history: true,
  ...over,
});

/** A core with `deleted` listed, answering `text` for a last text, recording every ask. */
function core(deleted: unknown[], text: () => unknown = () => "---\nname: x\n---\n\nbody\n") {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "skill_studio")
      return {
        on: true,
        workspace: null,
        persona: null,
        first: { starts: false, unmoved: null },
        workspaces: [],
        levels: [{ level: { at: "project", local: false }, label: "project" }],
        harnesses: [],
        clashes: [],
        skills: [],
      };
    if (cmd === "skill_studio_on") return true;
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
