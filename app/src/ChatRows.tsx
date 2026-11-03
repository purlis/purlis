// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { SandboxMark, type SandboxSaid } from "./SandboxMark";
import { memo, useMemo, type ReactNode } from "react";
import { Columns2, Hand, ListTodo, Maximize2, Plus, SquareTerminal } from "lucide-react";
import { PersonaMark } from "./PersonaMark";
import { ChatMark, StoppingMark, WrappingUp } from "./NeedsYou";
import { markOf, ownWorkOf, useChatsHere, useChatsSelect, type ChatStates } from "./chatState";
import { PlaneUpdatedMark, type PlaneUpdates } from "./PlaneUpdated";
import { contentsOf, panesOf, shownIn, type Tabs } from "./tabs";
import type { TabFace } from "./tabFace";
import { sameShown, useShownState, type ShownOf } from "./ChatPersonaState";
import { ViewMark } from "./Views";
import { shownState, type RowFacts, type Shown } from "./shownState";
import { StateShown } from "./StateShown";
import { needsSaid } from "./tabTasks";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { tabPart } from "./ui/tabs/Tabs";

/*
 * **The rows that draw a chat** (SC-3): split out of `PlaneView`, whose rendering they no longer
 * share. Each reads its own chat's state off the project's store, so a chat moving redraws its
 * rows and not the project view around them.
 */

/**
 * **One chat's state as a row says it, a mark and a word, which reads that chat's state
 * itself** (#1484, SC-3). The Chats list's row and the explorer's both draw this, from the same
 * facts through the one function (`useShownState`), so they say the same of a chat, and so does
 * its tab (`TabStateMark`, #4935).
 *
 * Redrawn only when what it shows changes: a move redraws the moved chat's state and no row.
 * Held on plain values, so a row redrawn for its own reasons does not redraw it.
 */
export const ChatShownState = memo(function ChatShownState({
  changed = false,
  markOnly = false,
  ...of
}: ShownOf & {
  /** Its state changed while its row was not drawn, so it arrives in it (`StateShown`). */
  changed?: boolean;
  /** Its mark alone, its word said only to a screen reader: a narrow pane's head (#4760). */
  markOnly?: boolean;
}) {
  const shown = useShownState(of);
  return shown === undefined ? null : (
    <StateShown shown={shown} changed={changed} markOnly={markOnly} />
  );
});

/**
 * **What a chat's tab says it is doing** (#4935): the word its row says (`useShownState`), on
 * the tab's one `.state` mark, coloured by the board's state as the corner's dot is. A chat with
 * nothing to show draws no mark.
 *
 * Every row that draws a chat's tab — on the strip, and in the show-more menu — draws this, and
 * it is all of the tab that a move changes (SC-3): subscribed to its own chat, so a move redraws
 * the moved chat's marks and no other row; held (`memo`), so a tab redrawn for its own reasons
 * does not redraw it.
 */
const TabStateMark = memo(function TabStateMark(of: ShownOf) {
  const shown = useShownState(of);
  const board = useChatsSelect(useChatsHere(), (states) => markOf(states, of.session, of.shell));
  if (shown === undefined) return null;
  return <ChatMark state={board ?? "unknown"} word={shown.word} />;
});

/**
 * **Whether tab `id` shows a chat being stopped** (#1459): its own chat, or the task one of its
 * panes shows in its place. A task of the tab that is not on screen is the chip's to tell.
 */
export function showsStopping(tabs: Tabs, id: number, stopping: ReadonlySet<number>): boolean {
  return shownIn(tabs, id).some((one) => stopping.has(one.own) || stopping.has(one.session));
}

/** No chats: what a tab is handed where nothing is being stopped. */
const NO_SESSIONS: ReadonlySet<number> = new Set();

/** What a tab says of a chat it is told nothing of: no task, and no harness to name. */
const NO_FACTS: TabFacts = { report: null, outcome: null, harness: null };

/** What a tab is told of its chat beside the board, as its row has it (#4935). Its question is
 *  read where its state is (`useShownState`). */
export type TabFacts = Pick<RowFacts, "report" | "outcome" | "harness">;

