// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A narrow pane's head, one row per state** (#5446): the chat's mark first, its persona with its
 * state's glyph on the corner, then its name. A pane under 470px drops the state's word
 * (`paneHead.ts`), and the line then draws no glyph of its own, since the mark already wears it:
 * each state's glyph is drawn once at every width. Its word is still heard, from the mark.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { ChatBrowser } from "./bindings";
import { PaneBrowsers } from "./ChatBrowsersShown";
import { SkillRunChip } from "./SkillRunChip";
import { SkillRunsHere, fixedRuns } from "./skillRuns";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { ChatPersonaState } from "./ChatPersonaState";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { PaneHead, PaneNumber, PaneTasksWord, type TasksOn } from "./LayoutPanes";
import type { ChatHead } from "./paneHead";
import { HEAD_DROPS, paneLinesOf } from "./paneHead";
import { QuestionsHere } from "./QuestionsHere";
import type { QuestionsHeld } from "./taskQuestions";
import type { ChatRow } from "./chatsTree";
import { stateClock } from "./stateClock";
import { ChatTasks } from "./TabChip";

/** The width of the pane each row is drawn as: under 470, where the head drops the word. */
const NARROW = 400;

/** One chat, as the core lists it. */
function listed(session: number, name: string, over: Partial<ChatHead["chat"]> = {}) {
  return {
    session,
    name,
    persona: "steward",
    workspace: "smart-ide",
    shell: false,
    parent: null,
    mode: null,
    from: null,
    tab: true,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: "Claude Code",
    ...over,
  } satisfies ChatHead["chat"];
}

/** A chat in each state the mark tells. */
const CHATS = [
  listed(1, "needs you"),
  listed(2, "working"),
  listed(3, "waiting", { report: "owed" }),
  listed(4, "idle"),
  listed(5, "ended"),
];

/** A chat with two tasks, whose head with no word draws its tasks' button as the tasks' glyph. */
const WITH_TASKS = listed(11, "with tasks");

/** Its tasks, as the chat's tasks menu lists them: the chat first, then who it asked. */
const TASK_ROWS: ChatRow[] = [
  { ...WITH_TASKS, level: 1, posinset: 1, setsize: 1, orphaned: false },
  ...[14, 15].map((session, at) => ({
    ...listed(session, `task ${at + 1}`, { parent: 11, mode: "task" as const, report: "owed" }),
    persona: "devops",
    tab: false,
    level: 2,
    posinset: at + 1,
    setsize: 2,
    orphaned: false,
  })),
];

/** The head's word opens the chat's tasks menu, as the window's does (`PlaneView.tasksOn`). */
const withItsTasks: TasksOn = (session, _at, word, alone) =>
  session !== 11 ? (
    alone ? null : (
      word
    )
  ) : (
    <ChatTasks
      id="tasks-of-with-tasks"
      name="with tasks"
      rows={TASK_ROWS}
      ended={[]}
      current={11}
      asked={0}
      clock={stateClock()}
      dragging={() => false}
      onShow={() => undefined}
      alone={alone}
    >
      {word}
    </ChatTasks>
  );

/** What the board says of each. */
const STATES: ChatStates = {
  ...nothingKnown,
  bySession: {
    1: "waiting",
    2: "running",
    3: "running",
    4: "waiting",
    5: "done",
    11: "waiting",
    14: "running",
    15: "waiting",
  },
  needsYou: [1],
};

/** Chat 3 asks its asking chat a question, which is what waiting is told by here (#4564). */
const QUESTIONS: QuestionsHeld = {
  asks: [],
  with_asking_chat: [{ session: 3, asker: "steward 1", since: Math.floor(Date.now() / 1000) - 60 }],
};

/** The parts a head draws at {@link NARROW}, as a pane that wide draws them. */
const LINE = new Set(paneLinesOf("chat", NARROW, {}).head);

