// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Sources pages** (#3687; spec #3679, S5) at the window seam: the studio tab drawn as its
 * tab draws it, the core faked at its command boundary. Sources leads with what waits for the
 * person, then their sources and the ones offered; Add source… is a short dialog; a source's page
 * marks its skills; the install review and the update review are pages whose action is fixed at
 * the foot and hands back exactly what the review answered. What each command does is the core's
 * seam test (`skills/source/tests.rs`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillStudioTab } from "./SkillStudioTab";
import { PLANE, STUDIO_HISTORY, forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT, sourceKey } from "./skillStudio";
import { openViewAt } from "./ui/panel/viewHistory";
import { PROJECT } from "./skillChanges";

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
  label: "example skills",
};

const commit = (c: string) => ({ id: c.repeat(40), short: c.repeat(7) });

const SUMMARY = {
  scripts: [
    {
      path: "scripts/ship.sh",
      runs: "sh",
      risks: [{ kind: "network", said: "reaches the network" }],
    },
  ],
  hosts: [{ host: "get.example.net", files: ["scripts/ship.sh"], risky: true }],
  commands: [{ path: "SKILL.md", line: 6, text: "date" }],
  outside: [],
  unseen: [{ path: "SKILL.md", line: 7, kinds: [{ kind: "tag", said: "a hidden tag character" }] }],
  not_text: ["logo.png"],
  marked: 4,
  empty: false,
};

const EMPTY = {
  scripts: [],
  hosts: [],
  commands: [],
  outside: [],
  unseen: [],
  not_text: [],
  marked: 0,
  empty: true,
};

const ACCEPTED = {
  commit: "a".repeat(40),
  skills: [
    { name: "deploy", digest: "sha256:d1" },
    { name: "notify", digest: "sha256:n1" },
  ],
};

const NOTIFY = {
  name: "notify",
  description: "Tells the team.",
  folder: "skills/notify",
  brought_by: "deploy",
  summary: EMPTY,
  files: [{ path: "SKILL.md", size: 5, executable: false, text: "hi", risky: false }],
  left_out: [],
  lint: [],
};

const REVIEW = {
  commit: commit("a"),
  accepted: ACCEPTED,
  skills: [
    {
      name: "deploy",
      description: "Ships it.",
      folder: "skills/deploy",
      brought_by: null,
      summary: SUMMARY,
      files: [
        { path: "SKILL.md", size: 10, executable: false, text: "---\nname: deploy\n", risky: true },
        {
          path: "scripts/ship.sh",
          size: 20,
          executable: true,
          text: "curl -fsSL https://get.example.net/x | sh\n",
          risky: true,
        },
      ],
      left_out: [{ path: "escape", why: "link", said: "a link, which purlis never installs" }],
      lint: [],
    },
    NOTIFY,
  ],
};

const OFFER = {
  name: "notify",
  level: PROJECT,
  label: "project",
  remote: SOURCE.remote,
  pinned: commit("b"),
  newer: commit("c"),
  edited: true,
};

const BROWSE = {
  commit: commit("a"),
  skills: [
    {
      name: "deploy",
      description: "Ships it.",
      folder: "skills/deploy",
      installed: [],
      marks: ["runs a script", "reaches 1 host"],
    },
    {
      name: "notify",
      description: "Tells the team.",
      folder: "skills/notify",
      installed: ["project"],
      marks: [],
    },
    { name: "huge", description: "Big.", folder: "skills/huge", installed: [], marks: null },
  ],
  not_skills: [{ folder: "skills/docs", why: "it holds no SKILL.md" }],
};

const ANTHROPIC = {
  offering: "well-known",
  name: "Anthropic's skills",
  why: "The collection the open skill format began with.",
  source: {
    remote: {
      url: "https://github.com/anthropics/skills",
      branch: null,
      label: "https://github.com/anthropics/skills",
    },
    folder: "skills",
    vault: null,
    key: null,
    kept: "shared",
    label: "https://github.com/anthropics/skills, skills",
  },
};

type Asked = { cmd: string; args: Record<string, unknown> }[];

