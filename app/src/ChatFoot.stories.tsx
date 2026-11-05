// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a chat's pane foot says** (#5204, spec #5191, look A), drawn by the window's own
 * `ChatFoot` from what the pane's model answers: harness · workspace, `↳ from`, what the Skill
 * studio started it for, its work item, its skills warning and its sandbox at its start, and the
 * gauge at its end. Each part that can be pressed runs its row of the chat's list; Tab walks them
 * in the order they are drawn.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type ReactNode } from "react";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { ChatUsage, HarnessGlance } from "./bindings";
import { ChatFoot, type FootPress } from "./LayoutPanes";
import type { ChatHead } from "./paneHead";
import { FOOT_ROWS, paneLinesOf, type LineFacts, type PaneKind } from "./paneHead";
import { expectFocusInSight } from "./stories.testkit";
import { ChatSkillSetsHere, type ChatSkillSets } from "./UseSkill";
import { StudioChatsHere } from "./studioChats";

const SESSION = 7;

const GLANCE: HarnessGlance = {
  name: "claude",
  title: "Claude Code",
  label: "What Claude Code can do here",
  lines: [],
  cannot_type: null,
};

const USAGE: ChatUsage = {
  context: { value: 62, tone: "ok" },
  cache: { value: 91, tone: "ok" },
  rebuilds: null,
  turns: [],
  cold: null,
};

/** What the head says of the chat. */
const HEAD = {
  chat: { session: SESSION, name: "M69 chats, tabs and panes: build", workspace: "smart-ide" },
  sandbox: { level: "floor", why: null, sites: null },
  wrapping: false,
} as unknown as ChatHead;

/** A chat's set of skills past its budget, as the window holds it. */
const OVER = {
  budget: {
    skills: 39,
    chars: 5900,
    manual: 1,
    most_skills: 30,
    most_chars: 6000,
    hidden: 0,
    still_over: null,
    pinned_by: [],
    pins_together: null,
    warns: "chars",
  },
  routing: null,
  missing: null,
  changed: null,
} as unknown as ChatSkillSets[number];

type FootArgs = {
  kind?: PaneKind;
  width?: number;
  facts?: LineFacts;
  from?: string;
  workItem?: string;
  usage?: ChatUsage;
};

function Lent({ children }: { children: ReactNode }) {
  return (
    <StudioChatsHere.Provider
      value={{
        chats: [
          {
            session: SESSION,
            name: "7",
            link: {
              skill: "test-design",
              level: { at: "project", local: false },
              job: "draft",
              done: false,
            },
          },
        ],
        show: () => undefined,
        openSkill: () => undefined,
      }}
    >
      <ChatSkillSetsHere.Provider value={{ [SESSION]: OVER }}>
        {children}
      </ChatSkillSetsHere.Provider>
    </StudioChatsHere.Provider>
  );
}

/** A pane's foot at `width`, with the row last pressed said under it. */
function Foot({ kind = "chat", width = 900, facts = {}, from, workItem, usage = USAGE }: FootArgs) {
  const [said, setSaid] = useState("Nothing pressed");
  const parts = paneLinesOf(kind, width, facts).foot ?? [];
  const rowOf = (part: keyof typeof FOOT_ROWS): FootPress => ({
    title: FOOT_ROWS[part](SESSION),
    press: () => setSaid(FOOT_ROWS[part](SESSION)),
  });
  return (
    <Lent>
      <div className="pane-frame pane-headed" style={{ inlineSize: width, blockSize: 96 }}>
        <div className="pane-body" />
        <ChatFoot
          plane="story"
          session={SESSION}
          parts={parts}
          head={HEAD}
          crumbs={undefined}
          harness={GLANCE}
          from={from}
          workItem={workItem}
          usage={usage}
          rowOf={rowOf}
        />
      </div>
      <p data-testid="pressed">{said}</p>
    </Lent>
  );
}

const meta = {
  component: Foot,
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "skill_studio_on" ? true : cmd === "skill_drafts" ? [] : null));
  },
} satisfies Meta<typeof Foot>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A plain chat**: harness · workspace, its sandbox and its gauge. */
export const APlainChat: Story = {
  args: { facts: { sandbox: true } },
};

/** **Every fact**: where it came from, what the studio started it for, its work item, its skills
 *  warning, in the drawn order, and Tab takes each that can be pressed in that order. */
export const EveryFact: Story = {
  args: {
    facts: { from: true, studio: true, workItem: true, skills: true, sandbox: true },
    from: "↳ from steward 3 · ops",
    workItem: "PRJ-1",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("group", { name: /where it runs and its context$/ }),
    ).toBeTruthy();
    for (const name of [
      "What Claude Code can do here",
      /chat\.workspace/,
      /↳ from steward 3/,
      /test-design/,
      /skills\. /,
      /^ctx 62%/,
    ]) {
      await userEvent.tab();
      await expect(canvas.getByRole("button", { name })).toHaveFocus();
      await expectFocusInSight();
    }
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByTestId("pressed")).toHaveTextContent("chat.activity:7");
  },
};

/** **A chat that is low on room and cold**: the bar and the low number in their tones. */
export const NearlyFullAndCold: Story = {
  args: {
    facts: { sandbox: true },
    usage: {
      ...USAGE,
      context: { value: 87, tone: "warn" },
      cache: { value: 12, tone: "bad" },
    },
  },
};

/** **A task's foot**: the task's harness and gauge, and `↳ from` the chat that asked (#5205). */
export const ATasksFoot: Story = {
  args: { kind: "task", facts: { from: true, sandbox: true }, from: "↳ from steward 2" },
};

/** **An ended chat's foot**: its gauge frozen at its last values, greyed and marked `ended`, with
 *  a value that was warning before it ended (#5205). */
export const AnEndedChat: Story = {
  args: {
    kind: "ended",
    facts: { sandbox: true },
    usage: { ...USAGE, context: { value: 87, tone: "warn" }, cache: { value: 40, tone: "warn" } },
  },
};
