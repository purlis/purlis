// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo, useMemo, type ReactNode } from "react";
import { Hand, ListTodo, Maximize2, Plus } from "lucide-react";
import { StoppingMark, WrappingUp } from "./NeedsYou";
import {
  asksThePerson,
  failuresUnseenOf,
  markOf,
  ownWorkOf,
  stuckOf,
  useChatsHere,
  useChatsSelect,
  type ChatStates,
} from "./chatState";
import { PlaneUpdatedMark, type PlaneUpdates } from "./PlaneUpdated";
import { contentsOf, panesOf, shownIn, type Tabs } from "./tabs";
import type { TabFace } from "./tabFace";
import {
  ChatPersonaState,
  sameShown,
  useShownState,
  useUrgentChat,
  type ShownOf,
} from "./ChatPersonaState";
import { ViewMark } from "./Views";
import { CHAT_STATE_SAID, shownState, type RowFacts, type Shown } from "./shownState";
import { StateShown } from "./StateShown";
import { needsSaid } from "./tabTasks";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { TabPicture, tabPart } from "./ui/tabs/Tabs";

/*
 * **The rows that draw a chat** (SC-3): split out of `PlaneView`, whose rendering they no longer
 * share. Each reads its own chat's state off the project's store, so a chat moving redraws its
 * rows and not the project view around them.
 */

/**
 * **One chat's state as a row says it, a mark and a word, which reads that chat's state
 * itself** (#1484, SC-3). The Chats list's row and the explorer's both draw this, from the same
 * facts through the one function (`useShownState`), so they say the same of a chat, and so does
 * its tab (`TabStateWord`, #4935).
 *
 * Redrawn only when what it shows changes: a move redraws the moved chat's state and no row.
 * Held on plain values, so a row redrawn for its own reasons does not redraw it.
 */
export const ChatShownState = memo(function ChatShownState({
  changed = false,
  markOnly = false,
  heardOnly = false,
  ...of
}: ShownOf & {
  /** Its state changed while its row was not drawn, so it arrives in it (`StateShown`). */
  changed?: boolean;
  /** Its mark alone, its word said only to a screen reader: a narrow pane's head (#4760). */
  markOnly?: boolean;
  /** **Its word alone, said only to a screen reader** (#5446): where a mark beside it already
   *  draws the state's glyph, so the glyph is never drawn twice. */
  heardOnly?: boolean;
}) {
  const shown = useShownState(of);
  if (shown === undefined) return null;
  if (heardOnly) return <span className="hidden-words">{shown.word}</span>;
  // The one state's glyph (#5340), so this line never disagrees with the mark beside it.
  const said = { ...shown, ...CHAT_STATE_SAID[shown.mark] };
  return <StateShown shown={said} changed={changed} markOnly={markOnly} />;
});

/**
 * **What a chat's tab says it is doing** (#4935): the word its row says (`useShownState`), heard
 * after the tab's name. Its mark is seen on the tab's icon (`ChatPersonaState`).
 *
 * Every row that draws a chat's tab — on the strip, and in the show-more menu — draws this, and
 * it is all of the tab that a move changes (SC-3): subscribed to its own chat, so a move redraws
 * the moved chat's marks and no other row; held (`memo`), so a tab redrawn for its own reasons
 * does not redraw it.
 */
const TabStateWord = memo(function TabStateWord(of: ShownOf) {
  const shown = useShownState(of);
  if (shown === undefined) return null;
  // A name and no text: a tab's text is its name (`.tab-name`), and this is heard after it.
  return <span className="tab-said hidden-words" role="img" aria-label={shown.word} />;
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
   * never read as the tab of the chat that asked. Left out for every other tab.
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
  /** What the tab wears (`tabFace.ts`): its first pane's name, its panes' chats for its mark,
   *  and its picture of its layout. */
  face: TabFace;
  /** The persona a chat runs as, when it runs as one: its mark is the combined icon's. */
  personaOf: (session: number) => string | null | undefined;
  /** What a chat's row reads its state from beside the board ({@link TabFacts}), so its tab
   *  says the row's word (#4935). A chat it knows nothing of is a chat that is no task. */
  factsOf?: (session: number) => TabFacts | undefined;
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
  // **Its mark is its most urgent pane's** (#5342): of every chat its panes hold, a task shown in
  // a pane's place too, the one whose state comes first in the one order, so a pane that needs
  // the person can't hide behind the focused one. Its first pane's where none has a state.
  const urgent = useUrgentChat(
    face.chats.map((session) => ({
      session,
      shell: shells.has(session),
      ...(factsOf?.(session) ?? NO_FACTS),
    })),
  );
  const first = lead.kind === "chat" ? lead.session : undefined;
  const marked = urgent ?? first;
  if (marked === undefined) {
    // In italics while its first pane is the preview tab's view: the face is the first pane's.
    const shown = contentsOf(tabs, id)[0]?.content;
    const preview = shown?.kind === "view" && shown.preview === true;
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
        <SplitMore face={face} />
      </>
    );
  }
  const chat = marked;
  const shell = shells.has(chat);
  const persona = personaOf(chat);
  // What its row reads its state from beside the board (#4935), so the two say one word.
  const facts = factsOf?.(chat) ?? NO_FACTS;
  return (
    <>
      {/* **A pinned tab shows its pin before its icon** (#4762), and the pin is the tab's. */}
      {pin}
      {/* **The most urgent pane's chat's mark** (#5342, #5340): the one its row wears, by the one
          component, so the two cannot disagree. A task's own tab wears the task mark in the who's
          place (#1489), with the same state on it. The tab says its state after its name, below. */}
      <ChatPersonaState
        persona={persona ?? null}
        session={chat}
        shell={shell}
        {...facts}
        heard={false}
        who={
          taskOf !== undefined && chat === first ? (
            <ListTodo className="tab-mark" data-mark="task" aria-hidden="true" />
          ) : undefined
        }
      />
      <span className="tab-name" {...tabPart("label")}>
        {face.name}
      </span>
      <SplitMore face={face} />
      {/* No sandbox badge: it moved to the chat's pane head and its row (#4760). */}
      <PlaneUpdatedMark files={updates?.[chat]} />
      {/* **What the tab is doing, said after what it is**: its own element, so a tab whose text
          changed every time a turn began would not be unreadable, and before the task's name
          where it shows one (#1486). Heard and not seen: its mark is the icon's corner. */}
      <TabStateWord session={chat} shell={shell} {...facts} />
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
              <span className="tab-task tab-asker ui-text-meta">{taskOf.asker}</span>
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
          <span className="tab-task ui-text-meta">{task}</span>
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
 * **A split tab's small picture of its layout** (#5342, spec #5339), after its name and never
 * giving way to it: the name is cut first. It took the place of `⊟+N` (#4762): it tells the
 * shape as well as that the tab is split. A screen reader hears how many panes in words.
 */
function SplitMore({ face }: { face: TabFace }) {
  const { picture, zoomed } = face;
  if (picture === undefined) return null;
  return (
    <>
      <span className="tab-split-more" data-mark="split">
        <TabPicture picture={picture} />
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

/** The states a persona's row tells, the strongest first (#3341, #5340): the person is asked for,
 *  a task failed unseen, then a question or a wait is open, then something is at work. A persona
 *  at rest tells none. */
const AT_WORK: readonly Shown["kind"][] = [
  "needs-you",
  "stuck",
  "task-failed",
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
      needsYou: asksThePerson(states, session),
      failuresUnseen: failuresUnseenOf(states, session),
      task: null,
      harness: null,
      ownWork: ownWorkOf(states, session),
      stuck: stuckOf(states, session) !== undefined,
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