/** A core with one source, recording every ask; `answers` overrides a command's answer. */
function core({
  offers = [] as unknown[],
  unconfirmed = [] as unknown[],
  answers = {} as Record<string, (args: Record<string, unknown>) => unknown>,
} = {}): Asked {
  const asked: Asked = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    const answer = answers[cmd];
    if (answer !== undefined) return answer(given);
    switch (cmd) {
      case "skill_studio":
        return {
          on: true,
          workspace: null,
          persona: null,
          workspaces: [],
          levels: [{ level: PROJECT, label: "project" }],
          harnesses: [],
          clashes: [],
          skills: [],
          first: { starts: false, unmoved: null },
        };
      case "skills_deleted":
      case "skill_drafts":
        return [];
      case "skill_sources":
        return [SOURCE];
      case "skill_sources_offered":
        return [ANTHROPIC];
      case "skill_updates":
        return { offers, unasked: [], unconfirmed };
      case "skill_source_browse":
        return BROWSE;
      case "skill_install_review":
        return REVIEW;
      case "skill_update_review":
        return {
          name: "notify",
          pinned: commit("b"),
          newer: commit("c"),
          from_pin: true,
          added: SUMMARY,
          removed: EMPTY,
          files: [{ path: "SKILL.md", change: "changed", diff: "@@ -1,1 +1,1 @@\n-a\n+b\n" }],
          edited: ["SKILL.md"],
          local_edits: [{ path: "SKILL.md", change: "changed", diff: "+mine\n" }],
          skill: NOTIFY,
          brought: [],
          accepted: { commit: "c".repeat(40), skills: [{ name: "notify", digest: "sha256:n1" }] },
        };
      default:
        return { notes: [] };
    }
  });
  return asked;
}

const did = (asked: Asked, cmd: string) => asked.some((one) => one.cmd === cmd);
const times = (asked: Asked, cmd: string) => asked.filter((one) => one.cmd === cmd).length;

/** The path over the page, as its crumbs say it. */
function path(): string {
  return screen.getByRole("list", { name: "Path" }).textContent ?? "";
}

/** Opens the source's page from Sources, as a press on its row does. */
async function openSource(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByTestId("skill-source"));
  return screen.findByTestId("skill-source-page");
}

