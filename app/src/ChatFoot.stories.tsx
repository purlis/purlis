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
import { expect, userEvent, waitFor, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { ChatUsage, HarnessGlance } from "./bindings";
import { ChatFoot, PaneActions, PaneActionsOf, PaneBellOf, PaneHead } from "./LayoutPanes";
import { catalogued, type Offer } from "./actions";
import type { ChatHead } from "./paneHead";
import {
  FOOT_ROWS,
  paneLinesOf,
  type FootPart,
  type FootPress,
  type LineFacts,
  type PaneKind,
} from "./paneHead";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
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
  /** Draws the head over it too: the two lines drop together (#5206). */
  withHead?: boolean;
};

const SPLITS: Offer[] = [
  {
    id: "pane.split.right",
    title: "Split right",
    available: true,
    reason: "",
    does: { verb: "split", direction: "row" },
  },
  {
    id: "pane.split.down",
    title: "Split down",
    available: true,
    reason: "",
    does: { verb: "split", direction: "column" },
  },
];
const CLOSE: Offer = {
  id: "pane.close",
  title: "Close this pane",
  available: true,
  reason: "",
  does: { verb: "closePane" },
};
/** The chat's list, which its ⋯ opens. */
const LIST = catalogued([
  {
    id: `chat.rename:${SESSION}`,
    title: "Rename…",
    available: true,
    reason: "",
    does: { verb: "renameChat", session: SESSION },
  },
]);

/** The head the pane's model answers at its width: the chat's name and mark, then its actions. */
function Head({ kind, width }: { kind: PaneKind; width: number }) {
  const lines = paneLinesOf(kind, width, {});
  return (
    <PaneActionsOf.Provider value={lines.actions}>
      <PaneHead
        grip={false}
        doing={
          <PaneBellOf.Provider value={{ notes: 2, trouble: 0, asks: 0 }}>
            <PaneActions
              pane={1}
              state={lines.actions.includes("stateWord") ? <span>Stop</span> : undefined}
              chat={{ session: SESSION, offers: LIST }}
              offerFor={(id) => SPLITS.find((offer) => offer.id === id)}
              closeOf={() => CLOSE}
              onPaneDoes={() => undefined}
            />
          </PaneBellOf.Provider>
        }
      >
        <div className="pane-chips">
          <span className="pane-name">M69 chats, tabs and panes: build</span>
          {lines.head.includes("mark") && (
            <span className="pane-state-mark">{lines.head.includes("markWord") && "working"}</span>
          )}
        </div>
      </PaneHead>
    </PaneActionsOf.Provider>
  );
}

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
function Foot({
  kind = "chat",
  width = 900,
  facts = {},
  from,
  workItem,
  usage = USAGE,
  withHead = false,
}: FootArgs) {
  const [said, setSaid] = useState("Nothing pressed");
  const parts = paneLinesOf(kind, width, facts).foot ?? [];
  const rowOf = (part: keyof typeof FOOT_ROWS): FootPress => ({
    // A row's title says what the part's words say, as the catalogue's do.
    title: part === "workspace" ? "Focus workspace smart-ide" : FOOT_ROWS[part](SESSION),
    press: () => setSaid(FOOT_ROWS[part](SESSION)),
  });
  return (
    <Lent>
      <div className="pane-frame pane-headed" style={{ inlineSize: width, blockSize: 96 }}>
        {withHead && <Head kind={kind} width={width} />}
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

/** Whether a part of the foot is the skills warning, which is drawn once the studio has said. */
const warns = (part: FootPart) => part === "skills" || part === "skillsMark";

const meta = {
  component: Foot,
  play: async (context) => walked(context),
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "skill_studio_on" ? true : cmd === "skill_drafts" ? [] : null));
  },
} satisfies Meta<typeof Foot>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **The foot's keyboard path**: Tab takes each part that can be pressed in the order it is drawn,
 * with its ring in sight, and out again, so the picture holds no focus and no tooltip. What the
 * foot waits on is drawn first, so the picture never races it.
 */
async function walked({ canvasElement, args }: { canvasElement: HTMLElement; args: FootArgs }) {
  const canvas = within(canvasElement);
  const foot = canvas.getByRole("group", { name: /where it runs and its context$/ });
  if (paneLinesOf(args.kind ?? "chat", args.width ?? 900, args.facts ?? {}).foot?.some(warns))
    await canvas.findByTestId("pane-skills");
  for (const part of within(foot).getAllByRole("button")) {
    await userEvent.tab();
    await untilFocused(() => part, "the foot's next part");
    await expectFocusInSight();
  }
  await userEvent.tab();
  await waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull());
}

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
  play: async (context) => {
    const canvas = within(context.canvasElement);
    await walked(context);
    // The press on `ctx`, and on every other part, is its row.
    await userEvent.click(canvas.getByRole("button", { name: /^ctx 62%/ }));
    await expect(canvas.getByTestId("pressed")).toHaveTextContent("chat.activity:7");
    await userEvent.tab();
    await waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull());
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

/** The longest case at a width, the head's own model saying which of its parts stay. */
const AT_A_WIDTH = {
  withHead: true,
  facts: { from: true, skills: true, sandbox: true },
  from: "↳ from steward 3 · ops",
};

/** **The foot at the four widths the prototype draws** (#5206): whole at 900; no `cache` and the
 *  warning its ⚠ at 420; no `↳ from` at 300 too, the harness, ⚠ and `ctx` staying. */
export const At900: Story = { args: { ...AT_A_WIDTH, width: 900 } };
export const At520: Story = { args: { ...AT_A_WIDTH, width: 520 } };
export const At420: Story = {
  args: { ...AT_A_WIDTH, width: 420 },
  play: async (context) => {
    const canvas = within(context.canvasElement);
    const warning = await canvas.findByTestId("pane-skills");
    await expect(warning.textContent).toBe("");
    await expect(warning).toHaveAccessibleName(/^39 skills\./);
    await expect(canvas.queryByText(/cache/)).toBeNull();
    await walked(context);
  },
};
export const At300: Story = {
  args: { ...AT_A_WIDTH, width: 300 },
  play: async (context) => {
    const canvas = within(context.canvasElement);
    await canvas.findByTestId("pane-skills");
    await expect(canvas.queryByText(/↳ from/)).toBeNull();
    await expect(canvas.getByText(/ctx/)).toBeInTheDocument();
    await walked(context);
  },
};
