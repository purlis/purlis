// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Delete all, Turn all off and on, and their one review** (#5331) at the window seam: the studio
 * tab drawn as its tab draws it, the core faked at its command boundary. What each command does is
 * the core's seam test (`skills/verbs/set/tests.rs`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetStudio, renderStudioAt } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT, sourceKey, type StudioScope } from "./skillStudio";
import { PROJECT } from "./skillChanges";
import { skill, studio } from "./skillsListing.testkit";
import { NO_SKILL_POLICY } from "./skillPolicy.testkit";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const SOURCE = {
  remote: { url: "https://example.com/skills", branch: null, label: "https://example.com/skills" },
  folder: "skills",
  vault: null,
  key: null,
  kept: "shared",
  label: "acme-skills",
};

const LOCAL = { at: "project", local: true };

const member = (name: string, level: unknown = PROJECT, label = "project") => ({
  level,
  name,
  label,
});

const copy = (name: string, over: Record<string, unknown> = {}) => ({
  member: member(name),
  changed: false,
  on_here: null,
  ...over,
});

const COPIES = [
  copy("grilling"),
  copy("tdd", { changed: true }),
  copy("review"),
  { ...copy("grilling"), member: member("grilling", LOCAL, "local") },
];

type Asked = { cmd: string; args: Record<string, unknown> };

/** A core with `copies` installed from the one source, recording every ask. */
function core(
  copies: unknown[] = COPIES,
  answers: Record<string, (args: Record<string, unknown>) => unknown> = {},
): Asked[] {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    const answer = answers[cmd];
    if (answer !== undefined) return answer(given);
    switch (cmd) {
      case "skill_policy":
        return NO_SKILL_POLICY;
      case "skill_studio":
        return studio([], { workspaces: ["ops"] });
      case "skill_sources":
        return [SOURCE];
      case "skill_updates":
        return { offers: [], unasked: [], unconfirmed: [] };
      case "skill_source_browse":
        return {
          commit: { id: "a".repeat(40), short: "aaaaaaa" },
          skills: [],
          not_skills: [],
          checked: 0,
        };
      case "skill_source_installed":
        return copies;
      case "skill_set_review":
        return {
          skills: (given.members as { name: string }[]).map((one) => ({
            member: one,
            changed: one.name === "tdd",
            cannot: null,
          })),
          names: [],
          open_chats: 0,
        };
      case "skill_set_delete":
        return { notes: [], deleted: ["g-id", "r-id"] };
      case "skill_set_restore":
        return { notes: [] };
      case "skill_set_turn":
        return { notes: [], changed: given.names };
      default:
        return [];
    }
  });
  return asked;
}

const asks = (asked: Asked[], cmd: string) => asked.filter((one) => one.cmd === cmd);

/** The source's page, in `scope`. */
async function sourcePage(scope: StudioScope = AT_THE_ROOT) {
  renderStudioAt({
    page: "sources",
    scope,
    sources: {
      at: "source",
      source: sourceKey(SOURCE.remote),
      label: SOURCE.label,
    },
  });
  return screen.findByTestId("skill-source-page");
}

async function ready(page: HTMLElement, name: string) {
  const button = within(page).getByRole("button", { name });
  await waitFor(() => expect(button).not.toHaveAttribute("aria-disabled", "true"));
  return button;
}

const WORKSPACE: StudioScope = { at: "workspace", name: "ops" };

describe("Delete all… on a source's page", () => {
  it("is greyed out with its reason while nothing is installed from the source", async () => {
    core([]);
    const page = await sourcePage();
    const button = await within(page).findByRole("button", { name: "Delete all…" });
    await waitFor(() => expect(button).toHaveAttribute("aria-disabled", "true"));
  });

  it("lists every level, ticks only the current one's, and leaves a changed skill unticked", async () => {
    core();
    const user = userEvent.setup();
    const page = await sourcePage();
    await user.click(await ready(page, "Delete all…"));
    const list = await screen.findByTestId("set-choose");
    await within(list).findByTestId("set-project-grilling");
    const box = (id: string) => within(within(list).getByTestId(id)).getByRole("checkbox");
    expect(box("set-project-grilling")).toBeChecked();
    expect(box("set-project-review")).toBeChecked();
    expect(box("set-project-tdd")).not.toBeChecked();
    expect(within(list).getByTestId("set-project-tdd")).toHaveTextContent("Changed since install");
    // The other level is listed, under its own name, and not ticked.
    expect(box("set-local-grilling")).not.toBeChecked();
    expect(within(list).getByText("local")).toBeInTheDocument();
    expect(within(list).getByText("project")).toBeInTheDocument();
  });

  it("deletes what the review holds as one change, and Undo brings it back", async () => {
    const asked = core();
    const user = userEvent.setup();
    const page = await sourcePage();
    await user.click(await ready(page, "Delete all…"));
    const list = await screen.findByTestId("set-choose");
    await within(list).findByTestId("set-project-grilling");
    await user.click(within(list).getByRole("button", { name: "Review 2 skills" }));
    const review = await screen.findByTestId("set-review");
    await user.click(await within(review).findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(asks(asked, "skill_set_delete")).toHaveLength(1));
    const [ask] = asks(asked, "skill_set_delete");
    expect((ask?.args.members as { name: string }[]).map((one) => one.name)).toEqual([
      "grilling",
      "review",
    ]);
    expect(ask?.args.from).toBe("acme-skills");
    expect(ask?.args.removeSource).toBeNull();
    const notice = await screen.findByRole("status", { name: /Skills deleted/ });
    await user.click(within(notice).getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(asks(asked, "skill_set_restore")).toHaveLength(1));
    expect(asks(asked, "skill_set_restore")[0]?.args.ids).toEqual(["g-id", "r-id"]);
  });
});