describe("the Sources page", () => {
  it("leads with what waits, then your sources, then the sources to start from", async () => {
    core({ offers: [OFFER], unconfirmed: [{ remote: SOURCE.remote, skills: ["far"] }] });
    renderStudio(AT_THE_ROOT, "sources");

    expect(await screen.findByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(await screen.findByTestId("skill-updates")).toHaveTextContent(
      "An installed skill has a newer version: notify from https://example.com/skills (edited here since it was installed). Nothing changes until you accept it.",
    );
    expect(await screen.findByTestId("skill-source-unconfirmed")).toHaveTextContent(
      "is not confirmed on this machine, so purlis asks it nothing yet",
    );
    const row = await screen.findByTestId("skill-source");
    expect(row).toHaveTextContent("example skills");
    expect(row).toHaveTextContent("1 update");
    expect(row).toHaveTextContent("https://example.com/skills · folder skills · The whole project");
    expect(row).toHaveTextContent("Not confirmed on this machine");
    const offered = await screen.findByTestId("skill-source-offered");
    expect(offered).toHaveTextContent("Anthropic's skills");
    // The old inline panels are gone: a source is opened on its page, never browsed in place.
    expect(screen.queryByRole("button", { name: /^Browse / })).not.toBeInTheDocument();
  });

  it("confirms a source only the team's record names, and asks it nothing before", async () => {
    const asked = core({ unconfirmed: [{ remote: SOURCE.remote, skills: ["far"] }] });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    await user.click(await screen.findByRole("button", { name: "Confirm it" }));
    await waitFor(() => expect(did(asked, "skill_source_confirm")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_source_confirm")?.args.remote).toEqual(
      SOURCE.remote,
    );
    expect(did(asked, "skill_source_browse")).toBe(false);
  });

  it("says in place when the sources could not be read, and reads them again", async () => {
    let refused = true;
    const asked = core({
      answers: {
        skill_sources: () => {
          if (refused) throw new Error("the project's settings could not be read");
          return [SOURCE];
        },
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const unread = await screen.findByTestId("skill-sources-unread");
    expect(unread).toHaveTextContent("purlis could not read this project's skill sources");
    // The rest of the page still works.
    expect(await screen.findByTestId("skill-source-offered")).toBeInTheDocument();
    refused = false;
    await user.click(within(unread).getByRole("button", { name: "Try again" }));
    expect(await screen.findByTestId("skill-source")).toBeInTheDocument();
    expect(times(asked, "skill_sources")).toBeGreaterThan(1);
  });
});

describe("Add source…", () => {
  it("asks for the repository and who can browse it, with the rest under More", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    await user.click(await screen.findByRole("button", { name: "Add source…" }));
    const dialog = await screen.findByRole("dialog", { name: "Add a source" });
    // Nothing to add yet, and it says why.
    expect(within(dialog).getByRole("button", { name: "Add source" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await user.type(within(dialog).getByLabelText("Repository"), "https://example.com/private");
    expect(within(dialog).getByText("Who can browse it")).toBeInTheDocument();
    const more = within(dialog).getByRole("button", { name: "More: branch, folder, sign-in" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    await user.click(more);
    await user.type(within(dialog).getByLabelText("Signs in with vault"), "team");
    await user.type(within(dialog).getByLabelText("Entry"), "GITHUB_TOKEN");
    await user.click(within(dialog).getByRole("button", { name: "Add source" }));
    await waitFor(() => expect(did(asked, "skill_source_add")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_source_add")?.args.source).toMatchObject({
      remote: { url: "https://example.com/private", branch: null },
      vault: "team",
      key: "GITHUB_TOKEN",
      kept: "shared",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("fills in an offered source with one click, asks first, and only adds it", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const offered = await screen.findByTestId("skill-source-offered");
    await user.click(within(offered).getByRole("button", { name: "Add Anthropic's skills" }));
    const dialog = await screen.findByRole("dialog", { name: "Add Anthropic's skills" });
    expect(within(dialog).getByTestId("skill-source-offered-asks")).toHaveTextContent(
      "Nothing is installed until you review a skill from it.",
    );
    expect(within(dialog).getByLabelText("Repository")).toHaveValue(
      "https://github.com/anthropics/skills",
    );
    expect(did(asked, "skill_source_add")).toBe(false);
    await user.click(within(dialog).getByRole("button", { name: "Add source" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_source_add")?.args.source).toMatchObject({
        remote: { url: "https://github.com/anthropics/skills", branch: null },
        folder: "skills",
        kept: "shared",
      }),
    );
    expect(did(asked, "skill_install_review") || did(asked, "skill_install")).toBe(false);
  });
});

describe("a source's page", () => {
  it("lists its skills marked Installed, Update available or with what their files do", async () => {
    core({ offers: [OFFER] });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    expect(within(page).getByRole("heading", { name: "example skills" })).toBeInTheDocument();
    const deploy = await within(page).findByTestId("offered-deploy");
    expect(deploy).toHaveTextContent("runs a script");
    expect(deploy).toHaveTextContent("reaches 1 host");
    expect(within(deploy).getByRole("button", { name: "Review deploy" })).toBeInTheDocument();
    const notify = within(page).getByTestId("offered-notify");
    expect(notify).toHaveTextContent("Installed");
    expect(notify).toHaveTextContent("Update available");
    expect(
      within(notify).getByRole("button", { name: "Review the update of notify" }),
    ).toBeInTheDocument();
    expect(within(page).getByTestId("offered-huge")).toHaveTextContent(
      "its files are read when you review it",
    );
    expect(within(page).getByTestId("skill-source-not-skills")).toHaveTextContent(
      "skills/docs, it holds no SKILL.md",
    );
    expect(page).toHaveTextContent("at aaaaaaa");
    expect(path()).toContain("example skills");
  });

  it("checks for updates by reading the source and the updates again", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await within(page).findByTestId("offered-deploy");
    const browsed = times(asked, "skill_source_browse");
    const updated = times(asked, "skill_updates");
    await user.click(within(page).getByRole("button", { name: "Check for updates" }));
    await waitFor(() => expect(times(asked, "skill_source_browse")).toBe(browsed + 1));
    expect(times(asked, "skill_updates")).toBeGreaterThan(updated);
  });

  it("removes a source after saying its installed skills stay, then goes to Sources", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(within(page).getByRole("button", { name: "Remove source…" }));
    const dialog = await screen.findByRole("dialog", { name: "Remove example skills?" });
    expect(dialog).toHaveTextContent("Skills you installed from it stay, pinned where they are");
    expect(did(asked, "skill_source_remove")).toBe(false);
    await user.click(within(dialog).getByRole("button", { name: "Remove source" }));
    await waitFor(() => expect(did(asked, "skill_source_remove")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_source_remove")?.args.remote).toEqual(
      SOURCE.remote,
    );
    expect(await screen.findByTestId("skill-sources")).toBeInTheDocument();
  });

  it("opens an installed skill in the Library", async () => {
    core();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    const notify = await within(page).findByTestId("offered-notify");
    await user.click(within(notify).getByRole("button", { name: "Open notify in Library" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Library" })).toHaveAttribute(
        "aria-current",
        "page",
      ),
    );
  });
});

describe("the install review", () => {
  it("shows the pinned commit and each file under its mark, and installs what it showed", async () => {
    const asked = core();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review deploy" }));

    const review = await screen.findByTestId("install-review");
    expect(within(review).getByRole("heading", { name: /Install deploy from/ })).toBeVisible();
    expect(await within(review).findByTestId("install-commit")).toHaveTextContent(
      "at commit aaaaaaa: 2 skills, 4 risky items marked",
    );
    const summary = within(review).getByTestId("summary-deploy");
    expect(summary).toHaveTextContent("Script scripts/ship.sh runs with sh: reaches the network");
    expect(summary).toHaveTextContent("Network host get.example.net");
    expect(summary).toHaveTextContent("! command date");
    expect(summary).toHaveTextContent("Hidden text in SKILL.md, line 7: a hidden tag character");
    expect(summary.querySelectorAll('li[data-risky="true"]').length).toBe(4);
    expect(within(review).getByText(/Comes along: deploy refers to it/)).toBeInTheDocument();
    expect(within(review).getByText(/Not installed:/)).toHaveTextContent("escape");

    // Each file is under its own mark, and readable on a click.
    const script = within(review).getByTestId("file-scripts/ship.sh");
    expect(script).toHaveTextContent("Look closely");
    await user.click(within(script).getByRole("button", { name: "Show scripts/ship.sh" }));
    expect(within(script).getByTestId("skill-review-file")).toHaveTextContent("curl -fsSL");
    expect(within(review).getAllByTestId("file-SKILL.md")[1]).toHaveTextContent("Plain");

    expect(within(review).getByRole("radio", { name: "project" })).toBeChecked();
    expect(path()).toContain("Install deploy");
    expect(did(asked, "skill_install")).toBe(false);
    await user.click(within(review).getByRole("button", { name: "Install" }));
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    const install = asked.find((one) => one.cmd === "skill_install");
    expect(install?.args.reviewed).toEqual(ACCEPTED);
    expect(install?.args.level).toEqual(PROJECT);
    // An install goes on to its source's page.
    expect(await screen.findByTestId("skill-source-page")).toBeInTheDocument();
  });

  it("holds Install, saying why, until the review is read, and Cancel goes back", async () => {
    let answer: (review: unknown) => void = () => undefined;
    core({
      answers: {
        skill_install_review: () =>
          new Promise((resolve) => {
            answer = resolve;
          }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review deploy" }));
    const review = await screen.findByTestId("install-review");
    expect(within(review).getByRole("button", { name: "Install" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    answer(REVIEW);
    await within(review).findByTestId("install-commit");
    expect(within(review).getByRole("button", { name: "Install" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await user.click(within(review).getByRole("button", { name: "Cancel" }));
    expect(await screen.findByTestId("skill-source-page")).toBeInTheDocument();
  });
});

describe("the update review", () => {
  it("shows what changes, the source's diff and your edits, and changes nothing until accepted", async () => {
    const asked = core({ offers: [OFFER] });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    await screen.findByTestId("skill-updates");
    await user.click(screen.getByRole("button", { name: "Review update" }));
    const review = await screen.findByTestId("update-review");
    expect(await within(review).findByTestId("update-added")).toHaveTextContent("get.example.net");
    expect(within(review).getByText("bbbbbbb")).toBeInTheDocument();
    expect(within(review).getByText("ccccccc")).toBeInTheDocument();
    expect(within(review).getByTestId("update-edited")).toHaveTextContent(
      "Accepting replaces these: SKILL.md",
    );
    expect(path()).toContain("Update notify");
    expect(did(asked, "skill_update")).toBe(false);
    await user.click(within(review).getByRole("button", { name: "Accept update" }));
    await waitFor(() => expect(did(asked, "skill_update")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_update")?.args.reviewed).toEqual({
      commit: "c".repeat(40),
      skills: [{ name: "notify", digest: "sha256:n1" }],
    });
    expect(await screen.findByTestId("skill-sources")).toBeInTheDocument();
  });

  it("Not now leaves it as it is, and back walks the pages again", async () => {
    const asked = core({ offers: [OFFER] });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    const notify = await within(page).findByTestId("offered-notify");
    await user.click(within(notify).getByRole("button", { name: "Review the update of notify" }));
    const review = await screen.findByTestId("update-review");
    await user.click(within(review).getByRole("button", { name: "Not now" }));
    expect(await screen.findByTestId("skill-source-page")).toBeInTheDocument();
    expect(did(asked, "skill_update")).toBe(false);
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByTestId("update-review")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back" }));
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByTestId("skill-sources")).toBeInTheDocument();
  });
});

describe("the states of each page", () => {
  it("holds rows while the sources are read, and says what will appear when there are none", async () => {
    let answer: (sources: unknown) => void = () => undefined;
    core({
      answers: {
        skill_sources: () =>
          new Promise((resolve) => {
            answer = resolve;
          }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    expect(await screen.findByTestId("skill-sources-reading")).toHaveAttribute("aria-busy", "true");
    answer([]);
    const empty = await screen.findByTestId("skill-sources-empty");
    expect(empty).toHaveTextContent("This project has no skill sources yet");
    await user.click(within(empty).getByRole("button", { name: "Add source…" }));
    expect(await screen.findByRole("dialog", { name: "Add a source" })).toBeInTheDocument();
  });

  it("says in place when a source could not be read, and when it holds no skill", async () => {
    let refused = true;
    core({
      answers: {
        skill_source_browse: () => {
          if (refused) throw new Error("the network is unreachable");
          return { commit: commit("a"), skills: [], not_skills: [] };
        },
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    const unread = await within(page).findByTestId("skill-sources-unread");
    expect(unread).toHaveTextContent("purlis could not read example skills");
    expect(unread).toHaveTextContent("the network is unreachable");
    refused = false;
    await user.click(within(unread).getByRole("button", { name: "Try again" }));
    expect(await within(page).findByTestId("skill-source-no-skills")).toBeInTheDocument();
  });

  it("says when no update is waiting any more, with Accept update unavailable", async () => {
    // A review whose update was accepted elsewhere, reached by back, forward or a stale path.
    core();
    openViewAt(STUDIO_HISTORY, {
      page: "sources",
      scope: AT_THE_ROOT,
      sources: {
        at: "update",
        source: sourceKey(SOURCE.remote),
        label: SOURCE.label,
        skill: "notify",
      },
    });
    render(<SkillStudioTab plane={PLANE} changed={0} />);
    const review = await screen.findByTestId("update-review");
    expect(await within(review).findByTestId("update-none")).toHaveTextContent(
      "No update of notify is waiting",
    );
    expect(within(review).getByRole("button", { name: "Accept update" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });
});
