// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Sources pages** (#3687, `prototype/skill-sources` variant A): Sources with an update
 * waiting, a source not confirmed here, your sources and the ones to start from; Add source…; a
 * source's page with its skills marked; the install review and the update review, each with its
 * action fixed at the foot. Walked by the keyboard, held to axe, and pictured in every pictured
 * theme cell. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { AT_THE_ROOT, sourceKey, type SourcesAt } from "./skillStudio";
import { PROJECT } from "./skillChanges";
import { memberKey } from "./SkillSetReview";
import { studio } from "./skillsListing.testkit";
import { updateOffer, updateReview } from "./skillUpdates.testkit";
import { extensionKey } from "./studioSources";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, tabInto, untilFocusWithin } from "./stories.testkit";

const commit = (c: string) => ({ id: c.repeat(40), short: c.repeat(7) });

const SUPERPOWERS = {
  remote: {
    url: "https://github.com/obra/superpowers",
    branch: null,
    label: "https://github.com/obra/superpowers",
  },
  folder: "skills",
  vault: null,
  key: null,
  kept: "shared",
  label: "Superpowers",
} as const;

const PLATFORM = {
  remote: { url: "https://git.example.com/platform/skills", branch: "main", label: "platform" },
  folder: null,
  vault: "forge-token",
  key: "TOKEN",
  kept: "local",
  label: "platform-skills",
} as const;

/** What is installed from the source, at two levels (#5331). */
const COPIES = [
  {
    member: { level: PROJECT, name: "brainstorming", label: "project" },
    changed: false,
    on_here: null,
  },
  {
    member: { level: PROJECT, name: "writing-plans", label: "project" },
    changed: false,
    on_here: null,
  },
  {
    member: { level: PROJECT, name: "systematic-debugging", label: "project" },
    changed: true,
    on_here: null,
  },
  {
    member: {
      level: { at: "project", local: true } as const,
      name: "brainstorming",
      label: "local",
    },
    changed: false,
    on_here: null,
  },
];

/** The review of `members`: one taken out, one left naming another, two open chats. */
const SET_REVIEW = ({ act, members }: { act: string; members: { name: string }[] }) => ({
  skills: members.map((member) => ({
    member,
    changed: member.name === "systematic-debugging",
    // What takes one out of a delete: a turn writes nothing of the skill's.
    cannot:
      act === "delete" && member.name === "writing-plans"
        ? "It holds changes that are not committed yet: save them first."
        : null,
  })),
  names: [
    act === "turn-off"
      ? null
      : act === "turn-on"
        ? {
            // One turned on names one that stays off, which is not among those ticked.
            namer: "brainstorming",
            named: "writing-plans",
            member: { level: PROJECT, name: "writing-plans", label: "project" },
          }
        : {
            namer: "to-spec",
            named: "brainstorming",
            member: { level: PROJECT, name: "brainstorming", label: "project" },
          },
  ].filter((one) => one !== null),
  open_chats: 2,
});

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

const SCRIPTED = {
  ...EMPTY,
  scripts: [
    {
      path: "scripts/repro.sh",
      runs: "sh",
      risks: [{ kind: "network", said: "reaches the network" }],
    },
  ],
  hosts: [{ host: "deploy.example.com", files: ["scripts/repro.sh"], risky: true }],
  marked: 2,
  empty: false,
};

const DEBUGGING = {
  name: "systematic-debugging",
  description: "Finds the cause of a bug before fixing it, with a loop that goes red.",
  folder: "skills/systematic-debugging",
  brought_by: null,
  already: [],
  clash: null,
  notes: [],
  summary: SCRIPTED,
  files: [
    {
      path: "SKILL.md",
      size: 640,
      executable: false,
      text: "---\nname: systematic-debugging\ndescription: Finds the cause of a bug before fixing it.\n---\n\n1. Reproduce it with scripts/repro.sh.\n2. …\n",
      risky: false,
    },
    {
      path: "scripts/repro.sh",
      size: 64,
      executable: true,
      text: '#!/bin/sh\nset -eu\ncurl -fsS https://deploy.example.com/health\ncargo test --quiet "$1"\n',
      risky: true,
    },
  ],
  left_out: [],
  lint: [],
  grade: null,
};

/** The grade superpowers published for systematic-debugging (#4346): the newest of two. */
const PUBLISHED = (thisVersion: boolean) => ({
  whose: "superpowers",
  newest: {
    date: "2026-11-02",
    harness: "claude-code",
    grader: "built-in",
    with: 92,
    without: 40,
    runs: 3,
    ended: "Finished",
  },
  this_version: thisVersion,
  more: 1,
  left_out: null,
});

