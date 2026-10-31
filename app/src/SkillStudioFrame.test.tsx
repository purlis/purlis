// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio's frame and Library** (#3681, spec #3679, the amendment's S2): one tab per
 * project, its pages in a left navigation, its own back and forward and path, the scope picker,
 * and the Library as a list with search and filter chips. Drawn at the window seam, the core faked
 * at its command boundary.
 */
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { STUDIO_HISTORY, forgetStudio, openSkill, renderStudio } from "./SkillStudioTab.testkit";
import {
  AT_THE_ROOT,
  SKILL_STUDIO_TITLE,
  SKILL_STUDIO_VIEW,
  inWorkspace,
  studioTabsGone,
} from "./skillStudio";
import { need, skill, studio } from "./skillsListing.testkit";
import { closeTab, noTabs, openView } from "./tabs";
import { openViewAt } from "./ui/panel/viewHistory";
import { pickIn } from "./ui/select/select.testkit";
import type { Studio } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const LIBRARY = [
  skill("deploy", { description: "Deploys a service to staging or production." }),
  skill("review", { description: "Reviews a pull request against REVIEW.md." }),
  skill("lint", {
    level: "workspace",
    label: "workspace",
    on: false,
    turned_off: true,
    left_out: "turned off in this workspace",
    description: "Runs clippy and explains each warning.",
    folder: "workspaces/ops/skills/lint",
  }),
  skill("secrets", {
    level: "built-in",
    label: "built-in",
    grant: null,
    granted: null,
    description: "Uses a credential without its value entering the chat.",
    folder: "/app/plugin/skills/secrets",
  }),
];

/** A core that answers the studio with `answer` (by the scope asked) and records each ask. */
function core(
  answer: (asked: { workspace: string | null }) => Studio | string = () => studio(LIBRARY),
  more: Record<string, unknown> = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "skill_studio") {
      const said = answer({ workspace: (given.workspace as string | null) ?? null });
      if (typeof said === "string") throw said;
      return said;
    }
    if (cmd in more) return more[cmd];
    if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
    return [];
  });
  return asked;
}

/** The pages the navigation names, in its order. */
function pages(): string[] {
  const nav = screen.getByRole("navigation", { name: "Skill studio pages" });
  return within(nav)
    .getAllByRole("button")
    .map((one) => one.querySelector('[data-part="label"]')?.textContent ?? "");
}

/** The page the navigation marks as where the person is. */
function current(): string | null {
  const nav = screen.getByRole("navigation", { name: "Skill studio pages" });
  return (
    within(nav)
      .getAllByRole("button")
      .find((one) => one.getAttribute("aria-current") === "page")
      ?.querySelector('[data-part="label"]')?.textContent ?? null
  );
}

/** The path the bar draws, crumb by crumb. */
function path(): string[] {
  return within(screen.getByRole("list", { name: "Path" }))
    .getAllByRole("listitem")
    .map((one) => (one.textContent ?? "").replace("›", "").trim());
}

describe("the Skill studio's frame", () => {
  it("names its pages, leaving out Needs you and Drafts while they hold nothing", async () => {
    core();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    expect(pages()).toEqual(["Library", "Sources", "Deleted", "Harness skills", "Routing"]);
    expect(current()).toBe("Library");
  });

  it("shows Needs you and Drafts, with their counts, once each holds something", async () => {
    core(() => studio(LIBRARY, { workspace: "ops", needs: [need("triage")] }), {
      skill_drafts: [
        {
          id: "triage-id",
          name: "triage",
          level: { at: "project", local: false },
          chat: 4,
          new: true,
          stale: false,
          files: [],
        },
      ],
    });
    renderStudio(inWorkspace("ops"));
    await waitFor(() =>
      expect(pages()).toEqual([
        "Library",
        "Needs you",
        "Drafts",
        "Sources",
        "Deleted",
        "Harness skills",
        "Routing",
      ]),
    );
    expect(screen.getByRole("button", { name: /^Needs you/ })).toHaveTextContent("1");
    expect(screen.getByRole("button", { name: /^Drafts/ })).toHaveTextContent("1");
  });

  it("walks its own history: a page, back, forward, and the mouse's back button", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    expect(screen.getByRole("button", { name: "Back" })).toHaveAttribute("aria-disabled", "true");

    await user.click(screen.getByRole("button", { name: "Sources" }));
    await waitFor(() => expect(current()).toBe("Sources"));
    expect(path()).toEqual(["Sources"]);

    await user.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(current()).toBe("Library"));
    await user.click(screen.getByRole("button", { name: "Forward" }));
    await waitFor(() => expect(current()).toBe("Sources"));

    const frame = document.querySelector<HTMLElement>(
      '[data-scope="view-frame"][data-part="root"]',
    );
    if (frame === null) throw new Error("no frame");
    fireEvent.mouseUp(frame, { button: 3 });
    await waitFor(() => expect(current()).toBe("Library"));
  });

  it("says where the person is in its path, and a crumb goes back to the page", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await openSkill("skill-deploy", user);
    expect(path()).toEqual(["Library", "deploy"]);

    await user.click(
      within(screen.getByRole("list", { name: "Path" })).getByRole("button", { name: "Library" }),
    );
    await waitFor(() => expect(path()).toEqual(["Library"]));
    expect(screen.getByTestId("skill-studio-unpicked")).toHaveTextContent("Pick a skill");
    // Choosing it was a step: back opens it again.
    await user.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(path()).toEqual(["Library", "deploy"]));
  });

  it("changes whose chats it covers in its own picker, as a step back undoes", async () => {
    const asked = core(({ workspace }) =>
      studio(workspace === "ops" ? LIBRARY.slice(0, 1) : LIBRARY, { workspace }),
    );
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-review");

    await pickIn(screen.getByLabelText("Whose chats"), "ops", user);
    await waitFor(() => expect(screen.queryByTestId("skill-review")).toBeNull());
    expect(screen.getByTestId("skill-deploy")).toBeInTheDocument();
    expect(screen.getByText(/Every skill a chat in ops can get/)).toBeInTheDocument();
    expect(
      asked.filter((one) => one.cmd === "skill_studio").map((one) => one.args.workspace),
    ).toContain("ops");

    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByTestId("skill-review")).toBeInTheDocument();
  });

  it("lands on the page and scope an entry point opens it at, with the way back", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    // The palette's Open Skill studio on ops's Routing, while the tab is open (#3667).
    act(() => openViewAt(STUDIO_HISTORY, { page: "routing", scope: inWorkspace("ops") }));
    await waitFor(() => expect(current()).toBe("Routing"));
    expect(screen.getByLabelText("Whose chats")).toHaveTextContent("A chat in ops");
    expect(screen.getByText(/a router decides which it sees/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(current()).toBe("Library"));
    expect(screen.getByLabelText("Whose chats")).toHaveTextContent("A chat at the project's root");
  });

  it("forgets a studio tab's history once the tab has closed", () => {
    const open = openView(noTabs(), SKILL_STUDIO_VIEW, SKILL_STUDIO_TITLE, "ops");
    const id = open.order[0];
    const closed = closeTab(open, id, () => "ops");
    expect(studioTabsGone(open, closed)).toEqual([SKILL_STUDIO_VIEW]);
    expect(studioTabsGone(open, open)).toEqual([]);
  });

  it("goes page-and-back where the tab is narrow", async () => {
    core();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "library", { narrow: true });
    await openSkill("skill-deploy", user);
    expect(screen.queryByRole("grid", { name: "Skills" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Back to Library" }));
    expect(await screen.findByRole("grid", { name: "Skills" })).toBeInTheDocument();
  });
});