describe("the one review", () => {
  /** The review of the two ticked, with `over` in the core's answer. */
  async function review(over: Record<string, unknown>) {
    const asked = core(COPIES, {
      skill_set_review: (given) => ({
        skills: (given.members as { name: string }[]).map((one) => ({
          member: one,
          changed: false,
          cannot: null,
        })),
        names: [],
        open_chats: 0,
        ...over,
      }),
    });
    const user = userEvent.setup();
    const page = await sourcePage();
    await user.click(await ready(page, "Delete all…"));
    const list = await screen.findByTestId("set-choose");
    await within(list).findByTestId("set-project-grilling");
    await user.click(within(list).getByRole("button", { name: "Review 2 skills" }));
    return { asked, user, page: await screen.findByTestId("set-review") };
  }

  it("warns of a skill left naming one that goes, without blocking, and keeps it when asked", async () => {
    const { asked, user, page } = await review({
      names: [{ namer: "to-spec", named: "grilling", member: member("grilling") }],
    });
    const warning = await within(page).findByTestId("set-names-to-spec-grilling");
    expect(warning).toHaveTextContent("to-spec names grilling, which you're deleting.");
    expect(within(page).getByRole("button", { name: "Delete" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await user.click(within(warning).getByRole("button", { name: "Keep grilling" }));
    await waitFor(() => {
      const last = asks(asked, "skill_set_review").at(-1);
      expect((last?.args.members as { name: string }[]).map((one) => one.name)).toEqual(["review"]);
    });
    expect(asks(asked, "skill_set_delete")).toHaveLength(0);
  });

  it("says how many open chats use them, and nothing where none does", async () => {
    const { page } = await review({ open_chats: 3 });
    expect(await within(page).findByTestId("set-open-chats")).toHaveTextContent(
      "3 open chats use these.",
    );
  });

  it("takes out a skill that cannot apply, says why, and deletes the rest", async () => {
    const asked = core(COPIES, {
      skill_set_review: (given) => ({
        skills: (given.members as { name: string }[]).map((one) => ({
          member: one,
          changed: false,
          cannot: one.name === "review" ? "It holds changes." : null,
        })),
        names: [],
        open_chats: 0,
      }),
    });
    const user = userEvent.setup();
    const source = await sourcePage();
    await user.click(await ready(source, "Delete all…"));
    const list = await screen.findByTestId("set-choose");
    await within(list).findByTestId("set-project-grilling");
    await user.click(within(list).getByRole("button", { name: "Review 2 skills" }));
    const page = await screen.findByTestId("set-review");
    expect(await within(page).findByTestId("set-cannot-review")).toHaveTextContent(
      "It holds changes.",
    );
    await user.click(within(page).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(asks(asked, "skill_set_delete")).toHaveLength(1));
    expect(
      (asks(asked, "skill_set_delete")[0]?.args.members as { name: string }[]).map(
        (one) => one.name,
      ),
    ).toEqual(["grilling"]);
  });
});

describe("a review none of whose skills can change", () => {
  it("says nothing is left, holds its main action, and names no section of nothing", async () => {
    core(COPIES, {
      skill_set_review: (given) => ({
        skills: (given.members as { name: string }[]).map((one) => ({
          member: one,
          changed: false,
          cannot: "It holds changes.",
        })),
        names: [],
        open_chats: 0,
      }),
    });
    const user = userEvent.setup();
    const source = await sourcePage();
    await user.click(await ready(source, "Delete all…"));
    const list = await screen.findByTestId("set-choose");
    await within(list).findByTestId("set-project-grilling");
    await user.click(within(list).getByRole("button", { name: "Review 2 skills" }));
    const page = await screen.findByTestId("set-review");
    await within(page).findByTestId("set-cannot-grilling");
    expect(within(page).getByTestId("set-review-lead")).toHaveTextContent(
      "Nothing is left in this change.",
    );
    expect(within(page).queryByText("In this change")).toBeNull();
    expect(within(page).getByRole("button", { name: "Delete" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });
});

describe("Remove source…", () => {
  it("keeps the skills by default", async () => {
    const asked = core();
    const user = userEvent.setup();
    const page = await sourcePage();
    await user.click(await ready(page, "Remove source…"));
    const dialog = await screen.findByRole("dialog");
    expect(
      await within(dialog).findByRole("checkbox", {
        name: "Also delete the 4 skills installed from it",
      }),
    ).not.toBeChecked();
    // Ticked and closed, it starts from keeping them the next time it opens.
    await user.click(
      within(dialog).getByRole("checkbox", { name: "Also delete the 4 skills installed from it" }),
    );
    expect(dialog).toHaveTextContent("You review the skills next.");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await user.click(await ready(page, "Remove source…"));
    const again = await screen.findByRole("dialog");
    expect(
      await within(again).findByRole("checkbox", {
        name: "Also delete the 4 skills installed from it",
      }),
    ).not.toBeChecked();
    await user.click(within(again).getByRole("button", { name: "Remove source" }));
    await waitFor(() => expect(asks(asked, "skill_source_remove")).toHaveLength(1));
    expect(asks(asked, "skill_set_delete")).toHaveLength(0);
  });

  it("with its one choice opens the same review, and takes the source out with them", async () => {
    const asked = core();
    const user = userEvent.setup();
    const page = await sourcePage();
    await user.click(await ready(page, "Remove source…"));
    const dialog = await screen.findByRole("dialog");
    await user.click(
      await within(dialog).findByRole("checkbox", {
        name: "Also delete the 4 skills installed from it",
      }),
    );
    await user.click(within(dialog).getByRole("button", { name: "Review" }));
    const review = await screen.findByTestId("set-review");
    // The changed one starts left out, and can be ticked again.
    expect(await within(review).findByTestId("set-left-out-tdd")).toBeInTheDocument();
    await user.click(await within(review).findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(asks(asked, "skill_set_delete")).toHaveLength(1));
    expect(asks(asked, "skill_set_delete")[0]?.args.removeSource).toEqual(SOURCE.remote);
    expect(asks(asked, "skill_source_remove")).toHaveLength(0);
  });
});

describe("Turn all off… and on…", () => {
  const ON = [
    copy("grilling", { on_here: true }),
    copy("tdd", { on_here: true }),
    copy("review", { on_here: false }),
  ];

  it("says why it cannot be pressed at the project's root", async () => {
    core(ON.map((one) => ({ ...one, on_here: null })));
    const page = await sourcePage();
    const off = await within(page).findByRole("button", { name: "Turn all off…" });
    await waitFor(() => expect(off).toHaveAttribute("aria-disabled", "true"));
  });

  it("turns those ticked off in one change, and Undo turns on only those it changed", async () => {
    const asked = core(ON, {
      skill_set_turn: (given) => ({ notes: [], changed: given.names }),
    });
    const user = userEvent.setup();
    const page = await sourcePage(WORKSPACE);
    await user.click(await ready(page, "Turn all off…"));
    const list = await screen.findByTestId("set-choose");
    await within(list).findByTestId("set-project-grilling");
    // Only those on are listed to turn off.
    expect(within(list).queryByTestId("set-project-review")).not.toBeInTheDocument();
    await user.click(within(list).getByRole("button", { name: "Review 2 skills" }));
    const review = await screen.findByTestId("set-review");
    await user.click(await within(review).findByRole("button", { name: "Turn off" }));
    await waitFor(() => expect(asks(asked, "skill_set_turn")).toHaveLength(1));
    expect(asks(asked, "skill_set_turn")[0]?.args).toMatchObject({
      workspace: "ops",
      names: ["grilling", "tdd"],
      on: false,
    });
    const notice = await screen.findByRole("status", { name: /Skills turned off/ });
    await user.click(within(notice).getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(asks(asked, "skill_set_turn")).toHaveLength(2));
    expect(asks(asked, "skill_set_turn")[1]?.args).toMatchObject({
      names: ["grilling", "tdd"],
      on: true,
    });
  });
});

describe("the Library's Source filter", () => {
  const SKILLS = [
    skill("grilling", { source_label: "acme-skills" }),
    skill("deploy", { source_label: "platform" }),
    skill("mine"),
  ];

  it("narrows to the source a row's tag names", async () => {
    core(COPIES, { skill_studio: () => studio(SKILLS, { workspaces: [] }) });
    const user = userEvent.setup();
    renderStudioAt({ page: "library", scope: AT_THE_ROOT });
    const row = await screen.findByTestId("skill-grilling");
    expect(screen.getByTestId("skill-mine")).toBeInTheDocument();
    await user.click(within(row).getByRole("button", { name: "acme-skills" }));
    await waitFor(() => expect(screen.queryByTestId("skill-mine")).toBeNull());
    expect(screen.queryByTestId("skill-deploy")).toBeNull();
    expect(screen.getByTestId("skill-grilling")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /\bSource$/ })).toHaveTextContent("acme-skills");
  });

  it("is not there while no skill came from a source", async () => {
    core(COPIES, { skill_studio: () => studio([skill("mine")], { workspaces: [] }) });
    renderStudioAt({ page: "library", scope: AT_THE_ROOT });
    await screen.findByTestId("skill-mine");
    expect(screen.queryByRole("button", { name: /\bSource$/ })).toBeNull();
  });
});