/** The grade each story's skill carries, by the name its review is asked for. */
const GRADES: Record<string, unknown> = {
  "graded-this-version": PUBLISHED(true),
  "graded-earlier": PUBLISHED(false),
  "graded-malformed": {
    whose: "superpowers",
    newest: null,
    this_version: false,
    more: 0,
    left_out: 'a grade\'s "with" rate is 140, outside 0 to 100',
  },
};

const OFFER = {
  name: "brainstorming",
  level: PROJECT,
  label: "project",
  source: { kind: "git", remote: SUPERPOWERS.remote, label: SUPERPOWERS.remote.label },
  pinned: commit("3"),
  newer: commit("8"),
  edited: true,
};

/** brainstorming's newer commit, too young to take for three days and a few hours (#6031). */
const YOUNG = {
  kind: "too-young",
  why: "8888888 was committed 4 days ago, and this project installs only commits at least 7 days old: it can be installed in 3 days",
  until: Math.floor(Date.now() / 1000) + 3 * 24 * 60 * 60 - 3600,
};

/** An org's controls (#6031): Superpowers allowed, platform-skills not, one skill blocked. */
const POLICY = {
  file: "purlis.toml",
  state: {
    kind: "set",
    allow: ["https://github.com/obra/superpowers", "the extension acme-registry"],
    block: ["systematic-debugging from https://github.com/obra/superpowers"],
    min_age_days: 7,
    exempt: [],
    said: "This project's controls apply: 2 sources allowed, 1 skill blocked, commits at least 7 days old.",
  },
  sources: [
    {
      origin: { kind: "git", remote: SUPERPOWERS.remote, label: SUPERPOWERS.remote.label },
      kind: null,
      refused: null,
      only: [],
    },
    {
      origin: { kind: "git", remote: PLATFORM.remote, label: PLATFORM.remote.label },
      kind: "not-allowed",
      refused:
        "https://git.example.com/platform/skills (main) is not on this project's allow list of skill sources",
      only: [],
    },
  ],
};

/** test-driven-development as its review shows it: instructions only. */
const TDD = {
  ...DEBUGGING,
  name: "test-driven-development",
  description: "Builds each behaviour test-first, one red-green slice at a time.",
  folder: "skills/test-driven-development",
  summary: EMPTY,
  files: [
    {
      path: "SKILL.md",
      size: 512,
      executable: false,
      text: "---\nname: test-driven-development\ndescription: Builds each behaviour test-first.\n---\n\n1. Write one failing test.\n2. Make it pass.\n",
      risky: false,
    },
  ],
};

/** Seconds since the epoch, `ago` seconds ago: the checks are ages, so the pictures hold. */
const secondsAgo = (ago: number) => Math.floor(Date.now() / 1000) - ago;

/** The faked core: two sources, one update waiting, one source not confirmed here; the one
 *  review of Install all… is more than one read where `tooMuch` says so (#4024). */
