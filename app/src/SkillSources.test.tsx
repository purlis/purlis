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
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillStudioTab } from "./SkillStudioTab";
import { checkedAgo, checkedSaid } from "./SkillSources";
import { PLANE, STUDIO_HISTORY, forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { AT_THE_ROOT, sourceKey } from "./skillStudio";
import { openViewAt } from "./ui/panel/viewHistory";
import { PROJECT } from "./skillChanges";
import { pickIn } from "./ui/select/select.testkit";

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
  already: [],
  clash: null,
  notes: [],
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
      already: [],
      clash: null,
      notes: [],
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

/** The update review of notify: one file changed, and an edit made here since the install. */
const UPDATE = {
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
  grant: null,
  widenings: [],
  accepted: { commit: "c".repeat(40), skills: [{ name: "notify", digest: "sha256:n1" }] },
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
        return UPDATE;
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

/**
 * **The clock the ages are read against** (#4110), pinned, at moments of the day where a second is
 * about to turn. While the core answers, the clock moves on, as a read takes time on a runner. An
 * age the test works out when the answer comes, instead of when the window was drawn, then reads
 * a day short ("4 days" for 5).
 */
const MOMENTS = [
  "2026-10-30T00:00:00.000Z",
  "2026-10-30T09:41:59.999Z",
  "2026-10-30T23:59:59.999Z",
];

describe.each(MOMENTS)("what each source's last check found (#3840), at %s", (moment) => {
  const nowSecs = () => Math.floor(Date.now() / 1000);
  /** The moment the window was drawn, which the ages it shows are counted from. */
  const drawnSecs = Math.floor(Date.parse(moment) / 1000);
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(Date.parse(moment));
  });
  afterEach(() => {
    vi.useRealTimers();
  });
  /** The core's read takes a moment: the clock moves on before it answers. */
  const aMomentLater = () => vi.setSystemTime(Date.now() + 1);

  it("says its skills, how many are installed and when it was checked on its row, asking no source", async () => {
    const asked = core({
      answers: {
        skill_sources_checked: () => {
          aMomentLater();
          return [
            { remote: SOURCE.remote, at: drawnSecs - 3 * 3600 - 30, skills: 14, installed: 2 },
          ];
        },
        // The updates check has not answered: the row does not wait for it.
        skill_updates: () => new Promise(() => undefined),
      },
    });
    renderStudio(AT_THE_ROOT, "sources");
    const row = await screen.findByTestId("skill-source");
    await waitFor(() =>
      expect(row).toHaveTextContent(
        "https://example.com/skills · folder skills · 14 skills, 2 installed · checked 3 hours ago · The whole project",
      ),
    );
    expect(did(asked, "skill_source_browse")).toBe(false);
  });

  it("reads what was recorded again once the updates check has answered", async () => {
    let at: number | null = null;
    const asked = core({
      answers: {
        skill_sources_checked: () => [{ remote: SOURCE.remote, at, skills: null, installed: 1 }],
        skill_updates: () => {
          at = nowSecs();
          return { offers: [], unasked: [], unconfirmed: [] };
        },
      },
    });
    renderStudio(AT_THE_ROOT, "sources");
    const row = await screen.findByTestId("skill-source");
    await waitFor(() => expect(row).toHaveTextContent("1 installed · checked just now"));
    expect(times(asked, "skill_sources_checked")).toBeGreaterThan(1);
  });

  it("says when its page read it in the page's lead", async () => {
    core({
      answers: {
        skill_sources_checked: () => {
          aMomentLater();
          return [{ remote: SOURCE.remote, at: drawnSecs - 5 * 86_400, skills: 2, installed: 0 }];
        },
        skill_source_browse: () => ({ ...BROWSE, checked: nowSecs() }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const row = await screen.findByTestId("skill-source");
    await waitFor(() => expect(row).toHaveTextContent("checked 5 days ago"));
    const page = await openSource(user);
    await within(page).findByTestId("offered-deploy");
    expect(page).toHaveTextContent("folder skills · checked just now · The whole project");
  });

  it("reads what was recorded again on coming back from a source's page, which browsed it", async () => {
    let at: number | null = null;
    core({
      answers: {
        skill_sources_checked: () => [{ remote: SOURCE.remote, at, skills: 2, installed: 0 }],
        skill_source_browse: () => {
          at = nowSecs();
          return { ...BROWSE, checked: at };
        },
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await within(page).findByTestId("offered-deploy");
    await user.click(screen.getByRole("button", { name: "Back" }));
    const row = await screen.findByTestId("skill-source");
    await waitFor(() => expect(row).toHaveTextContent("2 skills, 0 installed · checked just now"));
  });

  it("says ages a person reads at a glance, and nothing where nothing is known", () => {
    const now = new Date(Date.UTC(2026, 9, 30, 12));
    expect(checkedAgo(-5, now)).toBe("checked just now");
    expect(checkedAgo(59, now)).toBe("checked just now");
    expect(checkedAgo(60, now)).toBe("checked 1 minute ago");
    expect(checkedAgo(3599, now)).toBe("checked 59 minutes ago");
    expect(checkedAgo(7200, now)).toBe("checked 2 hours ago");
    expect(checkedAgo(86_400, now)).toBe("checked 1 day ago");
    expect(checkedAgo(29 * 86_400, now)).toBe("checked 29 days ago");
    const remote = SOURCE.remote;
    expect(checkedSaid(undefined, now)).toBeNull();
    expect(checkedSaid({ remote, at: null, skills: null, installed: 0 }, now)).toBeNull();
    expect(checkedSaid({ remote, at: null, skills: 1, installed: 0 }, now)).toBe(
      "1 skill, 0 installed",
    );
  });
});

/** Runs `run` with the process's time zone set to `zone`, then puts it back. */
function inZone(zone: string, run: () => void) {
  const was = process.env.TZ;
  process.env.TZ = zone;
  try {
    run();
  } finally {
    if (was === undefined) delete process.env.TZ;
    else process.env.TZ = was;
  }
}

/**
 * **An age is the same in every time zone; a date is the day where the person is** (#4110). A
 * check 60 days before 12:00 UTC on 30 Oct was made at 12:00 UTC on 31 Aug, which is already
 * 1 Sep in Kiritimati (UTC+14).
 */
describe("a source's last check in any time zone (#4110)", () => {
  const now = new Date(Date.UTC(2026, 9, 30, 12));

  it.each([
    ["UTC", "checked 31 Aug 2026"],
    ["Pacific/Pago_Pago", "checked 31 Aug 2026"],
    ["America/Los_Angeles", "checked 31 Aug 2026"],
    ["Asia/Tokyo", "checked 31 Aug 2026"],
    ["Pacific/Kiritimati", "checked 1 Sept 2026"],
  ])("says the same age, and the day it was there, in %s", (zone, date) =>
    inZone(zone, () => {
      expect(checkedAgo(5 * 86_400, now)).toBe("checked 5 days ago");
      expect(checkedAgo(60 * 86_400, now)).toBe(date);
    }),
  );
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
    expect(
      within(deploy).getByRole("button", { name: "Review install: deploy" }),
    ).toBeInTheDocument();
    const notify = within(page).getByTestId("offered-notify");
    expect(notify).toHaveTextContent("Installed");
    expect(notify).toHaveTextContent("Update available");
    expect(
      within(notify).getByRole("button", { name: "Review update: notify" }),
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
    await user.click(within(notify).getByRole("button", { name: "See it in Library: notify" }));
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
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));

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

  it("reads the review again where it lands, and leaves a referred skill already there as it is (#4032)", async () => {
    const OPS = { at: "workspace", name: "ops", local: false };
    const asked = core({
      answers: {
        skill_studio: () => ({
          on: true,
          workspace: null,
          persona: null,
          workspaces: [],
          levels: [
            { level: PROJECT, label: "project" },
            { level: OPS, label: "ops" },
          ],
          harnesses: [],
          clashes: [],
          skills: [],
          first: { starts: false, unmoved: null },
        }),
        // The project holds notify already; ops does not.
        skill_install_review: (args) =>
          (args.level as { at: string }).at === "project"
            ? {
                ...REVIEW,
                skills: [{ ...REVIEW.skills[0], already: ["notify"] }],
                accepted: { ...ACCEPTED, skills: [ACCEPTED.skills[0]] },
              }
            : REVIEW,
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
    const review = await screen.findByTestId("install-review");
    expect(await within(review).findByTestId("already-deploy-notify")).toHaveTextContent(
      "It refers to notify, which its chats there have already: it is left as it is.",
    );
    expect(within(review).queryByTestId("reviewed-notify")).toBeNull();
    expect(within(review).getByTestId("install-commit")).toHaveTextContent("1 skill");
    const reviews = () => asked.filter((one) => one.cmd === "skill_install_review");
    expect(reviews().map((one) => one.args.level)).toEqual([PROJECT]);

    // Another level that does not hold it: read again, and notify comes along.
    await user.click(within(review).getByRole("radio", { name: "ops" }));
    expect(await within(review).findByTestId("reviewed-notify")).toBeInTheDocument();
    expect(within(review).queryByTestId("already-deploy-notify")).toBeNull();
    expect(reviews().map((one) => one.args.level)).toEqual([PROJECT, OPS]);
    await user.click(within(review).getByRole("button", { name: "Install" }));
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    const install = asked.find((one) => one.cmd === "skill_install");
    expect(install?.args.level).toEqual(OPS);
    expect(install?.args.reviewed).toEqual(ACCEPTED);
  });

  it("stops on a taken name until it is skipped or named, and hands the new name over (#4530)", async () => {
    const clash = {
      held_by: "the project skills have a deploy from mattpocock",
      offered: "example-deploy",
      taken: ["deploy", "notify"],
    };
    const asked = core({
      answers: {
        skill_install_review: () => ({
          ...REVIEW,
          skills: [{ ...REVIEW.skills[0], clash, notes: ["The ops skills have a deploy too."] }],
          accepted: { ...ACCEPTED, skills: [ACCEPTED.skills[0]] },
        }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
    const review = await screen.findByTestId("install-review");
    expect(await within(review).findByTestId("clash-deploy")).toHaveTextContent(
      "The project skills have a deploy from mattpocock.",
    );
    expect(within(review).getByTestId("note-deploy")).toHaveTextContent(
      "The ops skills have a deploy too.",
    );
    const install = within(review).getByRole("button", { name: "Install" });
    expect(install).toHaveAttribute("aria-disabled", "true");

    await user.click(within(review).getByRole("radio", { name: "Skip it" }));
    expect(install).toHaveAttribute("aria-disabled", "true");

    await user.click(within(review).getByRole("radio", { name: "Install it under a new name" }));
    const named = within(review).getByRole("textbox", { name: "New name for deploy" });
    expect(named).toHaveValue("example-deploy");
    expect(install).not.toHaveAttribute("aria-disabled", "true");

    await user.clear(named);
    await user.type(named, "notify");
    expect(install).toHaveAttribute("aria-disabled", "true");

    await user.clear(named);
    await user.type(named, "acme-deploy");
    await user.click(install);
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_install")?.args.renames).toEqual([
      { from: "deploy", to: "acme-deploy" },
    ]);
  });

  it("has nothing to install once the one skill named is skipped, whatever it brought (#4530)", async () => {
    const clash = { held_by: "held", offered: "example-deploy", taken: ["deploy"] };
    core({
      answers: {
        skill_install_review: () => ({
          ...REVIEW,
          skills: [{ ...REVIEW.skills[0], clash }, REVIEW.skills[1]],
        }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
    const review = await screen.findByTestId("install-review");
    await within(review).findByTestId("clash-deploy");
    await user.click(within(review).getByRole("radio", { name: "Skip it" }));
    expect(within(review).getByRole("button", { name: "Install" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("refuses as typed the name it had, a built-in's, a sibling's and a long one (#4530)", async () => {
    const clash = { held_by: "held", offered: "example-deploy", taken: ["deploy", "secrets"] };
    core({
      answers: {
        skill_install_review: () => ({
          ...REVIEW,
          skills: [{ ...REVIEW.skills[0], clash }, REVIEW.skills[1]],
        }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
    const review = await screen.findByTestId("install-review");
    await within(review).findByTestId("clash-deploy");
    await user.click(within(review).getByRole("radio", { name: "Install it under a new name" }));
    const named = within(review).getByRole("textbox", { name: "New name for deploy" });
    const install = within(review).getByRole("button", { name: "Install" });
    for (const bad of ["deploy", "secrets", "notify", "a".repeat(65)]) {
      await user.clear(named);
      await user.type(named, bad);
      expect(install).toHaveAttribute("aria-disabled", "true");
    }
  });

  it("offers the grant at the project level and says what it widens before Install (#4185)", async () => {
    const OPS = { at: "workspace", name: "ops", local: false };
    let widened: (answer: unknown) => void = () => undefined;
    const asked = core({
      answers: {
        skill_studio: () => ({
          on: true,
          workspace: null,
          persona: null,
          workspaces: ["ops", "web"],
          levels: [
            { level: PROJECT, label: "project" },
            { level: OPS, label: "ops" },
          ],
          harnesses: [],
          clashes: [],
          skills: [],
          first: { starts: false, unmoved: null },
        }),
        // The project holds notify already, granted to the root alone.
        skill_install_review: () => ({
          ...REVIEW,
          skills: [{ ...REVIEW.skills[0], already: ["notify"] }],
          accepted: { ...ACCEPTED, skills: [ACCEPTED.skills[0]] },
        }),
        skill_install_widenings: () =>
          new Promise((resolve) => {
            widened = resolve;
          }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
    const review = await screen.findByTestId("install-review");
    expect(await within(review).findByTestId("already-deploy-notify")).toHaveTextContent(
      "It refers to notify, which its chats there have already: it is left as it is.",
    );
    // The root's grant widens nothing, so nothing is asked.
    expect(did(asked, "skill_install_widenings")).toBe(false);

    await pickIn(
      await within(review).findByLabelText("Where deploy will reach"),
      "The root and ops",
      user,
    );
    await waitFor(() => expect(did(asked, "skill_install_widenings")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_install_widenings")?.args).toEqual({
      plane: PLANE,
      level: PROJECT,
      grant: { to: "only", workspaces: ["ops"] },
      already: ["notify"],
    });
    // Install waits until what it widens is shown.
    const install = within(review).getByRole("button", { name: "Install" });
    expect(install).toHaveAttribute("aria-disabled", "true");
    widened([{ name: "notify", granted: "root", added: "ops", turned_off: ["web"] }]);
    const line = await within(review).findByText(/which is granted to root/);
    expect(within(review).getByTestId("already-deploy-notify")).toHaveTextContent(
      "It refers to notify, which is granted to root: it is left as it is, and the install grants it to ops too.",
    );
    expect(line).toBeInTheDocument();
    expect(within(review).getByTestId("already-deploy-notify")).toHaveTextContent(
      "web turned notify off, so its chats get deploy without it.",
    );
    await waitFor(() => expect(install).not.toHaveAttribute("aria-disabled", "true"));
    await user.click(install);
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    expect(asked.find((one) => one.cmd === "skill_install")?.args.grant).toEqual({
      to: "only",
      workspaces: ["ops"],
    });
  });

  it("offers no grant where the install lands in a workspace, and installs with none (#4185)", async () => {
    const OPS = { at: "workspace", name: "ops", local: false };
    const asked = core({
      answers: {
        skill_studio: () => ({
          on: true,
          workspace: null,
          persona: null,
          workspaces: ["ops"],
          levels: [
            { level: PROJECT, label: "project" },
            { level: OPS, label: "ops" },
          ],
          harnesses: [],
          clashes: [],
          skills: [],
          first: { starts: false, unmoved: null },
        }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
    const review = await screen.findByTestId("install-review");
    expect(await within(review).findByLabelText("Where deploy will reach")).toBeInTheDocument();
    await user.click(within(review).getByRole("radio", { name: "ops" }));
    await waitFor(() =>
      expect(within(review).queryByLabelText("Where deploy will reach")).toBeNull(),
    );
    await user.click(await within(review).findByRole("button", { name: "Install" }));
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    const install = asked.find((one) => one.cmd === "skill_install");
    expect(install?.args.level).toEqual(OPS);
    expect(install?.args.grant).toBeNull();
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
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
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

  it("leaves a person who went to the Library before the install ended in the Library (#3973)", async () => {
    let installed: (answer: unknown) => void = () => undefined;
    const asked = core({
      answers: {
        skill_install: () =>
          new Promise((resolve) => {
            installed = resolve;
          }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
    const review = await screen.findByTestId("install-review");
    await within(review).findByTestId("install-commit");
    await user.click(within(review).getByRole("button", { name: "Install" }));
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    // The Sources page is left while the install is still being made.
    await user.click(screen.getByRole("button", { name: "Library" }));
    await waitFor(() => expect(screen.queryByTestId("install-review")).toBeNull());
    const reads = times(asked, "skill_studio");
    installed({ notes: [] });
    // The install's end reads the skills again, and leaves the tab where the person went.
    await waitFor(() => expect(times(asked, "skill_studio")).toBeGreaterThan(reads));
    expect(screen.getByRole("button", { name: "Library" })).toHaveAttribute("aria-current", "page");
    expect(screen.queryByTestId("skill-source-page")).toBeNull();
  });

  it("still goes on to the source's page for a person who came back to the review before the install ended", async () => {
    let installed: (answer: unknown) => void = () => undefined;
    const asked = core({
      answers: {
        skill_install: () =>
          new Promise((resolve) => {
            installed = resolve;
          }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await user.click(await within(page).findByRole("button", { name: "Review install: deploy" }));
    await within(await screen.findByTestId("install-review")).findByTestId("install-commit");
    await user.click(
      within(screen.getByTestId("install-review")).getByRole("button", { name: "Install" }),
    );
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    // Away and back: the review is drawn again, and the tab is where Install was pressed.
    await user.click(screen.getByRole("button", { name: "Library" }));
    await waitFor(() => expect(screen.queryByTestId("install-review")).toBeNull());
    await user.click(screen.getByRole("button", { name: "Back" }));
    await screen.findByTestId("install-review");
    installed({ notes: [] });
    expect(await screen.findByTestId("skill-source-page")).toBeInTheDocument();
  });
});

/** The one review of `names` as the core answers it (#4024): each named, notify brought along by
 *  deploy, and what accepting says of exactly those; those of `there` the level holds already,
 *  left out (#4153). */
function setReview(names: readonly string[], there: readonly string[] = []) {
  const HUGE = {
    ...NOTIFY,
    name: "huge",
    description: "Big.",
    folder: "skills/huge",
    brought_by: null,
  };
  const reviewed = names.filter((name) => !there.includes(name));
  const skills = [
    ...(reviewed.includes("deploy") ? [REVIEW.skills[0], NOTIFY] : []),
    ...(reviewed.includes("huge") ? [HUGE] : []),
  ];
  return {
    kind: "review",
    there: names.filter((name) => there.includes(name)),
    review: {
      commit: commit("a"),
      skills,
      accepted: {
        commit: "a".repeat(40),
        skills: skills.map((one) => ({ name: one.name, digest: `sha256:${one.name}` })),
      },
    },
  };
}

/** Opens Install all… from the source's page. */
async function openInstallAll(user: ReturnType<typeof userEvent.setup>) {
  const page = await openSource(user);
  await within(page).findByTestId("offered-deploy");
  await user.click(within(page).getByRole("button", { name: "Install all…" }));
  return screen.findByTestId("install-all-choose");
}

describe("Install all… (#4024)", () => {
  it("ticks every skill not installed yet, greys the installed one, and reviews those ticked", async () => {
    const asked = core({
      answers: {
        skill_install_set_review: (args) => setReview(args.names as string[]),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    expect(path()).toContain("Install all");
    expect(await within(choose).findByTestId("install-all-lead")).toHaveTextContent(
      "pinned to commit aaaaaaa",
    );
    const box = (name: string) =>
      within(within(choose).getByTestId(`choose-${name}`)).getByRole("checkbox");
    expect(box("deploy")).toBeChecked();
    expect(box("huge")).toBeChecked();
    // Installed: greyed, its box out of reach, with its way to the Library.
    expect(box("notify")).not.toBeChecked();
    expect(box("notify")).toBeDisabled();
    const notify = within(choose).getByTestId("choose-notify");
    expect(notify).toHaveTextContent("Installed");
    expect(
      within(notify).getByRole("button", { name: "See it in Library: notify" }),
    ).toBeInTheDocument();
    expect(within(choose).getByTestId("choose-deploy")).toHaveTextContent("runs a script");

    // Untick to narrow.
    await user.click(box("huge"));
    expect(box("huge")).not.toBeChecked();
    expect(did(asked, "skill_install_set_review")).toBe(false);
    await user.click(within(choose).getByRole("button", { name: "Review 1 skill" }));

    const review = await screen.findByTestId("install-all-review");
    expect(path()).toContain("Review");
    expect(asked.find((one) => one.cmd === "skill_install_set_review")?.args.names).toEqual([
      "deploy",
    ]);
    // Read for where they land, so a skill one refers to that is there already stays out (#4032).
    expect(asked.find((one) => one.cmd === "skill_install_set_review")?.args.level).toEqual(
      PROJECT,
    );
    expect(await within(review).findByTestId("install-commit")).toHaveTextContent(
      "at commit aaaaaaa: 2 skills, 4 risky items marked",
    );
    // One level for all of them, chosen at the top.
    expect(within(review).getAllByRole("radio", { name: "project" })).toHaveLength(1);
    // Each skill's risky items on its box; the one brought along follows the one that brought it.
    expect(within(review).getByTestId("keep-deploy")).toHaveTextContent(
      "Install deploy (4 risky items)",
    );
    expect(within(within(review).getByTestId("keep-notify")).getByRole("checkbox")).toBeDisabled();
    expect(within(review).getByText(/Comes along: deploy refers to it/)).toBeInTheDocument();
    // Its files are read when unfolded.
    expect(within(review).queryByTestId("file-scripts/ship.sh")).toBeNull();
    await user.click(within(review).getByRole("button", { name: "Its files (2)" }));
    expect(within(review).getByTestId("file-scripts/ship.sh")).toHaveTextContent("Look closely");

    await user.click(within(review).getByRole("button", { name: "Install" }));
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    const install = asked.find((one) => one.cmd === "skill_install");
    expect(install?.args.reviewed).toEqual(setReview(["deploy"]).review.accepted);
    expect(install?.args.level).toEqual(PROJECT);
    expect(await screen.findByTestId("skill-source-page")).toBeInTheDocument();
  });

  it("shows a ticked skill the level holds already as there, unticked, and installs the rest (#4153)", async () => {
    const asked = core({
      answers: {
        // The project holds huge already, made there by hand.
        skill_install_set_review: (args) => setReview(args.names as string[], ["huge"]),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    await user.click(await within(choose).findByRole("button", { name: "Review 2 skills" }));
    const review = await screen.findByTestId("install-all-review");
    const huge = await within(review).findByTestId("there-huge");
    expect(within(huge).getByRole("checkbox")).not.toBeChecked();
    expect(within(huge).getByRole("checkbox")).toBeDisabled();
    expect(within(review).getByText(/so it is left as it is/)).toBeInTheDocument();
    expect(within(review).queryByTestId("keep-huge")).toBeNull();
    expect(within(review).queryByTestId("left-out-huge")).toBeNull();

    await user.click(within(review).getByRole("button", { name: "Install" }));
    await waitFor(() => expect(did(asked, "skill_install")).toBe(true));
    const install = asked.find((one) => one.cmd === "skill_install");
    expect(install?.args.reviewed).toEqual(setReview(["deploy", "huge"], ["huge"]).review.accepted);
  });

  it("asks again when a skipped skill is ticked back in (#4530)", async () => {
    const clash = { held_by: "held", offered: "example-huge", taken: ["huge"] };
    core({
      answers: {
        skill_install_set_review: (args) => {
          const set = setReview(args.names as string[]);
          return {
            ...set,
            review: {
              ...set.review,
              skills: set.review.skills.map((one) =>
                one.name === "huge" ? { ...one, clash } : one,
              ),
            },
          };
        },
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    await user.click(await within(choose).findByRole("button", { name: "Review 2 skills" }));
    const review = await screen.findByTestId("install-all-review");
    const taken = await within(review).findByTestId("clash-huge");
    await user.click(within(taken).getByRole("radio", { name: "Skip it" }));
    const left = await within(review).findByTestId("left-out-huge");
    await user.click(within(left).getByRole("checkbox"));

    const back = await within(review).findByTestId("clash-huge");
    expect(within(back).getByRole("radio", { name: "Skip it" })).not.toBeChecked();
    expect(within(review).getByRole("button", { name: "Install" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("keeps a skill there already named when it reviews the first part (#4153)", async () => {
    let first = true;
    const asked = core({
      answers: {
        // A third skill, lint, to leave for the second part.
        skill_source_browse: () => ({
          ...BROWSE,
          skills: [
            ...BROWSE.skills,
            {
              name: "lint",
              description: "Lints.",
              folder: "skills/lint",
              installed: [],
              marks: [],
            },
          ],
        }),
        // The first review is past one read: deploy fits, lint is the rest, and the level holds
        // huge already.
        skill_install_set_review: (args) => {
          if (!first) return setReview(args.names as string[], ["huge"]);
          first = false;
          return {
            kind: "too-much",
            fits: ["deploy"],
            rest: ["lint"],
            why: "example skills holds more than 64 MiB of skills to read, the most purlis reads of one skill source at once",
            there: ["huge"],
          };
        },
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    await user.click(await within(choose).findByRole("button", { name: "Review 3 skills" }));
    const review = await screen.findByTestId("install-all-review");
    await user.click(await within(review).findByRole("button", { name: "Review the first skill" }));
    expect(await within(review).findByTestId("there-huge")).toBeInTheDocument();
    expect(within(review).queryByTestId("left-out-huge")).toBeNull();
    const reviews = asked.filter((one) => one.cmd === "skill_install_set_review");
    expect(reviews.at(-1)?.args.names).toEqual(["deploy", "huge"]);
  });

  it("holds Install where every ticked skill is there already (#4153)", async () => {
    core({
      answers: {
        skill_install_set_review: (args) =>
          setReview(args.names as string[], args.names as string[]),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    await user.click(await within(choose).findByRole("button", { name: "Review 2 skills" }));
    const review = await screen.findByTestId("install-all-review");
    await within(review).findByTestId("there-deploy");
    expect(within(review).getByRole("button", { name: "Install" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });

  it("keeps the list as it was ticked when the person comes back from the review", async () => {
    core({
      answers: {
        skill_install_set_review: (args) => setReview(args.names as string[]),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    const huge = () =>
      within(within(screen.getByTestId("install-all-choose")).getByTestId("choose-huge")).getByRole(
        "checkbox",
      );
    await within(choose).findByTestId("choose-huge");
    await user.click(huge());
    await user.click(within(choose).getByRole("button", { name: "Review 1 skill" }));
    const review = await screen.findByTestId("install-all-review");
    await user.click(within(review).getByRole("button", { name: "Cancel" }));
    await screen.findByTestId("install-all-choose");
    await waitFor(() => expect(huge()).not.toBeChecked());
  });

  it("says Update waiting on a skill whose update waits, and sends it to Review update", async () => {
    core({ offers: [OFFER] });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    const notify = await within(choose).findByTestId("choose-notify");
    expect(notify).toHaveTextContent("Update waiting");
    expect(within(notify).getByRole("checkbox")).toBeDisabled();
    await user.click(within(notify).getByRole("button", { name: "Review update: notify" }));
    expect(await screen.findByTestId("update-review")).toBeInTheDocument();
  });

  it("leaves a skill out from the review itself, reviewing the rest again, and brings it back", async () => {
    const asked = core({
      answers: {
        skill_install_set_review: (args) => setReview(args.names as string[]),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    await user.click(await within(choose).findByRole("button", { name: "Review 2 skills" }));
    const review = await screen.findByTestId("install-all-review");
    await within(review).findByTestId("keep-huge");
    await user.click(within(within(review).getByTestId("keep-deploy")).getByRole("checkbox"));
    await waitFor(() => expect(within(review).queryByTestId("keep-deploy")).toBeNull());
    const reviews = asked.filter((one) => one.cmd === "skill_install_set_review");
    expect(reviews.at(-1)?.args.names).toEqual(["huge"]);
    expect(within(review).getByTestId("left-out-deploy")).toHaveTextContent("Install deploy");
    await user.click(within(within(review).getByTestId("left-out-deploy")).getByRole("checkbox"));
    await within(review).findByTestId("keep-deploy");
    const again = asked.filter((one) => one.cmd === "skill_install_set_review");
    expect(again.at(-1)?.args.names).toEqual(["deploy", "huge"]);
  });

  it("says when the set is more than one review can read, and offers the first part", async () => {
    const asked = core({
      answers: {
        skill_install_set_review: (args) =>
          (args.names as string[]).length > 1
            ? {
                kind: "too-much",
                fits: ["deploy"],
                rest: ["huge"],
                why: "example skills holds more than 64 MiB of skills to read, the most purlis reads of one skill source at once",
                there: [],
              }
            : setReview(args.names as string[]),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const choose = await openInstallAll(user);
    await user.click(await within(choose).findByRole("button", { name: "Review 2 skills" }));
    const review = await screen.findByTestId("install-all-review");
    const said = await within(review).findByTestId("install-all-too-much");
    expect(said).toHaveTextContent("more than 64 MiB of skills to read");
    expect(said).toHaveTextContent(
      "Install them in two parts: the first skill now, then the other 1 with Install all… again.",
    );
    expect(within(review).getByRole("button", { name: "Install" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await user.click(within(said).getByRole("button", { name: "Review the first skill" }));
    await within(review).findByTestId("keep-deploy");
    expect(
      asked.filter((one) => one.cmd === "skill_install_set_review").at(-1)?.args.names,
    ).toEqual(["deploy"]);
  });

  it("cannot be pressed when every skill of the source is installed", async () => {
    core({
      answers: {
        skill_source_browse: () => ({
          ...BROWSE,
          skills: BROWSE.skills.map((one) => ({ ...one, installed: ["project"] })),
        }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    await within(page).findByTestId("offered-deploy");
    expect(within(page).getByRole("button", { name: "Install all…" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
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

  it("says where it reaches: what comes along lands with its grant, and what it widens (#4186)", async () => {
    core({
      offers: [OFFER],
      answers: {
        skill_update_review: () => ({
          ...UPDATE,
          skill: { ...NOTIFY, brought_by: null, already: ["lint"] },
          brought: [{ ...NOTIFY, name: "format", brought_by: "notify" }],
          grant: { to: "only", workspaces: ["ops"] },
          widenings: [{ name: "lint", granted: "root", added: "ops", turned_off: [] }],
        }),
      },
    });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    await screen.findByTestId("skill-updates");
    await user.click(screen.getByRole("button", { name: "Review update" }));
    const review = await screen.findByTestId("update-review");
    const reach = await within(review).findByRole("region", { name: "Where it reaches" });
    expect(within(reach).getByTestId("update-brought-grant")).toHaveTextContent(
      "format, which comes along, reaches the root and ops, as notify does.",
    );
    expect(within(reach).getByTestId("already-notify-lint")).toHaveTextContent(
      "It refers to lint, which is granted to root: it is left as it is, and the update grants it to ops too.",
    );
    // Said once: not again in the folded review of notify.
    expect(within(review).getAllByTestId("already-notify-lint")).toHaveLength(1);
  });

  it("Not now leaves it as it is, and back walks the pages again", async () => {
    const asked = core({ offers: [OFFER] });
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "sources");
    const page = await openSource(user);
    const notify = await within(page).findByTestId("offered-notify");
    await user.click(within(notify).getByRole("button", { name: "Review update: notify" }));
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
