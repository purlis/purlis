// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A studio chat's link to its skill and job** (#5185; `prototype/studio-chat-link`, variant A
 * with B's Grades card): the line above the skill's tabs and the Grades card while the chat
 * runs, the Runs tab's "Chats the studio started" once it is done, Open chat on its draft, and
 * the chip in the chat's header. Walked by the keyboard, held to axe, and pictured in every
 * pictured theme cell. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ClosedStudioChat, SkillDraft, StudioLink } from "./bindings";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { inWorkspace, skillRowId, type SkillPageTab, type StudioPage } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { StudioChatChip, StudioChatsHere, type StudioChatsLent } from "./studioChats";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { expectFocusInSight, untilFocusWithin } from "./stories.testkit";

const WHERE = "purlis-demo";
const SCOPE = inWorkspace(WHERE);
const LEVEL = { at: "workspace", name: WHERE, local: false } as const;
const SKILL = skill("test-design", {
  level: "workspace",
  label: `workspace ${WHERE}`,
  folder: `workspaces/${WHERE}/skills/test-design`,
  description: "Designs a test plan for a change: what to test, at which level, and the cases.",
});
/** Chat 7, writing test-design's outcome evals, started two minutes before the story draws. */
const LINK: StudioLink = {
  skill: "test-design",
  level: LEVEL,
  job: "outcome-evals",
  done: false,
  started: new Date(Date.now() - 2 * 60_000 - 5_000).toISOString(),
  finished: null,
};

/** Chat 5, closed since: it wrote test-design's checks, finished on 4 November at 17:42. */
const CLOSED: ClosedStudioChat = {
  session: 5,
  name: "Checks of test-design",
  closed: new Date(2026, 10, 4, 18, 0).toISOString(),
  link: {
    ...LINK,
    job: "checks",
    done: true,
    started: new Date(2026, 10, 4, 17, 31).toISOString(),
    finished: new Date(2026, 10, 4, 17, 42).toISOString(),
  },
};

/** The draft chat 7 wrote: test-design's outcome evals. */
const DRAFT: SkillDraft = {
  id: "d7",
  name: "test-design",
  level: LEVEL,
  chat: 7,
  chat_open: true,
  new: false,
  stale: false,
  files: [
    {
      path: "evals/evals.json",
      before: null,
      // Short lines, as a draft's merge view draws them: none scrolls sideways.
      after: '{\n  "skill_name": "test-design",\n  "evals": []\n}\n',
      hunks: [{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 4 }],
      stale: false,
    },
  ],
};

type Args = {
  width: number;
  page: StudioPage;
  tab?: SkillPageTab;
  drafts: SkillDraft[];
  closed?: ClosedStudioChat[];
  show: (session: number) => void;
  stop: (session: number) => void;
  openDraft: (link: StudioLink, id: string) => void;
};

/** The studio's tab, with chat 7 lent as the one the studio started for test-design. */
function Tab({ width, show, stop, openDraft }: Args) {
  const lent: StudioChatsLent = {
    chats: [{ session: 7, name: "Outcome evals of test-design", link: LINK }],
    show,
    openSkill: () => undefined,
    openDraft,
    stop,
  };
  return (
    <StudioChatsHere.Provider value={lent}>
      <StudioInPanel width={width} height={560} />
    </StudioChatsHere.Provider>
  );
}

const meta = {
  component: Tab,
  args: {
    width: 1000,
    page: "library",
    tab: "grades",
    drafts: [],
    show: fn(),
    stop: fn(),
    openDraft: fn(),
  },
  beforeEach: ({ args }) => {
    mockIPC((cmd) => {
      switch (cmd) {
        case "skill_studio":
          return studio([SKILL], { workspace: WHERE, workspaces: [WHERE] });
        case "skill_details":
          return {
            copies: [],
            uses: [],
            used_by: [],
            hinted_by: [],
            uses_cut: false,
            pinned_by: [],
            pins_refused: [],
          };
        case "skill_stats":
          return { by_harness: [], by_persona: [], by_workspace: [] };
        case "skill_grades":
          return { grades: [], said: null, tests: 4, evals: 0, tests_refused: null, running: null };
        case "skill_drafts":
          return args.drafts;
        case "studio_chats_closed":
          return args.closed ?? [];
        case "skill_draft_grade":
          return {
            only_evals: false,
            tests: 4,
            evals: 0,
            tests_refused: null,
            latest: null,
            scores: [],
            asked: 0,
            changed: false,
            running: null,
          };
        case "skill_grade_after_edit":
        case "skill_grade_running":
          return null;
        case "start_options":
          return { profiles: [] };
        case "skill_updates":
          return { offers: [], unconfirmed: [], unasked: [] };
        case "skill_deletes_cut_short":
          return { settled: [], unsettled: [] };
        default:
          return [];
      }
    });
    openViewAt(
      HISTORY,
      args.page === "drafts"
        ? { page: "drafts", scope: SCOPE, item: DRAFT.id }
        : {
            page: "library",
            scope: SCOPE,
            skill: skillRowId(SKILL),
            ...(args.tab === undefined ? {} : { tab: args.tab }),
          },
    );
    return () => {
      clearMocks();
      forgetViewHistory(HISTORY);
    };
  },
} satisfies Meta<typeof Tab>;
export default meta;
type Story = StoryObj<typeof meta>;