describe("the Library", () => {
  it("lists each skill as a row: its name, its skill level, one mark and one line", async () => {
    core();
    renderStudio();
    const lint = await screen.findByTestId("skill-lint");
    expect(within(lint).getByText("lint")).toBeInTheDocument();
    expect(within(lint).getByText("workspace")).toBeInTheDocument();
    expect(within(lint).getByText("Off")).toBeInTheDocument();
    expect(within(lint).getByText("Runs clippy and explains each warning.")).toHaveAttribute(
      "data-clamp",
      "1",
    );
  });

  it("finds a skill by its name or what it does, and says when nothing matches", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");

    await user.type(screen.getByRole("searchbox", { name: "Search skills" }), "pull request");
    await waitFor(() => expect(screen.queryByTestId("skill-deploy")).toBeNull());
    expect(screen.getByTestId("skill-review")).toBeInTheDocument();

    await user.type(screen.getByRole("searchbox", { name: "Search skills" }), " nowhere");
    const none = await screen.findByTestId("skill-library-none");
    expect(none).toHaveTextContent("No skill matches “pull request nowhere”");
    await user.click(within(none).getByRole("button", { name: "Show every skill" }));
    expect(await screen.findByTestId("skill-deploy")).toBeInTheDocument();
    expect(screen.getByRole("searchbox", { name: "Search skills" })).toHaveValue("");
  });

  it("narrows by its chips: the project's own, built-in, on, and off or left out", async () => {
    core();
    const user = userEvent.setup();
    renderStudio();
    await screen.findByTestId("skill-deploy");
    const chips = screen.getByRole("group", { name: "Show only" });
    const shown = () =>
      within(screen.getByRole("grid", { name: "Skills" }))
        .getAllByRole("row")
        .map((row) => row.getAttribute("data-testid"));

    await user.click(within(chips).getByRole("checkbox", { name: "Built-in" }));
    await waitFor(() => expect(shown()).toEqual(["skill-secrets"]));
    await user.click(within(chips).getByRole("checkbox", { name: "The project's own" }));
    await waitFor(() => expect(shown()).toHaveLength(4));
    await user.click(within(chips).getByRole("checkbox", { name: "Off or left out" }));
    await waitFor(() => expect(shown()).toEqual(["skill-lint"]));
  });

  it("keeps its layout while the skills are read, with rows held in their place", async () => {
    mockIPC(() => new Promise(() => undefined));
    renderStudio();
    expect(await screen.findByTestId("skill-studio-reading")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("navigation", { name: "Skill studio pages" })).toBeInTheDocument();
  });

  it("says in place what could not be read, and reads again on Try again", async () => {
    let fails = true;
    core(() => (fails ? "the skills folder is not readable" : studio(LIBRARY)));
    const user = userEvent.setup();
    renderStudio();
    const refused = await screen.findByTestId("skill-studio-refused");
    expect(refused).toHaveTextContent("the skills folder is not readable");
    expect(screen.getByRole("navigation", { name: "Skill studio pages" })).toBeInTheDocument();
    fails = false;
    await user.click(within(refused).getByRole("button", { name: "Try again" }));
    expect(await screen.findByTestId("skill-deploy")).toBeInTheDocument();
  });

  it("says what appears on an empty page, and offers the way to it", async () => {
    core();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "deleted");
    const none = await screen.findByTestId("skill-deleted-none");
    await user.click(within(none).getByRole("button", { name: "Go to Library" }));
    await waitFor(() => expect(current()).toBe("Library"));
  });
});