/** One chat's head in a pane {@link NARROW} pixels wide: its mark, its name and its state. */
function Head({ chat, tasksOn }: { chat: ChatHead["chat"]; tasksOn?: TasksOn }) {
  return (
    <div className="pane-frame pane-headed" style={{ inlineSize: NARROW, blockSize: 56 }}>
      <div className="pane-top">
        <PaneHead grip={false} doing={null}>
          <div className="pane-chips" data-chat={chat.name}>
            <span className="pane-persona">
              <ChatPersonaState
                persona={chat.persona}
                session={chat.session}
                shell={chat.shell}
                report={chat.report}
                outcome={chat.outcome}
                harness={chat.harness}
              />
            </span>
            <span className="pane-name ui-text-heading">{chat.name}</span>
            {LINE.has("number") && <PaneNumber session={chat.session} />}
            <PaneTasksWord
              tasksOn={tasksOn}
              session={chat.session}
              at={undefined}
              word={LINE.has("mark")}
              markWord={LINE.has("markWord")}
              chat={chat}
            />
          </div>
        </PaneHead>
      </div>
    </div>
  );
}

/** A narrow head for each state. */
function NarrowHeads() {
  return (
    <ChatsHere.Provider value={fixedChats(STATES)}>
      <QuestionsHere.Provider value={QUESTIONS}>
        <div style={{ display: "grid", gap: "var(--space-3)" }}>
          {CHATS.map((chat) => (
            <Head key={chat.session} chat={chat} />
          ))}
          <Head chat={WITH_TASKS} tasksOn={withItsTasks} />
        </div>
      </QuestionsHere.Provider>
    </ChatsHere.Provider>
  );
}

const meta = { component: NarrowHeads } satisfies Meta<typeof NarrowHeads>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Each row wears its state's glyph once, on its mark, and no second one after its name. */
export const EachStateOnce: Story = {
  play: async ({ canvasElement }) => {
    for (const [name, state] of [
      ["needs you", "needs"],
      ["working", "working"],
      ["waiting", "waiting"],
      ["idle", "idle"],
      ["ended", "ended"],
    ] as const) {
      const chips = canvasElement.querySelector<HTMLElement>(`[data-chat="${name}"]`);
      const mark = chips?.querySelector('[data-scope="chat-mark"][data-part="root"]') ?? null;
      await expect(mark, name).toHaveAttribute("data-variant", state);
      const glyphs = [...(chips?.querySelectorAll("svg") ?? [])].map((one) =>
        one.getAttribute("class"),
      );
      await expect(glyphs, name).toEqual([...new Set(glyphs)]);
      // Only the mark's badge draws the state: the line after the name draws no glyph.
      await expect(chips?.querySelector(".pane-state-mark svg") ?? null, name).toBeNull();
    }
  },
};

/**
 * **A head with no word, whose chat has tasks** (#5524): its tasks' button is drawn as the tasks'
 * glyph, never the state's, which the mark beside it wears; it is named by the state's word, and
 * opens the chat's tasks menu from the keyboard.
 */
export const TasksButtonWithNoWord: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const chips = canvasElement.querySelector<HTMLElement>('[data-chat="with tasks"]');
    const glyphs = [...(chips?.querySelectorAll("svg") ?? [])].map((one) =>
      one.getAttribute("class"),
    );
    await expect(glyphs).toEqual([...new Set(glyphs)]);
    await expect(chips?.querySelector(".pane-state-mark .lucide-list-tree") ?? null).not.toBeNull();
    // The story's only button: named by the chat's state word, which the mark beside it draws.
    const button = canvas.getByRole("button");
    await expect(button).toHaveAttribute("aria-haspopup", "menu");
    await userEvent.tab();
    await untilFocused(() => button, "Tab onto the head's tasks button");
    expectFocusInSight();
  },
};

/** A chat that uses something now: a skill's run open, and two browsers (#5524). */
const USING = listed(21, "using things");

/** The run open in {@link USING}, as the core tells it. */
const RUN = {
  skill: "deploy",
  name: "deploy",
  id: "01RUN",
  tool_calls: 2,
  tokens: null,
  level: null,
  copy_workspace: null,
};

/** One browser of {@link USING}'s, as the core lists it. */
function aBrowser(name: string): ChatBrowser {
  return {
    session: USING.session,
    name,
    program: `browser-${name}`,
    unended: false,
    pinned: false,
    was: null,
    login: null,
    driver: null,
    pool: null,
    secrets: [],
  };
}

const BROWSERS = [aBrowser("b1"), aBrowser("b2")].map((browser) => ({
  browser,
  chat: USING.name,
}));

/** **The widths the head is drawn at**: either side of where the state's word and what the chat
 *  uses go, then where the button's word and the mark do (`HEAD_DROPS`). */