function core({
  tooMuch = false,
  twoUpdates = false,
  overBudget = false,
  extension = false,
  controls = false,
}: {
  tooMuch?: boolean;
  /** An org's `[skill_policy]` (#6031): platform-skills not on its allow list, systematic-debugging
   *  blocked, and the update of brainstorming too young. */
  controls?: boolean;
  /** An extension that declares `skill-source` too, listed beside the git sources (#3092). */
  extension?: boolean;
  /** A second update waiting from Superpowers, for Review updates… (#5354). */
  twoUpdates?: boolean;
  overBudget?: boolean;
} = {}) {
  mockIPC((cmd, args) => {
    switch (cmd) {
      case "skill_sources_checked":
        return [
          { remote: SUPERPOWERS.remote, at: secondsAgo(3 * 3600 + 60), skills: 14, installed: 1 },
          { remote: PLATFORM.remote, at: null, skills: null, installed: 1 },
        ];
      case "skill_studio":
        return studio(
          [],
          overBudget
            ? {
                router: {
                  said: "Chats here are routed by the built-in router.",
                  problem: false,
                  extension: null,
                  chats: [],
                  grade: null,
                  graded: null,
                  written: null,
                  budget: [
                    {
                      program: "Claude Code",
                      key: "claude",
                      skills: 60,
                      chars: 15000,
                      skills_said: "60, Claude Code's default",
                      chars_said: "15,000, Claude Code's default",
                      skills_here: null,
                      chars_here: null,
                    },
                  ],
                  past_limits: [],
                  preview: "",
                },
              }
            : {},
        );
      case "skill_budget_preview":
        return {
          listed: 59,
          hidden: 0,
          now_listed: [],
          now_hidden: [],
          more_tokens: 0,
          past_limit: null,
          over: null,
        };
      case "skill_sources":
        return [SUPERPOWERS, PLATFORM];
      case "skill_sources_offered":
        return [
          {
            offering: "well-known",
            name: "Anthropic's skills",
            why: "The collection the open skill format began with: documents, design, building with Claude.",
            source: {
              ...SUPERPOWERS,
              remote: { ...SUPERPOWERS.remote, url: "https://github.com/anthropics/skills" },
            },
          },
          {
            offering: "well-known",
            name: "OpenAI's curated skills",
            why: "The skills OpenAI curates for Codex, kept by its makers.",
            source: {
              ...SUPERPOWERS,
              remote: { ...SUPERPOWERS.remote, url: "https://github.com/openai/skills" },
            },
          },
        ];
      case "skill_policy":
        return controls ? POLICY : { file: "purlis.toml", state: { kind: "none" }, sources: [] };
      case "skill_updates":
        return {
          offers: controls
            ? [{ ...OFFER, refused: YOUNG }]
            : twoUpdates
              ? [OFFER, updateOffer("writing-plans", SUPERPOWERS.remote)]
              : [OFFER],
          unasked: [],
          unconfirmed: [{ remote: PLATFORM.remote, skills: ["deploy-checks"] }],
        };
      case "skill_extension_sources":
        return extension ? [ACME] : [];
      case "skill_extension_source_browse":
        return {
          commit: { id: "2026.11.3", short: "2026.11.3" },
          checked: secondsAgo(30),
          skills: [
            {
              name: "release-notes",
              description: "Drafts the release notes from what merged since the last tag.",
              folder: "release-notes",
              installed: [],
              // What its files do, read to browse as a git source's are (#5760).
              marks: ["runs a script"],
            },
            {
              name: "incident-review",
              description: "Walks a blameless incident review, timeline first.",
              folder: "incident-review",
              installed: ["project"],
              marks: [],
            },
          ],
          not_skills: [],
        };
      case "skill_source_browse":
        return {
          commit: commit("3"),
          checked: secondsAgo(5 * 60 + 30),
          skills: [
            {
              name: "brainstorming",
              description: "Turns a loose idea into a short design before any code is written.",
              folder: "skills/brainstorming",
              installed: ["project"],
              marks: [],
            },
            {
              name: "test-driven-development",
              description: "Builds each behaviour test-first, one red-green slice at a time.",
              folder: "skills/test-driven-development",
              installed: [],
              marks: [],
            },
            {
              name: "systematic-debugging",
              description: DEBUGGING.description,
              folder: DEBUGGING.folder,
              installed: [],
              marks: ["runs a script", "reaches 1 host"],
              refused: controls
                ? {
                    kind: "blocked",
                    why: "systematic-debugging from https://github.com/obra/superpowers is on this project's block list",
                    until: null,
                  }
                : null,
            },
          ],
          not_skills: [
            { folder: "docs", why: "it holds no SKILL.md" },
            { folder: "scripts", why: "it holds no SKILL.md" },
          ],
        };
      case "skill_install_review":
        if ((args as { names: string[] }).names[0] === "taken-by-another")
          return {
            commit: commit("3"),
            skills: [
              {
                ...DEBUGGING,
                clash: {
                  held_by: "the project skills have a systematic-debugging from mattpocock",
                  offered: "superpowers-systematic-debugging",
                  taken: ["systematic-debugging"],
                },
                notes: [
                  "The ops skills have a systematic-debugging too: where a chat gets both, the narrowest copy wins.",
                ],
              },
            ],
            accepted: { commit: "3".repeat(40), skills: [] },
          };
        // It names test-driven-development bare, so a hint brings it along (#5874), unless the
        // person unticked it.
        if ((args as { names: string[] }).names[0] === "hints-at-tdd")
          return {
            commit: commit("3"),
            skills: [
              { ...DEBUGGING, hints: ["test-driven-development"] },
              ...((args as { without: string[] }).without.includes("test-driven-development")
                ? []
                : [{ ...TDD, brought_by: DEBUGGING.name, brought_by_hint: true }]),
            ],
            accepted: { commit: "3".repeat(40), skills: [] },
          };
        if ((args as { names: string[] }).names[0] in GRADES)
          return {
            commit: commit("3"),
            skills: [{ ...DEBUGGING, grade: GRADES[(args as { names: string[] }).names[0]] }],
            accepted: { commit: "3".repeat(40), skills: [] },
          };
        return {
          commit: commit("3"),
          // It refers to a skill the level holds already, left as it is (#4032).
          skills: [{ ...DEBUGGING, already: ["test-driven-development"] }],
          accepted: { commit: "3".repeat(40), skills: [] },
        };
      case "skill_install_widenings":
        // test-driven-development is the project's, granted to the root alone (#4185).
        return [{ name: "test-driven-development", granted: "root", added: "ops", turned_off: [] }];
      case "skill_install_set_review": {
        const names = (args as { names: string[] }).names;
        if (tooMuch)
          return {
            kind: "too-much",
            fits: names.slice(0, 1),
            rest: names.slice(1),
            why: "https://github.com/obra/superpowers holds more than 64 MiB of skills to read, the most purlis reads of one skill source at once",
            there: [],
          };
        const skills = [TDD, DEBUGGING].filter((one) => names.includes(one.name));
        return {
          kind: "review",
          // Those ticked that the level holds already, made there by hand (#4153).
          there: names.filter((name) => name === "writing-plans"),
          review: {
            commit: commit("3"),
            skills,
            accepted: { commit: "3".repeat(40), skills: [] },
          },
        };
      }
      case "skill_update_review":
        // Review updates… (#5354): writing-plans' update newly reaches the network.
        if ((args as { name: string }).name === "writing-plans")
          return updateReview("writing-plans", ["script", "network"]);
        return {
          name: "brainstorming",
          pinned: commit("3"),
          newer: commit("8"),
          from_pin: true,
          added: EMPTY,
          removed: EMPTY,
          files: [
            {
              path: "SKILL.md",
              change: "changed",
              diff: "@@ -4,1 +4,2 @@\n-2. List three options.\n+2. Ask for the constraints before any option.\n+3. Then list at most three options.\n",
            },
          ],
          edited: ["SKILL.md"],
          local_edits: [
            {
              path: "SKILL.md",
              change: "changed",
              diff: "+5. Write the design to docs/designs/.\n",
            },
          ],
          skill: {
            ...DEBUGGING,
            name: "brainstorming",
            summary: EMPTY,
            files: [DEBUGGING.files[0]],
          },
          brought: [],
          grant: null,
          widenings: [],
          accepted: { commit: "8".repeat(40), skills: [] },
          gained: [],
        };
      case "skill_source_installed":
        return COPIES;
      case "skill_set_review":
        return SET_REVIEW(args as { act: string; members: { name: string }[] });
      case "skill_deletes_cut_short":
        return { settled: [], unsettled: [] };
      default:
        return [];
    }
  });
}