/** No names: what a tab with no hidden task that needs you is handed. */
const NO_NAMES: readonly string[] = [];

/**
 * What a tab says about itself, on the strip and in the menu of what the strip has no room for.
 *
 * **A chat's tab is its name and what it is doing; a view's tab is a mark and its name**, with
 * no state — a view is not doing anything, and a dot beside it would be a claim about a chat
 * the tab does not have. The mark says what kind of thing the tab holds before the name is
 * read: a person for a persona, a puzzle piece for a view an extension offers.
 */
export function TabMarks({
  tabs,
  id,
  updates,
  shells,
  pin,
  wrapping,
  stopping = NO_SESSIONS,
  face,
  personaOf,
  factsOf,
  sandbox,
  task,
  taskOf,
  needs = NO_NAMES,
}: {
  tabs: Tabs;
  id: number;
  /**
   * **The tab is a task's own** (#1489, V100-38): the name of the chat that asked for it, or
   * nothing where that chat has no name to say any more. The tab then wears the task mark
   * before its name and says whose task it is after it, dimmer (`talk · steward 4`), so it is
   * never read as a session's tab. Left out for every other tab.
   */
  taskOf?: { asker: string | undefined };
  /** The task the tab shows in place of its session's own chat, by name (#1486): the label
   *  says it after the session's name, dimmer. Nothing while the tab shows its own chat. */
  task?: string;
  /** The tab's chats that need you and are not on screen in it, by name, longest waiting
   *  first (#1486): the hand, where the tab is a row of the menu of what the strip has no
   *  room for. **Left out on the strip** (#1487): there the hand is on the tab's chip, beside
   *  the tab, and is pressed (`TabChip.tsx`). */
  needs?: readonly string[];
  /** What the tab wears (`tabFace.ts`): its focused pane's icon and name, and `⊟+N`. */
  face: TabFace;
  /** The persona a chat runs as, when it runs as one: its mark is the combined icon's. */
  personaOf: (session: number) => string | null | undefined;
  /** What a chat's row reads its state from beside the board ({@link TabFacts}), so its tab
   *  says the row's word (#4935). A chat it knows nothing of is a chat that is no task. */
  factsOf?: (session: number) => TabFacts | undefined;
  /** How much the tab's chat's sandbox holds (#2071): its mark is drawn after the name. */
  sandbox?: SandboxSaid | null;
  /** The chats wrapping up — being smart-closed (ADR 0064). */
  wrapping: ReadonlySet<number>;
  /** The chats being stopped (#1459): a tab that shows one says so, as its row does. */
  stopping?: ReadonlySet<number>;
  /** The chats the plane's instructions changed under, by session (charter#369). Left out on the
   *  strip, where the mark is a button beside the tab (`FreshMark`) rather than inside it. */
  updates?: PlaneUpdates;
  /** The chats that are shell tabs, whose tab wears a terminal's mark (SI-5). */
  shells: ReadonlySet<number>;
  /** The pin mark, on the strip; the menu of hidden tabs draws none. */
  pin?: ReactNode;
}) {
  const lead = face.lead;
  if (lead.kind !== "chat") {
    const focused = contentsOf(tabs, id).find((one) => one.pane === tabs.byId[id]?.focused);
    const preview = focused?.content.kind === "view" && focused.content.preview === true;
    return (
      <>
        {pin}
        {lead.kind === "view" ? (
          <ViewMark view={lead.view} />
        ) : (
          <Plus className="tab-mark" data-mark="empty" aria-hidden="true" />
        )}
        {/* The preview tab is in italics, as VS Code draws its own: the tab the next single
            click on a memory reuses (SI-9b). */}
        <span className={preview ? "tab-name is-preview" : "tab-name"} {...tabPart("label")}>
          {face.name}
        </span>
        <SplitMore more={face.more} zoomed={face.zoomed} />
      </>
    );
  }
  const chat = lead.session;
  const shell = shells.has(chat);
  const persona = personaOf(chat);
  // What its row reads its state from beside the board (#4935), so the two say one word.
  const facts = factsOf?.(chat) ?? NO_FACTS;
  return (
    <>
      {/* **A pinned tab shows its pin before its icon** (#4762), and the pin is the tab's. */}
      {pin}
      {/* **The focused pane's chat's combined icon** (#4762): who it runs as, a persona's
          mark or a shell's terminal (#1449), with its state's mark on its corner, in the
          sidebar's look (`chatPersona.css`). The marks are the tab's own, as before: the state
          dot is the board's. The badge is only seen: the tab says its state after
          its name, below, as it always has. A chat with neither wears the state mark alone. A
          task's own tab wears the task mark (#1489), whose state is said after its name. */}
      {taskOf !== undefined ? (
        <ListTodo className="tab-mark" data-mark="task" aria-hidden="true" />
      ) : (
        <span className="chat-persona tab-persona">
          {shell ? (
            <SquareTerminal className="tab-mark" data-mark="shell" aria-hidden="true" />
          ) : (
            persona != null && <PersonaMark persona={persona} className="tab-mark" />
          )}
          <StateBadge session={chat} shell={shell} />
        </span>
      )}
      <span className="tab-name" {...tabPart("label")}>
        {face.name}
      </span>
      <SplitMore more={face.more} zoomed={face.zoomed} />
      <SandboxMark level={sandbox?.level} why={sandbox?.why} sites={sandbox?.sites} />
      <PlaneUpdatedMark files={updates?.[chat]} />
      {/* **What the tab is doing, said after what it is**: its own element, so a tab whose text
          changed every time a turn began would not be unreadable, and before the task's name
          where it shows one (#1486). A task's own tab draws it here (#1489); any other tab wears
          it as the icon's badge, so here it is heard and not seen. */}
      {taskOf !== undefined ? (
        <TabStateMark session={chat} shell={shell} {...facts} />
      ) : (
        <span className="hidden-words">
          <TabStateMark session={chat} shell={shell} {...facts} />
        </span>
      )}
      {/* **`talk · steward 4`, on a task's own tab** (#1489): whose task it is, dimmer. To a
          screen reader the separator is the words it stands for. */}
      {taskOf !== undefined && (
        <>
          <span className="hidden-words">, a task{taskOf.asker === undefined ? "" : " of "}</span>
          {taskOf.asker !== undefined && (
            <>
              <span className="tab-task-sep" aria-hidden="true">
                {" · "}
              </span>
              <span className="tab-task tab-asker">{taskOf.asker}</span>
            </>
          )}
        </>
      )}
      {/* **`steward 4 › talk`, while the tab shows a task** (#1486, V100-35). The tab is the
          session's, so the session's name stays first and the task's is the dimmer one. To a
          screen reader the separator is the words it stands for. */}
      {task !== undefined && (
        <>
          <span className="tab-task-sep" aria-hidden="true">
            {" › "}
          </span>
          <span className="hidden-words">, showing task </span>
          <span className="tab-task">{task}</span>
        </>
      )}
      {/* **A chat of this tab is waiting for you and is not on screen** (#1486, V100-37): the
          hand, as a row of the Chats list wears it. A mark and no button here: this is inside
          a button, the tab's own or a row of the show-more menu. On the strip the hand is the
          chip's and goes to that chat (#1487); from the menu the way is to bring the tab
          forward, and its chip is there. */}
      {needs.length > 0 && (
        <Tooltip tip={needsSaid(needs)}>
          <span
            className="needs-you-mark"
            data-mark="needs-you"
            role="img"
            aria-label={needsSaid(needs)}
          >
            <Hand aria-hidden="true" />
          </span>
        </Tooltip>
      )}
      <StoppingMark held={showsStopping(tabs, id, stopping)} />
      <WrappingUp held={panesOf(tabs, id).some((one) => wrapping.has(one.session))} />
    </>
  );
}