const WIDTHS = [470, 450, 449, 379] as const;

/** {@link USING}'s head in a pane `width` pixels wide, drawing what that width leaves it. */
function UsingHead({ width }: { width: number }) {
  const line = new Set(paneLinesOf("chat", width, { using: true }).head);
  return (
    <div className="pane-frame pane-headed" style={{ inlineSize: width, blockSize: 56 }}>
      <div className="pane-top">
        <PaneHead grip={false} doing={null}>
          <div className="pane-chips" data-width={width}>
            <span className="pane-persona">
              <ChatPersonaState
                persona={USING.persona}
                session={USING.session}
                shell={USING.shell}
                report={USING.report}
                outcome={USING.outcome}
                harness={USING.harness}
              />
            </span>
            <span className="pane-name ui-text-heading">{USING.name}</span>
            {line.has("number") && <PaneNumber session={USING.session} />}
            <PaneTasksWord
              session={USING.session}
              at={undefined}
              word={line.has("mark")}
              markWord={line.has("markWord")}
              chat={USING}
            />
            {line.has("using") && (
              <span className="pane-using">
                <SkillRunChip session={USING.session} onOpen={() => undefined} />
                <PaneBrowsers browsers={BROWSERS} onOpen={() => undefined} />
              </span>
            )}
          </div>
        </PaneHead>
      </div>
    </div>
  );
}

/** {@link USING}'s head at each of {@link WIDTHS}. */
function UsingHeads() {
  return (
    <ChatsHere.Provider value={fixedChats({ ...STATES, bySession: { 21: "running" } })}>
      <SkillRunsHere.Provider value={fixedRuns({ [USING.session]: RUN })}>
        <div style={{ display: "grid", gap: "var(--space-3)" }}>
          {WIDTHS.map((width) => (
            <UsingHead key={width} width={width} />
          ))}
        </div>
      </SkillRunsHere.Provider>
    </ChatsHere.Provider>
  );
}

/**
 * **What the chat uses, in the head's narrow order** (#5614): its skill run and its browsers go
 * together after the state's word and before the button's word and the mark, so a narrow pane
 * keeps its name and mark. One head per width, either side of where each goes.
 */
export const UsingDropsInOrder: Story = {
  render: () => <UsingHeads />,
  beforeEach: () => {
    // The studio is on, and a browser's view is never attached, so each thumbnail is drawn as
    // its mark, as `BrowsersGridTab`'s are: a picture with no frame to race.
    mockIPC((cmd) =>
      cmd === "skill_studio_on"
        ? true
        : cmd === "watch_browser"
          ? new Promise(() => undefined)
          : null,
    );
  },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const at = (width: number) =>
      canvasElement.querySelector<HTMLElement>(`[data-width="${width}"]`);
    const goes = (part: string) => HEAD_DROPS.find((drop) => drop.part === part)?.below ?? 0;
    // The chip waits on the studio saying it is on: wait for it, so the picture never races it.
    await waitFor(() => expect(at(450)?.querySelector(".pane-skill-run") ?? null).not.toBeNull());
    for (const width of WIDTHS) {
      const chips = at(width);
      const uses = width >= goes("using");
      await expect(chips?.querySelector(".pane-skill-run") ?? null, `${width}`).toSatisfy(
        (chip: Element | null) => (chip !== null) === uses,
      );
      await expect(chips?.querySelectorAll(".pane-browser").length, `${width}`).toBe(uses ? 2 : 0);
      // The name and the chat's mark stay at every width.
      await expect(chips?.querySelector(".pane-name"), `${width}`).toHaveTextContent(USING.name);
      await expect(
        chips?.querySelector('.pane-persona [data-scope="chat-mark"]') ?? null,
        `${width}`,
      ).not.toBeNull();
    }
    // **Its keyboard path**: Tab takes the skill run of each head that draws it, in order, and
    // nothing of a head that dropped it; the browsers are out of the Tab order (`PaneBrowsers`).
    const runs = canvas.getAllByRole("button", { name: /^Using the skill deploy/ });
    await expect(runs).toHaveLength(WIDTHS.filter((width) => width >= goes("using")).length);
    for (const run of runs) {
      await userEvent.tab();
      await untilFocused(() => run, "Tab onto a head's skill run");
      expectFocusInSight();
    }
    (document.activeElement as HTMLElement | null)?.blur();
  },
};
