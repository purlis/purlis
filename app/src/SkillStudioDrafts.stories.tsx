// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio's Drafts page and a draft's page** (S9, #3690,
 * `prototype/studio-secondary-pages` variant A): the drafts as a list with the draft chosen
 * beside it, a review page of what it writes with Accept fixed at its foot; an out-of-date draft
 * in the attention tone with its redo; narrow, page-and-back; and the page with nothing on it.
 * Walked by the keyboard, held to axe, and pictured in every pictured theme cell. The core is
 * faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillDraft } from "./bindings";
import { SkillStudioTab } from "./SkillStudioTab";
import { AT_THE_ROOT, SKILL_STUDIO_VIEW, studioHistoryOf } from "./skillStudio";
import { studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";

const PLANE = "/home/dev/plane";
const HISTORY = studioHistoryOf(PLANE, SKILL_STUDIO_VIEW);

const BEFORE =
  "---\nname: review-pr\ndescription: Reviews a pull request.\n---\n\n1. Read REVIEW.md.\n2. Check correctness.\n";
const AFTER =
  "---\nname: review-pr\ndescription: Reviews a pull request against REVIEW.md.\n---\n\n1. Read REVIEW.md.\n2. Check correctness first, then the standards.\n3. Say what to fix, in order.\n";

/** The prototype's two drafts: a new skill of a workspace's, and an out-of-date change. */
const DRAFTS: SkillDraft[] = [
  {
    id: "d1",
    name: "release-checklist",
    level: { at: "workspace", name: "smart-ide", local: false },
    chat: 7,
    chat_open: true,
    new: true,
    stale: false,
    files: [
      {
        path: "SKILL.md",
        before: null,
        after:
          "---\nname: release-checklist\ndescription: Walks a release.\n---\n\n1. Check the milestone is closed.\n2. Write the release notes.\n",
        hunks: [],
        stale: false,
      },
      { path: "scripts/tag.sh", before: null, after: 'git tag "$1"\n', hunks: [], stale: false },
    ],
  },
  {
    id: "d2",
    name: "review-pr",
    level: { at: "project", local: false },
    chat: 4,
    chat_open: true,
    new: false,
    stale: true,
    files: [{ path: "SKILL.md", before: BEFORE, after: AFTER, hunks: [], stale: true }],
  },
];

/** The studio tab as a view tab draws it, `width` by 560 px, on Drafts at `item`. */
function Tab({
  width,
  narrow,
}: {
  width: number;
  narrow?: boolean;
  drafts: SkillDraft[];
  /** The draft chosen, by its id. */
  item?: string;
}) {
  return (
    <div className="view-body" style={{ inlineSize: width, blockSize: 560, display: "flex" }}>
      <SkillStudioTab plane={PLANE} changed={0} narrow={narrow} />
    </div>
  );
}

const meta = {
  component: Tab,
  args: { width: 1000, drafts: DRAFTS, item: "d1" },
  beforeEach: ({ args }) => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio")
        return studio([], {
          workspaces: ["smart-ide"],
          levels: [
            { level: { at: "project", local: false }, label: "project" },
            {
              level: { at: "workspace", name: "smart-ide", local: false },
              label: "workspace smart-ide",
            },
          ],
        });
      if (cmd === "skill_drafts") return args.drafts;
      if (cmd === "start_options") return { profiles: [{ name: "claude" }, { name: "codex" }] };
      if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
      if (cmd === "skill_deletes_cut_short") return { settled: [], unsettled: [] };
      return [];
    });
    openViewAt(HISTORY, {
      page: "drafts",
      scope: AT_THE_ROOT,
      ...(args.item === undefined ? {} : { item: args.item }),
    });
    return () => {
      clearMocks();
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab, until the keyboard is inside `target`, at most `steps` times. */
async function tabInto(target: () => Element | null, steps = 30): Promise<void> {
  for (let step = 0; step < steps; step++) {
    const into = target();
    if (into !== null && into.contains(document.activeElement)) return;
    await userEvent.tab();
  }
  await untilFocusWithin(target, "the keyboard reaches it by Tab");
}

/**
 * **Wide: the list, and the draft chosen beside it.** Its files fold, the first open; Try it on
 * picks a program; Accept and Discard stand in the bar at the page's foot, which Tab reaches with
 * them in sight.
 */
export const Wide: Story = {
  play: async ({ canvas }) => {
    const page = await canvas.findByTestId("skill-studio-draft");
    await within(page).findByRole("button", { name: "scripts/tag.sh" });
    const accept = within(page).getByRole("button", { name: "Accept release-checklist" });
    await tabInto(() => accept);
    expectFocusInSight();
  },
};

/** **Out of date**: said in the attention tone, with the redo in place of Accept. */
export const OutOfDate: Story = {
  args: { item: "d2" },
  play: async ({ canvas }) => {
    const page = await canvas.findByTestId("skill-studio-draft");
    await expect(within(page).queryByRole("button", { name: /^Accept/ })).toBeNull();
    const redo = within(page).getByRole("button", { name: "Ask the chat to redo it" });
    await tabInto(() => redo);
    expectFocusInSight();
  },
};

/** **Narrow: page-and-back.** A draft opened from the list is drawn alone with a way back. */
export const Narrow: Story = {
  args: { width: 460, narrow: true, item: undefined },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByTestId("draft-release-checklist");
    const root = canvasElement.querySelector<HTMLElement>('[data-part="root"][data-narrow]');
    if (root === null) throw new Error("the frame is not narrow");
    await expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1);
    await tabInto(() => canvas.getByRole("grid", { name: "Draft skills" }));
    await userEvent.keyboard("{Enter}");
    await waitFor(() => canvas.getByRole("button", { name: "Back to Drafts" }));
    expectFocusInSight();
  },
};

/** **No drafts**: what appears here, and the one way to get one. */
export const Empty: Story = {
  args: { drafts: [], item: undefined },
  play: async ({ canvas }) => {
    const none = await canvas.findByTestId("skill-drafts-none");
    const ask = within(none).getByRole("button", { name: "Ask a chat to draft a skill" });
    await tabInto(() => ask);
    expectFocusInSight();
  },
};
