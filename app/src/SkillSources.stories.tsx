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
import { SkillStudioTab } from "./SkillStudioTab";
import { Panel } from "./ui/panel/Panel";
import {
  AT_THE_ROOT,
  SKILL_STUDIO_VIEW,
  sourceKey,
  studioHistoryOf,
  type SourcesAt,
} from "./skillStudio";
import { PROJECT } from "./skillChanges";
import { studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";

const PLANE = "/home/dev/plane";
const HISTORY = studioHistoryOf(PLANE, SKILL_STUDIO_VIEW);

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

/** The faked core: two sources, one update waiting, one source not confirmed here. */
function core() {
  mockIPC((cmd) => {
    switch (cmd) {
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
        return {
          commit: commit("3"),
          skills: [DEBUGGING],
          accepted: { commit: "3".repeat(40), skills: [] },
        };
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
          reserved: [],
          refused: null,
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
  return (
    // In the content panel, as a view tab is drawn (`PlaneView`): its fill is the ground the
    // page header is drawn on while the page scrolls under it (#3944).
    <Panel variant="content">
      <div className="view-body" style={{ inlineSize: width, blockSize: 560, display: "flex" }}>
        <SkillStudioTab plane={PLANE} changed={0} narrow={narrow} />
      </div>
    </Panel>
  );
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

/** **The update review**: what changes, the source's diff, your edits, Accept update at the foot. */
export const UpdateReview: Story = {
  args: { at: { at: "update", source: SOURCE, label: SUPERPOWERS.label, skill: "brainstorming" } },
  play: async ({ canvas }) => {
    const review = await canvas.findByTestId("update-review");
    await within(review).findByTestId("update-edited");
    await expect(within(review).getByRole("button", { name: "Accept update" })).toBeVisible();
  },
};
