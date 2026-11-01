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
import { studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";

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
};

const OFFER = {
  name: "brainstorming",
  level: PROJECT,
  label: "project",
  remote: SUPERPOWERS.remote,
  pinned: commit("3"),
  newer: commit("8"),
  edited: true,
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
function core({ tooMuch = false }: { tooMuch?: boolean } = {}) {
  mockIPC((cmd, args) => {
    switch (cmd) {
      case "skill_sources_checked":
        return [
          { remote: SUPERPOWERS.remote, at: secondsAgo(3 * 3600 + 60), skills: 14, installed: 1 },
          { remote: PLATFORM.remote, at: null, skills: null, installed: 1 },
        ];
      case "skill_studio":
        return studio([], {});
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
      case "skill_updates":
        return {
          offers: [OFFER],
          unasked: [],
          unconfirmed: [{ remote: PLATFORM.remote, skills: ["deploy-checks"] }],
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
            },
          ],
          not_skills: [
            { folder: "docs", why: "it holds no SKILL.md" },
            { folder: "scripts", why: "it holds no SKILL.md" },
          ],
        };
      case "skill_install_review":
        if ((args?.names as string[] | undefined)?.[0] === "taken-by-another")
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
                notes: ["The ops skills have a systematic-debugging too: where a chat gets both, the narrowest copy wins."],
              },
            ],
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
        };
      case "skill_deletes_cut_short":
        return { settled: [], unsettled: [] };
      default:
        return [];
    }
  });
}

/** The studio tab, `width` by 560 px, opened on Sources at `at`. */
function Tab({ width, narrow }: { width: number; narrow?: boolean; at?: SourcesAt }) {
  return <StudioInPanel width={width} height={560} narrow={narrow} />;
}

const SOURCE = sourceKey(SUPERPOWERS.remote);

const meta = {
  component: Tab,
  args: { width: 1000 },
  beforeEach: ({ args }) => {
    core();
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
    await expect(within(review).getByRole("button", { name: "Install" })).toBeDisabled();
    await userEvent.click(rename);
    await expect(
      within(review).getByRole("textbox", { name: "New name for systematic-debugging" }),
    ).toHaveValue("superpowers-systematic-debugging");
    await expect(within(review).getByRole("button", { name: "Install" })).toBeEnabled();
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