/** Tab until the keyboard is on `target`, at most 40 times. */
async function tabTo(target: () => HTMLElement) {
  for (let step = 0; step < 40 && document.activeElement !== target(); step++)
    await userEvent.tab();
  await untilFocusWithin(target, "the keyboard reaches Open chat");
}

/**
 * **While the chat runs** (moment 2): the line above the tabs, and the Grades card showing the
 * job in place of its button, how long it has run, Open chat and Stop it (#5248). Each Open chat
 * shows the chat; Stop it asks to stop it.
 */
export const WhileItRuns: Story = {
  play: async ({ canvas, args }) => {
    const line = await canvas.findByTestId("studio-chat-running-7");
    await expect(line).toHaveTextContent(`A chat at ${WHERE} is writing outcome evals · running`);
    const card = await canvas.findByTestId("skill-checks-running-7");
    await expect(card).toHaveTextContent(
      `A chat at ${WHERE} is writing outcome evals · running 2 min`,
    );
    const [first] = canvas.getAllByRole("button", { name: "Open chat" });
    await tabTo(() => first);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.show).toHaveBeenCalledWith(7));
    const stop = () => canvas.getByRole("button", { name: "Stop it" });
    await tabTo(stop);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.stop).toHaveBeenCalledWith(7));
  },
};

/** **Once it is done** (moment 3): its draft waits, and Runs keeps the chat, with Review draft
 *  (#5248) beside Open chat. */
export const OnRunsOnceDone: Story = {
  args: { tab: "runs", drafts: [DRAFT] },
  play: async ({ canvas, args }) => {
    const row = await canvas.findByTestId("studio-chat-7");
    await expect(row).toHaveTextContent("Wrote outcome evals");
    await expect(row).toHaveTextContent("its draft waits in Drafts");
    await expect(canvas.queryByTestId("studio-chat-running-7")).toBeNull();
    const review = () => within(row).getByRole("button", { name: "Review draft" });
    await tabTo(review);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.openDraft).toHaveBeenCalledWith(LINK, DRAFT.id));
    const open = () => canvas.getByRole("button", { name: "Open chat" });
    await tabTo(open);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.show).toHaveBeenCalledWith(7));
  },
};

/** **Once its chat has closed** (#5248): Runs keeps it, after the open one, with when it
 *  finished, and Review draft while its draft waits. It has no tab, so no Open chat. */
export const OnRunsOnceClosed: Story = {
  args: { tab: "runs", drafts: [{ ...DRAFT, chat: 5, chat_open: false }], closed: [CLOSED] },
  play: async ({ canvas, args }) => {
    const row = await canvas.findByTestId("studio-chat-5");
    await expect(row).toHaveTextContent("Wrote checks");
    await expect(row).toHaveTextContent(
      "Checks of test-design, at purlis-demo · finished 11-04 17:42 · its draft waits in Drafts",
    );
    await expect(within(row).queryByRole("button", { name: "Open chat" })).toBeNull();
    const review = () => within(row).getByRole("button", { name: "Review draft" });
    await tabTo(review);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.openDraft).toHaveBeenCalledWith(CLOSED.link, DRAFT.id));
  },
};

/** **Its draft, waiting for review** (moment 4): Open chat beside its name. */
export const ItsDraft: Story = {
  args: { page: "drafts", drafts: [DRAFT] },
  play: async ({ canvas, args }) => {
    const open = await canvas.findByTestId("draft-open-chat");
    await tabTo(() => open);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.show).toHaveBeenCalledWith(7));
  },
};

/** **The chip in the chat's header**, running and done: each opens the skill's page. */
export const TheChatsChip: StoryObj<{ done: boolean; openSkill: (link: StudioLink) => void }> = {
  args: { done: false, openSkill: fn() },
  render: ({ openSkill }) => (
    <StudioChatsHere.Provider
      value={{
        chats: [{ session: 7, name: "7", link: LINK }],
        show: () => undefined,
        openSkill,
        openDraft: () => undefined,
        stop: () => undefined,
      }}
    >
      <div className="pane-foot" style={{ display: "flex", inlineSize: 420 }}>
        <StudioChatChip plane="story" session={7} onOpen={() => openSkill(LINK)} />
      </div>
    </StudioChatsHere.Provider>
  ),
  beforeEach: ({ args }) => {
    mockIPC((cmd) => (cmd === "skill_drafts" ? (args.done ? [DRAFT] : []) : null));
    return () => clearMocks();
  },
  play: async ({ canvas, args }) => {
    const said = args.done ? "wrote" : "writing";
    const chip = await canvas.findByRole("button", {
      name: `test-design · ${said} outcome evals`,
    });
    await tabTo(() => chip);
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.openSkill).toHaveBeenCalledWith(LINK));
    // The picture is of the chip at rest: neither its ring nor its tooltip.
    chip.blur();
    await waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull());
  },
};

export const TheChatsChipOnceDone: StoryObj<{
  done: boolean;
  openSkill: (link: StudioLink) => void;
}> = { ...TheChatsChip, args: { done: true, openSkill: fn() } };