/**
 * **The state's dot on a tab's icon** (#4762): seen and not heard, since the tab says its state
 * after its name (`TabStateMark`, in hidden words). A dot of its own, read off the chat's
 * state as that mark reads it, so a move redraws the two and no other row (SC-3). A chat with
 * no mark to draw (`markOf`) wears none.
 */
const StateBadge = memo(function StateBadge({
  session,
  shell,
}: {
  session: number;
  shell: boolean;
}) {
  const state = useChatsSelect(useChatsHere(), (states) => markOf(states, session, shell));
  if (state === undefined) return null;
  // `state-dot`, never `state`: the tab's one `.state` is the mark that says the word, which
  // the scenario specs read (`chat.state.e2e.ts`).
  return <span className={`state-dot state-${state}`} data-state={state} aria-hidden="true" />;
});

/**
 * **`⊟+N`: how many other panes share a split tab** (#4762), after its name and never giving way
 * to it: the name is cut first. A screen reader hears it as words.
 */
function SplitMore({ more, zoomed }: { more: number; zoomed?: true }) {
  if (more <= 0) return null;
  return (
    <>
      <span className="tab-split-more" data-mark="split">
        <Columns2 aria-hidden="true" />
        <span aria-hidden="true">+{more}</span>
        <span className="hidden-words">, split, {more + 1} panes</span>
      </span>
      {/* **A zoomed pane's mark** (#4765): one pane fills the tab for now, and Focus this pane
          brings the others back. */}
      {zoomed && (
        <span className="tab-zoomed" data-mark="zoomed">
          <Maximize2 aria-hidden="true" />
          <span className="hidden-words">, one pane zoomed</span>
        </span>
      )}
    </>
  );
}