/** The studio tab, `width` by 560 px, opened on Sources at `at`. */
function Tab({
  width,
  narrow,
}: {
  width: number;
  narrow?: boolean;
  at?: SourcesAt;
  /** Whether an extension's source is listed too. */
  extension?: boolean;
  /** Whether the project holds an org's `[skill_policy]` (#6031). */
  controls?: boolean;
}) {
  return <StudioInPanel width={width} height={560} narrow={narrow} />;
}

const SOURCE = sourceKey(SUPERPOWERS.remote);

/** An extension that declares `skill-source` (#3092). */
const ACME = { id: "acme-registry", label: "Acme skill registry", installed: 1 };

const meta = {
  component: Tab,
  args: { width: 1000 },
  beforeEach: ({ args }) => {
    core({
      twoUpdates: args.at?.at === "set-update",
      extension: args.extension === true,
      controls: args.controls === true,
    });
    openViewAt(HISTORY, {
      page: "sources",
      scope: AT_THE_ROOT,
      ...(args.at === undefined ? {} : { sources: args.at }),
    });
    return () => {
      clearMocks();
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Sources**: the update waiting and the unconfirmed source first, then your sources, then the
 *  sources to start from as cards. Tab reaches the update's Review. */
export const Sources: Story = {
  play: async ({ canvas }) => {
    await canvas.findByTestId("skill-updates");
    await canvas.findByTestId("skill-source-unconfirmed");
    await waitFor(() => expect(canvas.getAllByTestId("skill-source")).toHaveLength(2));
    // What each source's last check found, on its row (#3840).
    await waitFor(() =>
      expect(canvas.getAllByTestId("skill-source")[0]).toHaveTextContent(
        "14 skills, 1 installed · checked 3 hours ago",
      ),
    );
    await expect(canvas.getAllByTestId("skill-source")[1]).toHaveTextContent("1 installed");
    await expect(canvas.getAllByTestId("skill-source-offered")).toHaveLength(2);
    const review = canvas.getByRole("button", { name: "Review update" });
    for (let step = 0; step < 30 && document.activeElement !== review; step++)
      await userEvent.tab();
    await untilFocusWithin(() => review, "Tab reaches Review update");
    expectFocusInSight();
  },
};

/** **Sources on an org's project** (#6031, `prototype/skill-policy` A): one Notice says the
 *  controls apply, with See the controls; platform-skills is marked Not on the allow list with
 *  the core's sentence; the update waiting is too young, its Review out of reach. Tab reaches See
 *  the controls. */
export const SourcesWithControls: Story = {
  args: { controls: true },
  play: async ({ canvas }) => {
    const notice = await canvas.findByTestId("skill-policy");
    await expect(notice).toHaveTextContent("This project's controls apply");
    await waitFor(() =>
      expect(canvas.getAllByTestId("skill-source")[1]).toHaveTextContent("Not on the allow list"),
    );
    await expect(canvas.getByTestId("skill-updates")).toHaveTextContent(
      "(too young, 3 days to go)",
    );
    const see = canvas.getByRole("button", { name: "See the controls" });
    for (let step = 0; step < 30 && document.activeElement !== see; step++) await userEvent.tab();
    await untilFocusWithin(() => see, "Tab reaches See the controls");
    expectFocusInSight();
  },
};

/** **A source's page on an org's project** (#6031): systematic-debugging marked Blocked and its
 *  Review install out of reach, its reason on the row; the update of brainstorming too young,
 *  with the day it can be taken. Not pictured: that day moves with the day the run is on. */
export const SourcePageWithControls: Story = {
  args: { controls: true, at: { at: "source", source: SOURCE, label: SUPERPOWERS.label } },
  parameters: { screenshot: false },
  play: async ({ canvas }) => {
    const debugging = await canvas.findByTestId("offered-systematic-debugging");
    await expect(debugging).toHaveTextContent("Blocked");
    await expect(
      canvas.getByRole("button", { name: "Review install: systematic-debugging" }),
    ).toHaveAttribute("aria-disabled", "true");
    await waitFor(() =>
      expect(canvas.getByTestId("offered-brainstorming")).toHaveTextContent(
        "Too young, 3 days to go",
      ),
    );
    await expect(canvas.getByTestId("refused-brainstorming")).toHaveTextContent(
      /it can be installed in 3 days, on \w+day \d+ \w+\./,
    );
  },
};

/** **Sources with an extension's source** (#3092): listed beside the git ones, its row saying
 *  which extension it is. */
export const WithAnExtensionsSource: Story = {
  args: { extension: true },
  play: async ({ canvas }) => {
    await waitFor(() => expect(canvas.getAllByTestId("skill-source")).toHaveLength(3));
    await expect(canvas.getAllByTestId("skill-source")[2]).toHaveTextContent(
      "The extension acme-registry · 1 installed",
    );
  },
};

/** **An extension's source's page** (#3092): its skills at its revision, each marked with what its
 *  files do (#5760), browsed through the same page, with no Remove source…. */
export const ExtensionSourcePage: Story = {
  args: {
    extension: true,
    at: { at: "source", source: extensionKey(ACME.id), label: ACME.label },
  },
  play: async ({ canvas }) => {
    await canvas.findByTestId("offered-release-notes");
    const page = canvas.getByTestId("skill-source-page");
    await expect(page).toHaveTextContent("at revision 2026.11.3");
    await expect(canvas.getByTestId("offered-release-notes")).toHaveTextContent("runs a script");
    await expect(canvas.queryByRole("button", { name: "Remove source…" })).toBeNull();
    const review = canvas.getByRole("button", { name: "Review install: release-notes" });
    for (let step = 0; step < 30 && document.activeElement !== review; step++)
      await userEvent.tab();
    await untilFocusWithin(() => review, "Tab reaches Review install");
    expectFocusInSight();
  },
};

/** **Narrow**: the same page under the page picker, nothing scrolling sideways. */
export const Narrow: Story = {
  args: { width: 420, narrow: true },
  play: async ({ canvas }) => {
    await canvas.findByTestId("skill-updates");
    const root = canvas.getByTestId("skill-sources");
    await expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1);
  },
};

/** **Add source…**: the repository and who can browse it, with More folded. */
export const AddSource: Story = {
  play: async ({ canvas }) => {
    await userEvent.click(await canvas.findByRole("button", { name: "Add source…" }));
    const dialog = await within(document.body).findByRole("dialog", { name: "Add a source" });
    await expect(
      within(dialog).getByRole("button", { name: "More: branch, folder, sign-in" }),
    ).toHaveAttribute("aria-expanded", "false");
  },
};

/** **A source's page**: its skills marked Installed, Update available, or with what they do. */
export const SourcePage: Story = {
  args: { at: { at: "source", source: SOURCE, label: SUPERPOWERS.label } },
  play: async ({ canvas }) => {
    const debugging = await canvas.findByTestId("offered-systematic-debugging");
    await expect(debugging).toHaveTextContent("runs a script");
    await expect(canvas.getByTestId("offered-brainstorming")).toHaveTextContent("Update available");
    await expect(canvas.getByTestId("skill-source-page")).toHaveTextContent(
      "checked 5 minutes ago",
    );
  },
};

/** **The install review**: the pinned commit, each file under its mark, Lands in, and Install at
 *  the foot, which stays in view while the files scroll. */
export const InstallReview: Story = {
  args: {
    at: { at: "install", source: SOURCE, label: SUPERPOWERS.label, skill: "systematic-debugging" },
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("install-review");
    await within(review).findByTestId("install-commit");
    await userEvent.click(within(review).getByRole("button", { name: "Show scripts/repro.sh" }));
    await expect(within(review).getByRole("button", { name: "Install" })).toBeVisible();
  },
};

/** **A skill that comes along by a hint** (#5874, `prototype/skill-hints` A): ticked, with why it
 *  may be left out under its box. Space unticks it, and it waits under Left out of this install
 *  to be ticked back, which the picture shows. */
export const InstallReviewHint: Story = {
  args: {
    at: { at: "install", source: SOURCE, label: SUPERPOWERS.label, skill: "hints-at-tdd" },
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("install-review");
    const box = await within(review).findByRole("checkbox", {
      name: /^Install test-driven-development/,
    });
    await expect(within(review).getByTestId("reviewed-test-driven-development")).toHaveTextContent(
      "Comes along by a hint: systematic-debugging names it bare.",
    );
    for (let step = 0; step < 40 && document.activeElement !== box; step++) await userEvent.tab();
    await untilFocusWithin(() => box, "Tab reaches the hinted skill's box");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    const left = await within(review).findByRole("checkbox", {
      name: "Install test-driven-development",
    });
    await waitFor(() => expect(left).not.toBeChecked());
    await expect(within(review).queryByTestId("reviewed-test-driven-development")).toBeNull();
    // The picture shows where the hinted skill waits to be ticked back.
    left.scrollIntoView({ block: "center" });
  },
};

/** The install review of a skill whose source carries a grade, the grade scrolled into view. */
function gradedReview(skill: string, said: string): Story {
  return {
    args: { at: { at: "install", source: SOURCE, label: SUPERPOWERS.label, skill } },
    play: async ({ canvas }) => {
      const review = await canvas.findByTestId("install-review");
      const grade = await within(review).findByTestId("grade-systematic-debugging");
      await expect(grade).toHaveTextContent(said);
      grade.scrollIntoView({ block: "center" });
      await expect(within(review).getByRole("button", { name: "Install" })).toBeVisible();
    },
  };
}

/** **A grade the source carries, measured on this version** (#4346): whose it is and that purlis
 *  has not checked it on a line of their own, before any figure. */
export const InstallReviewGraded = gradedReview(
  "graded-this-version",
  "superpowers's grade, as published — not checked by purlis",
);

/** **A grade the source carries, measured on an earlier version** (#4346). */
export const InstallReviewGradedEarlier = gradedReview(
  "graded-earlier",
  "Graded on an earlier version",
);

/** **A grade file left out** (#4346): why, and nothing from it. */
export const InstallReviewGradeLeftOut = gradedReview(
  "graded-malformed",
  "Nothing from it is shown.",
);

/** **The install review at the project level, granted to ops** (#4185): the grant picked under
 *  Lands in, and the line saying the install grants the project skill it relies on to ops too. */
export const InstallReviewGranted: Story = {
  args: InstallReview.args,
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("install-review");
    await within(review).findByTestId("install-commit");
    await userEvent.click(
      within(review).getByRole("button", { name: /Where systematic-debugging will reach/ }),
    );
    await userEvent.click(
      await within(document.body).findByRole("option", { name: "The root and ops" }),
    );
    const widened = await within(review).findByText(/the install grants it to ops too/);
    await expect(widened).toBeVisible();
    await expect(within(review).getByRole("button", { name: "Install" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
    // The picture shows the line, with the list of grants closed.
    await waitFor(() => expect(within(document.body).queryByRole("listbox")).toBeNull());
    widened.scrollIntoView({ block: "center" });
  },
};

/** **A taken name stops the review** (ADR 0100, amended 2026-11-01): skip it, or a new name filled
 *  in to edit, and one line for a copy at another level. */
export const InstallReviewClash: Story = {
  args: {
    at: { at: "install", source: SOURCE, label: SUPERPOWERS.label, skill: "taken-by-another" },
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("install-review");
    const rename = await within(review).findByRole("radio", {
      name: "Install it under a new name",
    });
    await expect(within(review).getByRole("button", { name: "Install" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await userEvent.click(rename);
    await expect(
      within(review).getByRole("textbox", { name: "New name for systematic-debugging" }),
    ).toHaveValue("superpowers-systematic-debugging");
    await expect(within(review).getByRole("button", { name: "Install" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
  },
};

/** **The update review**: what changes, the source's diff, your edits, Accept update at the foot. */
export const UpdateReview: Story = {
  args: { at: { at: "update", source: SOURCE, label: SUPERPOWERS.label, skill: "brainstorming" } },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("update-review");
    await within(review).findByTestId("update-edited");
    await expect(within(review).getByRole("button", { name: "Accept update" })).toBeVisible();
  },
};

/** **Review updates…, from a source's page** (#5354): every update waiting from it on one page,
 *  the one that adds a script and network access first and unticked, the plain edit ticked. */
export const ReviewUpdates: Story = {
  args: { at: { at: "set-update", source: SOURCE, label: SUPERPOWERS.label } },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("update-set-review");
    const plans = await within(review).findByRole("checkbox", {
      name: "Update writing-plans (project)",
    });
    await expect(plans).not.toBeChecked();
    await expect(
      within(review).getByRole("checkbox", { name: "Update brainstorming (project)" }),
    ).toBeChecked();
    await expect(within(review).getByTestId("update-set-lead")).toHaveTextContent(
      "2 updates waiting.",
    );
    await tabInto(() =>
      within(review).getByRole("button", { name: "What changes in writing-plans" }),
    );
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(plans).toBeEnabled());
    await tabInto(() => plans);
    await userEvent.keyboard(" ");
    expectFocusInSight();
    await waitFor(() => expect(plans).toBeChecked());
    await expect(within(review).getByRole("button", { name: "Accept 2 updates" })).toBeVisible();
  },
};

/** **Install all…, the skills to tick** (#4024): every skill not installed yet ticked; the
 *  installed one greyed, its update waiting, with Review update; Review at the foot. */
export const InstallAllChoose: Story = {
  args: { at: { at: "choose", source: SOURCE, label: SUPERPOWERS.label } },
  play: async ({ canvas }) => {
    const choose = await canvas.findByTestId("install-all-choose");
    const debugging = await within(choose).findByTestId("choose-systematic-debugging");
    await expect(within(debugging).getByRole("checkbox")).toBeChecked();
    const brainstorming = within(choose).getByTestId("choose-brainstorming");
    await expect(brainstorming).toHaveTextContent("Update waiting");
    await expect(within(brainstorming).getByRole("checkbox")).toBeDisabled();
    await expect(within(choose).getByRole("button", { name: "Review 2 skills" })).toBeVisible();
  },
};

/** **Install all…, the one review**: where they land at the top, each skill with its box and its
 *  risky items, one skill's files unfolded, and Install at the foot. */
export const InstallAllReview: Story = {
  args: {
    at: {
      at: "install-all",
      source: SOURCE,
      label: SUPERPOWERS.label,
      skills: ["test-driven-development", "systematic-debugging"],
    },
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("install-all-review");
    await within(review).findByTestId("install-commit");
    await expect(within(review).getByTestId("keep-systematic-debugging")).toHaveTextContent(
      "2 risky items",
    );
    await userEvent.click(
      within(within(review).getByTestId("reviewed-systematic-debugging")).getByRole("button", {
        name: "Its files (2)",
      }),
    );
    await expect(within(review).getByRole("button", { name: "Install" })).toBeVisible();
  },
};

/** **Install all…, one ticked already there** (#4153): the level holds writing-plans, made
 *  there by hand, so it is shown as there, its box unticked and out of reach, and left as it is. */
export const InstallAllThere: Story = {
  args: {
    at: {
      at: "install-all",
      source: SOURCE,
      label: SUPERPOWERS.label,
      skills: ["test-driven-development", "writing-plans"],
    },
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("install-all-review");
    const there = await within(review).findByTestId("there-writing-plans");
    await expect(within(there).getByRole("checkbox")).not.toBeChecked();
    await expect(within(review).getByRole("button", { name: "Install" })).toBeVisible();
    // The picture shows the section, below the skill being installed.
    there.scrollIntoView({ block: "end" });
  },
};

/** **Delete all…, the skills to tick** (#5331): each under its level, only the current level's
 *  ticked, and one changed since install left unticked and marked. */
export const DeleteAllChoose: Story = {
  args: { at: { at: "set-choose", act: "delete", source: SOURCE, label: SUPERPOWERS.label } },
  play: async ({ canvas }) => {
    const choose = await canvas.findByTestId("set-choose");
    const row = await within(choose).findByTestId("set-project-systematic-debugging");
    await expect(within(row).getByRole("checkbox")).not.toBeChecked();
    await expect(row).toHaveTextContent("Changed since install");
    const list = within(choose).getByRole("grid");
    await tabInto(() => list);
    await userEvent.keyboard(" ");
    expectFocusInSight();
    await expect(within(choose).getByRole("button", { name: /^Review/ })).toBeVisible();
  },
};

/** **The one review** (#5331): the warning of a skill left naming one that goes, with its offer; how
 *  many open chats keep them; one taken out with its reason; one left out; Delete at the foot. */
export const DeleteAllReview: Story = {
  args: {
    at: {
      at: "set-review",
      act: "delete",
      source: SOURCE,
      label: SUPERPOWERS.label,
      members: COPIES.map((one) => one.member),
      ticked: COPIES.filter((one) => !one.changed).map((one) => memberKey(one.member)),
    },
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("set-review");
    const warning = await within(review).findByTestId("set-names-to-spec-brainstorming");
    await expect(warning).toHaveTextContent("to-spec names brainstorming, which you're deleting.");
    await expect(within(review).getByTestId("set-open-chats")).toHaveTextContent(
      "2 open chats use these.",
    );
    await expect(within(review).getByTestId("set-cannot-writing-plans")).toBeVisible();
    await expect(within(review).getByTestId("set-left-out-systematic-debugging")).toBeVisible();
    const keep = within(warning).getByRole("button", { name: "Keep brainstorming" });
    await tabInto(() => keep);
    expectFocusInSight();
    await expect(within(review).getByRole("button", { name: "Delete" })).toBeVisible();
  },
};

/** **Turn all off…, the one review** (#5331): the same page, in a workspace's words. */
export const TurnAllOffReview: Story = {
  args: {
    at: {
      at: "set-review",
      act: "turn-off",
      source: SOURCE,
      label: SUPERPOWERS.label,
      members: COPIES.slice(0, 2).map((one) => one.member),
      ticked: COPIES.slice(0, 2).map((one) => memberKey(one.member)),
    },
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("set-review");
    const going = await within(review).findByTestId("set-going-list");
    await expect(within(review).getByRole("button", { name: "Turn off" })).toBeVisible();
    // A skill's box is reached and unticked by the keyboard, and goes to Left out.
    await tabInto(() => going);
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await within(review).findByTestId("set-left-out-brainstorming");
  },
};

/** **Turn all on…, the one review** (#5331): one that stays off is named, with its offer to come
 *  too. */
export const TurnAllOnReview: Story = {
  args: {
    at: {
      at: "set-review",
      act: "turn-on",
      source: SOURCE,
      label: SUPERPOWERS.label,
      members: COPIES.slice(0, 1).map((one) => one.member),
      ticked: COPIES.slice(0, 1).map((one) => memberKey(one.member)),
    },
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("set-review");
    const warning = await within(review).findByTestId("set-names-brainstorming-writing-plans");
    await expect(warning).toHaveTextContent("which stays off.");
    const also = within(warning).getByRole("button", { name: "Also turn on writing-plans" });
    await tabInto(() => also);
    expectFocusInSight();
    await expect(within(review).getByRole("button", { name: "Turn on" })).toBeVisible();
  },
};

/** **Remove source…, with its one choice** (#5331): it keeps the skills until the choice is ticked,
 *  which says what comes next and leads to the same review. Esc closes it. */
export const RemoveSourceAlsoDeleting: Story = {
  args: { at: { at: "source", source: SOURCE, label: SUPERPOWERS.label } },
  play: async ({ canvas }) => {
    const page = await canvas.findByTestId("skill-source-page");
    const remove = await within(page).findByRole("button", { name: "Remove source…" });
    await tabInto(() => remove);
    await userEvent.keyboard("{Enter}");
    const dialog = await within(document.body).findByRole("dialog");
    const choice = await within(dialog).findByRole("checkbox", {
      name: /^Also delete the 4 skills installed from it/,
    });
    await tabInto(() => choice);
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await expect(choice).toBeChecked();
    await expect(dialog).toHaveTextContent("You review the skills next.");
    // The dialog may still be drawing itself in, so this is waited for rather than asked once.
    await waitFor(() =>
      expect(within(dialog).getByRole("button", { name: "Review" })).toBeVisible(),
    );
  },
};

/** **Install all…, past one read**: it says so and offers the first part, Install held. */
export const InstallAllTooMuch: Story = {
  args: InstallAllReview.args,
  beforeEach: () => {
    core({ tooMuch: true });
  },
  play: async ({ canvas }) => {
    const said = await canvas.findByTestId("install-all-too-much");
    await expect(
      within(said).getByRole("button", { name: "Review the first skill" }),
    ).toBeVisible();
  },
};

/** **Install all…, past the skill budget** (#5355): the program it takes past the most skills a
 *  chat lists, and Make room…, reached by the keyboard. */
export const InstallAllOverBudget: Story = {
  args: InstallAllReview.args,
  beforeEach: () => {
    core({ overBudget: true });
  },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("install-all-review");
    const over = await within(review).findByTestId("install-all-over");
    await expect(over).toHaveTextContent(
      "Installing these puts 61 skills in a chat's list on Claude Code, past its budget of 60.",
    );
    await tabInto(() => within(over).getByRole("button", { name: "Make room…" }));
    expectFocusInSight();
  },
};