/** The states a persona's row tells, the strongest first (#3341): the person is asked for, then
 *  a question or a wait is open, then something is at work. A persona at rest tells none. */
const AT_WORK: readonly Shown["kind"][] = [
  "needs-you",
  "asking",
  "waiting-on-tasks",
  "waiting-on-own-work",
  "working",
];

/** The strongest of `sessions`' states that tells a persona at work (#3341), or nothing for a
 *  persona whose chats all rest. The one reading both the row's mark and its group use (#3483),
 *  so the two always agree. */
function atWorkOf(states: ChatStates, sessions: readonly number[]): Shown | undefined {
  let best: Shown | undefined;
  for (const session of sessions) {
    const one = shownState({
      board: markOf(states, session, false),
      needsYou: states.needsYou.includes(session),
      task: null,
      harness: null,
      ownWork: ownWorkOf(states, session),
    });
    if (one === undefined || !AT_WORK.includes(one.kind)) continue;
    if (best === undefined || AT_WORK.indexOf(one.kind) < AT_WORK.indexOf(best.kind)) best = one;
  }
  return best;
}

/**
 * **The personas with a chat at work or asking** (#3483): what the Personas view's Working now
 * holds, by {@link atWorkOf}, as the row's mark reads it. Redrawn only when who is at work
 * changes, never on a move that leaves it the same.
 */
export function usePersonasAtWork(
  personaChats: ReadonlyMap<string, readonly number[]> | undefined,
): ReadonlySet<string> {
  const names = useChatsSelect(useChatsHere(), (states) =>
    [...(personaChats ?? NO_PERSONA_CHATS)]
      .filter(([, sessions]) => atWorkOf(states, sessions) !== undefined)
      .map(([persona]) => persona)
      .sort()
      .join("\n"),
  );
  return useMemo(() => new Set(names === "" ? [] : names.split("\n")), [names]);
}

/** No persona's chats: what a view drawn without them is handed. */
const NO_PERSONA_CHATS: ReadonlyMap<string, readonly number[]> = new Map();

/**
 * **Whether a persona's chats are at work or ask for you** (#3341, the prototype's Personas
 * `.dot`): the strongest state among its chats, as its own mark (a shape and a colour,
 * `shownState.ts`), with its word for a screen reader. Nothing for a persona at rest.
 */
export const PersonaAtWork = memo(function PersonaAtWork({
  sessions,
}: {
  sessions: readonly number[];
}) {
  const shown = useChatsSelect(useChatsHere(), (states) => atWorkOf(states, sessions), sameShown);
  if (shown === undefined) return null;
  return (
    <span className="persona-at-work">
      <StateShown shown={shown} markOnly />
    </span>
  );
});
