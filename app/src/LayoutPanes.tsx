// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { QuizBand } from "./QuizBand";
import { Measured } from "./slowLog";
import {
  createContext,
  Fragment,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type CSSProperties,
  type ReactNode,
  type RefObject,
} from "react";
import { chatNumberSaid } from "./chatNumber";
import { Badge, Count } from "./ui/badge/Badge";
import { inForce, tintVariables, type Token } from "./theme/theme";
import { followTheme } from "./PersonaMark";
import { Group, Panel, useParentGroup } from "react-resizable-panels";
import { Separator } from "./ui/panel/Separator";
// The line between two groups, named apart from the panel's resize handle above.
import { Separator as Divider } from "./ui/separator/Separator";
import {
  type ChatBlocked,
  type ChatUsage,
  type HarnessGlance,
  type FinishedTask,
  type OpenChat,
  type PlaneId,
  type MemoryView,
} from "./bindings";
import {
  briefId,
  menuOn,
  OUTSIDE,
  restartId,
  workItemSaid,
  type Catalogued,
  type Offer,
} from "./actions";
import { ChatMenuButton, PaneHeadMenued } from "./Menus";
import { RestoringPane } from "./RestoringPane";
import { NONE_RESTORING } from "./restoringChats";
import { SessionPane } from "./SessionPane";
import { droppedReference, overChat, useReferenceChats } from "./references";
import { NoticeOf, NoticePaneRow } from "./Notice";
import { TerminalPromptNotice } from "./TerminalPromptNotice";
import { WaitingStrip } from "./WaitingStrip";
import { keepHolds, keptHolds, paneBand } from "./noticeSettle";
import { type Blocks } from "./sandboxBlocks";
import { ChatGauge, useChatUsage } from "./ChatGauge";
import { footKept, keepFoot } from "./footKept";
import { SkillsChip, useChatSkillsWarn } from "./UseSkill";
import { harnessCardView, isOneJob, panes, type Content, type Layout, type ViewRef } from "./tabs";
import { EVEN, shareOf, WHOLE } from "./laidOut";
import { floorAlong, type Size } from "./paneDrop";
import { drawnOf, offerInRoom, type Drawn, type Folded } from "./paneFold";
import { PaneRail, type RailPane } from "./PaneRail";
import { dropPane, GRIP, PaneGrip } from "./paneDrag";
import { paneNamesOf } from "./panePlaces";
import { type Crumbs } from "./tabChats";
import { BriefButton, type BriefAsk } from "./Brief";
import { PaneCrumbs } from "./PaneCrumbs";
import { ChipExplained, type ChipToExplain } from "./ChipExplained";
import { TaskEndAsked } from "./TaskEnd";
import { SAID } from "./menuWords";
import { giveKeyboardTo } from "./paneKeyboard";
import { StateShown } from "./StateShown";
import { ViewMark, ViewPane } from "./Views";
import type { FirstTaskDoes } from "./FirstTaskTab";
import type { ExtensionView, Shown } from "./bindings";
import { qualifierOf, shownOf } from "./finished";
import { HarnessChip } from "./HarnessCard";
import { movedAt, stateOf, useChatsHere, useChatsSelect } from "./chatState";
import { EmptyPane } from "./EmptyPane";
import { EndedBand, endedBy, endedState, resumeOf } from "./EndedBand";
import { StudioChatChip, useStudioChats } from "./studioChats";
import { markOf } from "./StripMarks";
import {
  type RestartAct,
  type RestartSaid,
  type ByHandNote,
  type StartNotes,
  ChatNotices,
} from "./ChatNotices";
import { Button, IconButton } from "./ui/button/Button";
import { TIPS_BELOW } from "./ui/tooltip/Tooltip";
import {
  Bell,
  ListTree,
  Plus,
  Puzzle,
  Reply,
  ScrollText,
  Square,
  SquareTerminal,
  Undo2,
} from "lucide-react";
import {
  paneKindOf,
  paneLinesOf,
  FOOT_ROWS,
  WHOLE_ACTIONS,
  type ActionPart,
  type FootPart,
  type FootPress,
  stateButtonOf,
  type ChatHead,
} from "./paneHead";
import { PaneTerminalAt, type PaneTerminal } from "./paneTerminal";
import { stemOf } from "./menuModel";
import { ChatShownState } from "./ChatRows";
import { ChatPersonaState } from "./ChatPersonaState";
import { SandboxMark } from "./SandboxMark";
import { PaneFoot, PaneFootEnd, PaneFootSeparator, PaneFootStart } from "./ui/pane-foot/PaneFoot";
import { WrappingUp } from "./NeedsYou";
import { NEW_CHAT } from "./tabFace";
import { bellCount, bellSays, HOLDS_NOTHING, type BandHolds } from "./paneNotices";
import { showNotices, useNoticesMinimised } from "./paneNoticesPref";
import { useSaidAtMost } from "./ui/said";
import { sessionTitle, sessionView } from "./sessions";

/**
 * **The panes of the tab in front** (#1035, split out of `PlaneView.tsx`): the split layout
 * (`LayoutPanes`), each pane's frame and breadcrumb (`PaneFrame`), the Notices that belong to
 * a pane, and the controls in a pane's top corner (`PaneDoing`).
 */

/**
 * **The rows of the panes' menu, for a terminal to list under its own** (`TerminalMenu.tsx`):
 * a right-click on a terminal opens the terminal's menu, and what the panes' menu offered
 * there — the splits, Tile all, Send F2 and Close pane — is the same catalogue rows, picked
 * out by the ids that menu lists (`actions.menuOn`).
 */
export function paneMenuOf(
  offerFor: (id: string) => Offer | undefined,
  /** The tab the panes are of: its Tile all is one of the rows (#4765). */
  tab?: number,
): Catalogued {
  const { above, below } = menuOn({ on: "pane", tab });
  return new Map(
    [...above, ...below].flatMap((id) => {
      const offer = offerFor(id);
      return offer === undefined ? [] : [[id, offer] as const];
    }),
  );
}

/** **One pane's own rows, for its head's right-click** (#4760): the panes' menu's rows, with
 *  this pane's close in place of the focused pane's, and none where its head draws no close (a
 *  pane showing a task of its session, `PaneDoing`'s `task`), so the two never disagree. */
function paneRowsOf(
  pane: number,
  tab: number,
  offerFor: (id: string) => Offer | undefined,
  closeOf: (pane: number) => Offer | undefined,
  task: boolean,
  alone: boolean,
): Catalogued {
  const close = closeOf(pane);
  const drawn = !alone && (!task || close?.does.verb === "sendBack");
  return paneMenuOf(
    (id) => (id === "pane.close" ? (drawn ? close : undefined) : offerFor(id)),
    tab,
  );
}

/**
 * **What a pane head's state word opens** (#5341): handed chat `session`, the pane it is drawn in
 * and the word, it answers the word inside the button of the chat's tasks menu, or the word as it
 * is, or nothing for a head that says no state (`alone`) where there is no menu.
 */
export type TasksOn = (
  session: number,
  at: { tab: number; pane: number } | undefined,
  word: ReactNode,
  /** The head says no state: draw nothing where there is no menu to open. */
  alone: boolean,
) => ReactNode;

/**
 * **What a chat uses now, drawn on its pane head** (#5524): its skill run open and its browsers,
 * each pressed to open what it is. They left its tab, which says which chat this is and whether
 * it needs you (#5339). Nothing for a chat that uses nothing.
 */
export type UsingOn = (session: number) => ReactNode;

/** The word as it is: a head with nothing to open. */
const asItIs: TasksOn = (_session, _at, word, alone) => (alone ? null : word);

/**
 * **Its state**, as its row says it: the mark and its word, the word going first in a narrow pane
 * and then the mark (`paneHead.ts`). **And its tasks, from that word** (#5341): where the chat has
 * tasks, the word is the button their menu opens from, as ⌘⇧J's is. A head that says no state
 * (a task's path says it, or the pane is too narrow) still draws the mark alone as that button
 * where the chat has tasks, so ⌘⇧J always has its menu to open.
 */
export function PaneTasksWord({
  tasksOn = asItIs,
  session,
  at,
  word,
  markWord,
  chat,
}: {
  tasksOn?: TasksOn;
  session: number;
  at: { tab: number; pane: number } | undefined;
  /** Whether the head says the state. */
  word: boolean;
  markWord: boolean;
  chat: ChatHead["chat"];
}) {
  // **The state's glyph is drawn once on a head** (#5446, #5524): the mark it starts with wears it
  // on its badge, so the line says it again only with its word. A head with no word draws no
  // state of its own: only the button of its tasks, where it has any, drawn as the tasks' glyph
  // and named by the word, as it is with one.
  const said = word && markWord;
  const state = (
    <ChatShownState
      session={session}
      shell={chat.shell}
      report={chat.report}
      outcome={chat.outcome}
      harness={chat.harness}
      heardOnly={!said}
    />
  );
  const drawn = tasksOn(
    session,
    at,
    said ? (
      state
    ) : (
      <span className="pane-tasks-glyph">
        <ListTree aria-hidden="true" />
        {state}
      </span>
    ),
    !said,
  );
  return drawn == null ? null : <span className="pane-state-mark">{drawn}</span>;
}

/**
 * One pane's frame, three rows each with one job (#3142, the operator's variant A of
 * 2026-10-24; #4754): **the pane's header** with the chat at a glance at its start and the
 * pane's own controls at its end, its bell among them (`PaneHead`, `PaneBell`), **the band** of
 * what purlis has to say of its chats under it (`PaneNotices`), so a Notice arriving never moves
 * the header, then **the terminal**, which has the room the two leave.
 *
 * - **All of it is inside the pane**, on the terminal's own fill, so the chosen tab joins the
 *   pane as one piece whatever the band holds: nothing is drawn between a tab and its pane, and
 *   the top of a pane is always the terminal's fill (`paneFrame.css`).
 * - **Nothing is drawn over the terminal.** The chips and the controls floated in the pane's
 *   two corners over its first lines (charter-app#193), and covered the harness's own banner;
 *   the Notices had a row above the pane (#1647), between the tab and its pane. Each has a row
 *   of its own now, inside the pane.
 * - **The head and the foot are always there**, with a gauge or without, so a pane does not change
 *   height when its first usage arrives (the operator's reason in #207). Only the band comes and goes,
 *   and the terminal is pinned to its bottom: it grows or shrinks from the top.
 */
export function PaneFrame({
  plane,
  pane,
  tab,
  session,
  crumbs,
  onShowChat,
  from,
  harness,
  onOpenCard,
  workItem,
  strip,
  notices,
  asked,
  others,
  doing,
  head,
  headMenu,
  ended,
  said,
  tasksOn,
  usingOn,
  focused = false,
  oneJob = false,
  shell = false,
  closes = true,
  alone = false,
  children,
}: {
  plane: PlaneId;
  /** What the head's state word opens: its chat's tasks menu, where it has tasks (#5341). */
  tasksOn?: TasksOn;
  /** What the chat uses now, after its state: its skill run and its browsers (#5524). */
  usingOn?: UsingOn;
  /** **How the path is said for a task that has ended** (#4757, #4756): its state after the
   *  path and the names of chats no longer open. Its Brief is its state button's (#4760). */
  said?: { state?: ReactNode; gone?: (session: number) => boolean };
  /** **The chat's program has ended** (#4756): the ended band, under the header and the
   *  notices, above the last screen. */
  ended?: ReactNode;
  /** The pane it draws, by its number in the tabs: its band is remembered by it (#3167). */
  pane?: number;
  /** The tab it is a pane of: with `pane`, where a dragged pane lands (#4763). */
  tab?: number;
  session: number;
  /** Whether the keyboard is in this pane: a split says which of its panes has it (#2914). */
  focused?: boolean;
  /** **A one-job program's pane** (#4761; spec #4750 story 78): its head is no grip, so it
   *  cannot be picked up. It goes when its program exits. */
  oneJob?: boolean;
  /** **It runs a shell**, as the open chats say before the plane lists it: it draws no foot. A
   *  shell still restoring is not known yet, and has an empty foot until it is. */
  shell?: boolean;
  /** False where its head draws no close: a pane showing a task of its session (#1486), since a
   *  close there would read as ending the task. */
  closes?: boolean;
  /** The only pane of its tab, whose own × does what a Close would (#5318). */
  alone?: boolean;
  /** While the pane shows a task of its tab's session: the path to it (#1486). Also for a task
   *  in a pane of its own, and the session it is beside (#1489). */
  crumbs?: Crumbs;
  /** What the pane says of the chat (#4760): its name, state, "harness · workspace", its
   *  sandbox and whether it wraps up. None where the plane has not listed it yet. */
  head?: ChatHead;
  /** The head's right-click (#4760): the chat's list, then this pane's rows. */
  headMenu?: {
    offers: Catalogued;
    pane: Catalogued;
    /** The tab the pane is in, whose Tile all is among the pane's rows (#4765). */
    tab?: number;
    onPress: (offer: Offer) => void;
  };
  /** Goes to a chat: what a name in the breadcrumb does, in this pane (#4757), and a hidden
   *  chat's Notice. */
  onShowChat: (session: number, pane?: number) => void;
  /** Where a handed-off chat came from, `↳ from steward 3 · ops`, in the chat's own corner. */
  from?: string;
  /** The harness the chat runs, at a glance, on the pane foot (HP-19): none for a shell. */
  harness?: HarnessGlance;
  /** Opens that harness's card tab. */
  onOpenCard: (glance: HarnessGlance) => void;
  /** The work item this chat works on, `Work item: <key>`, in the same corner (V60). */
  workItem?: string;
  /** What the chat the pane shows waits on the person for (`WaitingStrip`, #4563): above the
   *  band, and never one of its Notices, so none is ever put behind "+N more" over it. */
  strip?: ReactNode;
  /** What purlis has to say of the chat the pane shows (`ChatNotices`). */
  notices: ReactNode;
  /** What it asks for several of the pane's tasks at once (#1508). */
  asked?: ReactNode;
  /** And of every other chat that lives in this pane (#1486): each with whose it is. */
  others: readonly HiddenChat[];
  doing: ReactNode;
  children: ReactNode;
}) {
  // **Read off the store, by the pane itself** (SC-3): the gauge reads its record again when its
  // chat moves and keeps reading while the chat is mid-turn, and a move by another chat is no
  // reason to redraw this pane.
  const chats = useChatsHere();
  const moved = useChatsSelect(chats, (states) => movedAt(states, session));
  const running = useChatsSelect(chats, (states) => stateOf(states, session) === "running");
  // **What has ended stays said** (#5205, stories 23 and 24): its band is there only once the chat
  // has ended, and the foot says what the window last knew of it, the core having let go of a chat
  // closed on purpose. Its gauge reads no more.
  const hasEnded = ended !== undefined;
  const last = hasEnded ? footKept(plane, session) : {};
  const read = useChatUsage(plane, session, moved, running, last.usage !== undefined);
  const usage = last.usage ?? read;
  const lastHead = head ?? last.head;
  const lastHarness = harness ?? last.harness;
  useEffect(() => {
    if (!hasEnded) keepFoot(plane, session, { head, harness, usage: read });
  }, [hasEnded, plane, session, head, harness, read]);
  const lent = useReferenceChats();
  const frame = useRef<HTMLDivElement>(null);
  // **What a narrow pane drops, in the ruled order** (#4760, #5206, `paneHead.ts`).
  const width = usePaneWidth(frame);
  const skillsWarn = useChatSkillsWarn(session);
  const startedByStudio = useStudioChats().chats.some((one) => one.session === session);
  // **Its two lines** (#5202, `paneHead.paneLinesOf`): the head's chips, and the foot's parts,
  // or no foot for a pane whose program has no harness or gauge to say.
  const kind = paneKindOf({
    task: crumbs !== undefined,
    oneJob,
    // The plane's listing, once it has come; until then what the open chats said of it, so a
    // chat reserves its foot from its first frame and its terminal gets one size (story 46).
    shell: lastHead?.chat.shell ?? shell,
    ended: hasEnded,
  });
  const lines = paneLinesOf(kind, width, {
    from: Boolean(from),
    skills: skillsWarn !== undefined,
    studio: startedByStudio,
    workItem: Boolean(workItem),
    sandbox: lastHead?.sandbox?.level != null,
    using: usingOn !== undefined,
    closes,
    alone,
  });
  const line = new Set(lines.head);
  /** **What a part of the foot presses** (#5204, spec #5191 story 37): its row of the chat's list,
   *  for a foot's part has no press of its own. None where the chat has no such row, or where the
   *  row cannot run now: the part is then its words alone. */
  const footPress = (part: keyof typeof FOOT_ROWS): FootPress | undefined => {
    const offer = headMenu?.offers.get(FOOT_ROWS[part](session));
    return offer === undefined || !offer.available
      ? undefined
      : { title: offer.title, press: () => headMenu?.onPress(offer) };
  };
  /** What the band holds, for the bell in the pane's header (#4754), and the bell itself, which
   *  the band's Dismiss all hands the keyboard to once it has cleared the band. */
  const [holds, setHolding] = useState<BandHolds>(HOLDS_NOTHING);
  // And kept by its band, for the rail that says it while the pane is folded (#4766).
  const setHolds = useCallback(
    (now: BandHolds) => {
      setHolding(now);
      if (pane !== undefined) keepHolds(paneBand(plane, pane), now);
    },
    [plane, pane],
  );
  const bellAt = useRef<HTMLButtonElement>(null);
  /** The terminal the pane draws, for Copy and Paste on its head's right-click (#4760). */
  const terminalAt = useRef<PaneTerminal | null>(null);
  const focusBell = useCallback(() => bellAt.current?.focus(), []);
  /** **The pane's grip** (#5290): its persona's mark, but a one-job program's. The same function
   *  from one drawing of the head to the next, so the mark (a memo) is not drawn again for it. */
  const grip = useMemo(
    () =>
      oneJob || tab === undefined || pane === undefined
        ? undefined
        : (mark: ReactNode) => (
            <PaneGrip tab={tab} pane={pane}>
              {mark}
            </PaneGrip>
          ),
    [oneJob, tab, pane],
  );
  return (
    <div
      ref={frame}
      className="pane-frame pane-headed"
      data-focused={focused || undefined}
      {...(tab === undefined || pane === undefined ? {} : dropPane(tab, pane))}
      // A reference dropped on the chat's pane is typed into it, unsent (FM-9).
      onDragOver={overChat}
      onDrop={(event) => {
        const r = droppedReference(event);
        if (r === undefined || lent === undefined) return;
        event.preventDefault();
        lent.hand(r, session, "add");
      }}
    >
      {/* **The band and the header first**: they are the top of the pane, and the first things
          read and tabbed to (#1647); a terminal keeps Tab for its shell, so a control written
          after it is one Tab never reaches (charter-app#189). */}
      <div className="pane-top">
        {strip}
        <PaneActionsOf.Provider value={lines.actions}>
          <PaneHead
            grip={!oneJob}
            menu={
              headMenu &&
              ((element) => (
                <PaneHeadMenued
                  session={session}
                  offers={headMenu.offers}
                  pane={headMenu.pane}
                  tab={headMenu.tab}
                  terminal={terminalAt}
                  onPress={headMenu.onPress}
                >
                  {element}
                </PaneHeadMenued>
              ))
            }
            doing={
              <PaneBellAt.Provider value={bellAt}>
                <PaneBellOf.Provider value={holds}>{doing}</PaneBellOf.Provider>
              </PaneBellAt.Provider>
            }
          >
            <div className="pane-chips">
              {/* **Who it runs as, with its state on it** (#4760, #4762): the one icon its row
              and its tab wear, first on the head. It is the pane's grip (#5290), but a one-job
              program's, which stays beside the pane it opened from. */}
              {head && line.has("persona") && (
                <span className="pane-persona">
                  <ChatPersonaState
                    grip={grip}
                    persona={head.chat.persona}
                    session={session}
                    shell={head.chat.shell}
                    report={head.chat.report}
                    outcome={head.chat.outcome}
                    harness={head.chat.harness}
                  />
                </span>
              )}
              {/* **Which chat this is** (#4760): its name, or, while the pane shows a task, the
              path to it (#1486), which ends with the task's name and says its state. */}
              {crumbs ? (
                <PaneCrumbs
                  crumbs={crumbs}
                  onShow={(chat) => onShowChat(chat, pane)}
                  state={said?.state}
                  gone={said?.gone}
                />
              ) : (
                head && line.has("name") && <span className="pane-name">{head.chat.name}</span>
              )}
              {/* **Its number beside its name** (#6212): the chat's current one, so a chat
              started again says its new number; while the pane shows a task, the task's. */}
              {/* Read off the pane, never its row: a chat just started again is not listed yet. */}
              {line.has("number") && (
                <PaneNumber
                  session={crumbs ? crumbs.path[crumbs.path.length - 1].session : session}
                />
              )}
              {/* A task's end, or its restart, asked on its line from its ⋯ (#1488). */}
              {crumbs && (
                <TaskEndAsked
                  session={crumbs.path[crumbs.path.length - 1].session}
                  focusBack={() => giveKeyboardTo(plane, session)}
                />
              )}
              {/* **The sandbox of a pane with no foot** (#4760, spec #5191): a chat's says it on its
              foot, and a narrow pane drops it with the line's details. */}
              {head && line.has("sandbox") && (
                <SandboxMark
                  level={head.sandbox?.level}
                  why={head.sandbox?.why}
                  sites={head.sandbox?.sites}
                />
              )}
              {/* **Its state**, as its row says it: the mark and its word, the word going
              first in a narrow pane and then the mark (`paneHead.ts`). */}
              {head && (
                <PaneTasksWord
                  tasksOn={tasksOn}
                  session={session}
                  at={tab === undefined || pane === undefined ? undefined : { tab, pane }}
                  word={line.has("mark")}
                  markWord={line.has("markWord")}
                  chat={head.chat}
                />
              )}
              {/* **Wrapping up** (ADR 0064, #4760): the breathing mark, where the strip's chip
              was. */}
              {head?.wrapping === true && (
                <span className="pane-wrapping">
                  <WrappingUp held />
                </span>
              )}
              {/* **What it uses now** (#5524): its skill run and its browsers, which left its
              tab, beside its state; a narrow pane drops them after the state's word (#5614). */}
              {head && usingOn !== undefined && line.has("using") && (
                <span className="pane-using">{usingOn(session)}</span>
              )}
              {/* **harness · workspace on a pane with no foot** (spec #5191): a shell's and a
              one-job program's keep their heads. */}
              {line.has("where") && saysWhere(head, harness) && (
                <span className="pane-head-where">
                  <PaneWhere
                    head={head}
                    harness={harness}
                    onHarness={harness && (() => onOpenCard(harness))}
                    between=" · "
                  />
                </span>
              )}
              {line.has("from") && <span className="pane-from">{from}</span>}
              {line.has("workItem") && workItem && (
                <span className="pane-work-item">{workItemSaid(workItem)}</span>
              )}
            </div>
          </PaneHead>
        </PaneActionsOf.Provider>
        {/* **The band under the header** (#4754): a Notice arriving never moves the header. */}
        <PaneNotices
          band={pane === undefined ? undefined : paneBand(plane, pane)}
          notices={
            <>
              {/* **A task's question, as a quiz, first in the band under the head** (#4565,
                  #5213): one of what waits on the person here, folded by the band's setting,
                  and above the terminal, so its input never moves for it. */}
              <QuizBand plane={plane} session={session} />
              {notices}
            </>
          }
          asked={asked}
          others={others}
          onShowChat={onShowChat}
          onHolds={setHolds}
          onFocusBell={focusBell}
        />
        {ended}
      </div>
      <PaneTerminalAt.Provider value={terminalAt}>
        <div className="pane-body">{children}</div>
      </PaneTerminalAt.Provider>
      {/* **The pane foot** (#5202, spec #5191, look A): under the terminal. Drawn whether or not
          the gauge is known yet, so the terminal keeps its rows when the first usage arrives;
          after it in the tab order, as it is drawn. */}
      {lines.foot !== null && (
        <ChatFoot
          plane={plane}
          session={session}
          parts={lines.foot}
          head={lastHead}
          crumbs={crumbs}
          harness={lastHarness}
          from={from}
          workItem={workItem}
          usage={usage}
          rowOf={footPress}
        />
      )}
    </div>
  );
}

/**
 * **What a pane foot draws** (#5202, #5204, spec #5191, look A), from what the pane's model answers
 * (`parts`): where the chat runs, where it came from and how healthy it is. Each part that can be
 * pressed runs its row of the chat's list (`rowOf`), and nothing on it asks for anything.
 */
export function ChatFoot({
  plane,
  session,
  parts,
  head,
  crumbs,
  harness,
  from,
  workItem,
  usage,
  rowOf,
}: {
  plane: PlaneId;
  session: number;
  parts: readonly FootPart[];
  head: ChatHead | undefined;
  crumbs: Crumbs | undefined;
  harness: HarnessGlance | undefined;
  from: string | undefined;
  workItem: string | undefined;
  usage: ChatUsage | undefined;
  rowOf: (part: keyof typeof FOOT_ROWS) => FootPress | undefined;
}) {
  const foot = new Set(parts);
  return (
    <PaneFoot
      className="pane-foot"
      label={`${footName(head, crumbs)}: where it runs and its context`}
    >
      <PaneFootStart>
        {/* **harness · workspace** (#4760, #5202), each a shortcut to its row (#5204). */}
        {saysWhere(head, harness) && (
          <span className="pane-where">
            <PaneWhere
              head={head}
              harness={harness}
              onHarness={rowOf("harness")?.press}
              onWorkspace={rowOf("workspace")}
              between={<PaneFootSeparator />}
              workspace={foot.has("workspace")}
            />
          </span>
        )}
        {/* **Where it came from** (#5204): the chat that started it, what the Skill studio
            started it for (#5185) and the work item it works on, each a fact of its origin. */}
        {foot.has("from") && (
          <PaneFootJoined>
            <FootFrom said={from ?? ""} row={rowOf("from")} />
          </PaneFootJoined>
        )}
        {foot.has("studio") && (
          <PaneFootJoined>
            <StudioChatChip plane={plane} session={session} onOpen={rowOf("studio")?.press} />
          </PaneFootJoined>
        )}
        {foot.has("workItem") && workItem && (
          <PaneFootJoined>
            <span className="pane-foot-work-item">{workItemSaid(workItem)}</span>
          </PaneFootJoined>
        )}
        {/* **Its skills warning** (#2775, #5204): only while something needs the person. */}
        {(foot.has("skills") || foot.has("skillsMark")) && (
          <PaneFootJoined>
            <SkillsChip
              plane={plane}
              session={session}
              row={rowOf("skills")}
              markOnly={foot.has("skillsMark")}
            />
          </PaneFootJoined>
        )}
        {/* **Its sandbox** (#4760, #5204): how the chat runs, a mark with its words on it. */}
        {foot.has("sandbox") && (
          <PaneFootJoined>
            <span className="pane-foot-sandbox">
              <SandboxMark
                level={head?.sandbox?.level}
                why={head?.sandbox?.why}
                sites={head?.sandbox?.sites}
              />
            </span>
          </PaneFootJoined>
        )}
      </PaneFootStart>
      <PaneFootEnd>
        {/* **The gauge at the end** (#3142, rule 15; #5202): drawn once the chat's first
            usage arrives, in a line that is there before it, so its arrival moves nothing.
            `ctx` is the chat's Activity's shortcut (#5204). */}
        {foot.has("ended") && (
          <span className="pane-foot-ended">
            <Badge variant="neutral">ended</Badge>
          </span>
        )}
        <ChatGauge usage={usage} parts={parts} context={rowOf("ctx")} ended={foot.has("ended")} />
      </PaneFootEnd>
    </PaneFoot>
  );
}

/** **A chat's number on its pane's head** (#6212): `#641`, beside its name or the path to the task
 *  it shows. Its stories draw this one, so the window's and theirs cannot drift apart. */
export function PaneNumber({ session }: { session: number }) {
  return <span className="pane-number">{chatNumberSaid(session)}</span>;
}

/**
 * **A pane's header** (#3142): one slim row under the band and over the terminal, the chat at a
 * glance at its start (its `children`, the `.pane-chips` line: its name or the
 * breadcrumb and its state; written where it is drawn, so the line's guard reads it) and the pane's
 * actions at its end (`PaneActions`). A row of its own, so neither covers what the terminal
 * draws; the same row in every pane with a chat, so panes side by side line up. Its tooltips
 * open below it, as a strip's do (`TIPS_BELOW`, #5189). With no
 * `children` it is the actions alone at the top right of a pane, as a view's pane draws them
 * (#3147).
 */
export function PaneHead({
  doing,
  menu,
  children,
  grip = true,
  lead,
}: {
  doing: ReactNode;
  /** Puts the head's right-click around it (#4760): the chat's list, then the pane's rows. */
  menu?: (head: ReactNode) => ReactNode;
  children?: ReactNode;
  /** Whether a press on it, outside its controls, picks the pane up to drag (#4763). */
  grip?: boolean;
  /** **A view's grip** (#5290): its mark, first in the head, where a chat's is its persona's in
   *  `children`. Only a view in a split has one: alone in its tab, it moves by its tab. */
  lead?: ReactNode;
}) {
  const drawn = (
    // Its hints open below it (#5189): one at a control's end would stand on the row, over the
    // next control, as the bell's did over Split right.
    <div className="pane-head" {...TIPS_BELOW} {...(grip ? { [GRIP]: "" } : {})}>
      {/* **The grip, first in the head** (#4764, #5290): the pane's mark, which turns into dots; a
          press, Enter or Space on it picks the pane up to place it with the keys, and a drag
          from it moves the pane. A chat's is its persona's mark, in `children`. */}
      {lead}
      {children}
      {doing}
    </div>
  );
  return menu === undefined ? drawn : menu(drawn);
}

/**
 * **A pane's actions** (#3142, #3147, #4760, #5203): two groups in one cluster, in its own place,
 * the top right of the pane (`.pane-actions`, `paneFrame.css`). First the chat's actions: its one
 * state button, its bell (#4754), then its ⋯ last, since an overflow menu ends the group it
 * overflows. Then, after a thin divider, the pane's controls: split it side by side, split it
 * stacked, then close it, last, in the corner (the person's ruling on the M70 prototype, spec
 * #5191). Tab takes them in that order, which is the order they are written. A chat's pane
 * draws it at the end of its header; a view's pane draws the same component with no chat, so
 * every pane's split and close look and behave as one. What it offers is the catalogue's own
 * rows, always drawn (#5318).
 */
export function PaneActions({
  chat,
  state,
  ...props
}: Parameters<typeof PaneDoing>[0] & {
  /** The chat the pane shows, and the catalogue its ⋯ reads (#4753): the chat's one list, the
   *  same its sidebar row's menu and the palette have. None for a pane showing a view. */
  chat?: { session: number; offers: Catalogued };
  /** Its one state button (#4760, `PaneStateButton`), where its state has one. */
  state?: ReactNode;
}) {
  // The bell is a chat's, drawn where the pane holds a chat's notices: a view's pane has none.
  const hasBell = useContext(PaneBellOf) !== null;
  const parts = useContext(PaneActionsOf);
  return (
    <div className="pane-actions">
      {/* **The state button, then the bell, then ⋯** (#4760, #5203), each always drawn: the
          chat's part of the head, before the pane's own controls. The bell keeps a fixed slot
          with its count or none (#4754), and the controls after it are always drawn too (#5318),
          so nothing beside them moves when a notice arrives or the pointer comes. */}
      {state}
      {parts.includes("bell") && <PaneBell />}
      {parts.includes("menu") && chat !== undefined && (
        <ChatMenuButton
          session={chat.session}
          offers={chat.offers}
          onPress={(offer) => props.onPaneDoes(props.pane, offer)}
        />
      )}
      <PaneDoing {...props} parted={state != null || hasBell || chat !== undefined} />
    </div>
  );
}

/** What a workspace's badge on a pane head is drawn in: its colour, as a persona's mark takes
 *  its own (`PersonaMark.MARK_TOKENS`), whatever the theme's recipe tints. */
const ELSEWHERE_TOKENS: readonly Token[] = ["accent.surface"];

/**
 * **The badge of the workspace a pane's chat is filed in, where it is not its tab's** (#4982;
 * #4583 rule 2): drawn in "harness · workspace" in place of the workspace's plain name, and goes
 * with that part. In the workspace's colour, or plain for one with none:
 * the window's accent is the front workspace's colour, and a badge in it would say the
 * opposite. A screen reader hears that the chat is filed there, as the drop's sentence says.
 */
function ElsewhereBadge({ workspace, colour }: { workspace: string; colour: string | null }) {
  const theme = useSyncExternalStore(followTheme, inForce);
  // The colour is put on the badge's ground, which its tokens inherit: a badge takes a place,
  // never a look, of its own (`ui/badge`).
  return (
    <span
      data-colour={colour ?? undefined}
      style={tintVariables(theme, colour, ELSEWHERE_TOKENS) as CSSProperties}
    >
      <Badge variant={colour === null ? "neutral" : "accent"}>
        <span className="sr-only">filed in </span>
        {workspace}
      </Badge>
    </span>
  );
}

/** Whether a pane has any of harness · workspace to say. */
function saysWhere(head: ChatHead | undefined, harness: HarnessGlance | undefined): boolean {
  return head
    ? head.chat.workspace !== "" || harnessWord(head, harness) !== ""
    : harness !== undefined;
}

/**
 * **Harness · workspace** (#4760, #5202): the harness opens its card (HP-19), and the workspace is
 * its badge where the chat is filed elsewhere. On the pane foot, or on the head of a pane with no
 * foot: each line says what parts them.
 */
function PaneWhere({
  head,
  harness,
  onHarness,
  onWorkspace,
  between,
  workspace = true,
}: {
  head: ChatHead | undefined;
  harness: HarnessGlance | undefined;
  /** What a press on the harness does, where one does anything. */
  onHarness?: () => void;
  /** The row a press on the workspace runs (#5204): none on a head, or where the row cannot run,
   *  the workspace being the focused one. */
  onWorkspace?: FootPress;
  between: ReactNode;
  /** Whether the pane's model keeps the workspace part. */
  workspace?: boolean;
}) {
  const shown = workspace && head !== undefined && head.chat.workspace !== "";
  const words =
    shown && head ? (
      head.elsewhere ? (
        <ElsewhereBadge {...head.elsewhere} />
      ) : (
        head.chat.workspace
      )
    ) : null;
  return (
    <>
      {harness && onHarness ? (
        <HarnessChip glance={harness} onOpen={onHarness} />
      ) : (
        (harness?.title ?? head?.chat.harness)
      )}
      {shown && head && harnessWord(head, harness) !== "" && between}
      {words !== null && onWorkspace ? (
        <Button
          inline
          variant="ghost"
          className="pane-foot-press"
          tooltip={onWorkspace.title}
          aria-label={onWorkspace.title}
          onPress={onWorkspace.press}
        >
          {words}
        </Button>
      ) : (
        words
      )}
    </>
  );
}

/** **A part of the foot after another**, the `·` that joins them before it (look A). Drawn as its
 *  one child, so the part stands straight in its group (`controls.guard.test.ts`). */
function PaneFootJoined({ children }: { children: ReactNode }) {
  return (
    <>
      <PaneFootSeparator />
      {children}
    </>
  );
}

/** **`↳ from` on the foot** (#5204): a shortcut to the chat it came from, or its words alone where
 *  that chat has closed. */
function FootFrom({ said, row }: { said: string; row: FootPress | undefined }) {
  return row === undefined ? (
    <span className="pane-foot-from">{said}</span>
  ) : (
    <Button
      inline
      variant="ghost"
      className="pane-foot-from pane-foot-press"
      tooltip={row.title}
      onPress={row.press}
    >
      {said}
    </Button>
  );
}

/** What the foot's group is named by: the chat's name, or the task's at the end of its path. */
function footName(head: ChatHead | undefined, crumbs: Crumbs | undefined): string {
  return crumbs?.path[crumbs.path.length - 1]?.name ?? head?.chat.name ?? "This chat";
}

/** The harness a pane head names, by its card's word or the core's: none for a shell. */
function harnessWord(head: ChatHead, glance: HarnessGlance | undefined): string {
  return glance?.title ?? head.chat.harness ?? "";
}

/** **The controls the pane's model answers for its head** (#5206, `paneHead.paneLinesOf`): all of
 *  them where nothing says otherwise. */
export const PaneActionsOf = createContext<readonly ActionPart[]>(WHOLE_ACTIONS);

/** How wide the pane frame in `frame` is, in CSS pixels, as it is resized: none where it cannot
 *  be measured, or is not laid out, as in a test with no layout. */
function usePaneWidth(frame: RefObject<HTMLDivElement | null>): number | undefined {
  const [width, setWidth] = useState<number>();
  useLayoutEffect(() => {
    const element = frame.current;
    if (element === null || typeof ResizeObserver === "undefined") return;
    // Nothing laid out (a pane not drawn yet, or a test with no layout) is not narrow.
    const measure = () => setWidth(element.getBoundingClientRect().width || undefined);
    measure();
    const watching = new ResizeObserver(measure);
    watching.observe(element);
    return () => watching.disconnect();
  }, [frame]);
  return width;
}

/**
 * **A chat's one state button on its pane head** (#4760, spec #4750 story 55): Stop while it
 * works, Answer while a task asks, Cancel smart close while it wraps up, Brief on a task, and
 * nothing on a quiet chat (`paneHead.stateButtonOf`). It is the chat's own row of its list, so
 * it says the catalogue's title to a screen reader and runs what that row runs; on screen its
 * one word, and in a narrow pane its icon alone, named all the same.
 */
export function PaneStateButton({
  session,
  task,
  offers,
  brief,
  ended = false,
  onPress,
}: {
  session: number;
  task: boolean;
  offers: Catalogued;
  /** Where the pane can open the task's brief itself (`BriefButton`), which hands the keyboard
   *  back to the button. */
  brief?: BriefAsk;
  /** **The pane is left on a task that has ended** (#4757): nothing of it runs or is listed, so
   *  its one button is its Brief, read by its finished row's record, and none once that row is
   *  gone (#1494). */
  ended?: boolean;
  onPress: (offer: Offer) => void;
}) {
  const working = useChatsSelect(
    useChatsHere(),
    (states) => stateOf(states, session) === "running",
  );
  const worded = useContext(PaneActionsOf).includes("stateWord");
  if (ended)
    return brief === undefined ? null : (
      <BriefButton of={brief} word={worded ? SAID.brief : undefined} />
    );
  const pick = stateButtonOf({ session, task, working, has: (id) => offers.has(id) });
  const offer = pick === undefined ? undefined : offers.get(pick.id);
  if (pick === undefined || offer === undefined) return null;
  if (pick.id === briefId(session) && brief !== undefined)
    return <BriefButton of={brief} word={worded ? pick.word : undefined} />;
  const Icon = STATE_ICONS[stemOf(pick.id)] ?? Square;
  const said = {
    "aria-label": offer.title,
    tooltip: offer.reason || offer.note || undefined,
    isDisabled: !offer.available,
    onPress: () => onPress(offer),
  };
  return worded ? (
    <Button
      variant="quiet"
      size="sm"
      className="pane-state"
      icon={<Icon aria-hidden="true" />}
      {...said}
    >
      {pick.word}
    </Button>
  ) : (
    <IconButton
      variant="quiet"
      size="sm"
      className="pane-state"
      icon={<Icon aria-hidden="true" />}
      {...said}
    />
  );
}

/** Each state button's icon, by its row's act. */
const STATE_ICONS: Readonly<Record<string, typeof Square>> = {
  "chat.stop": Square,
  "chat.answer": Reply,
  "chat.smartclose.cancel": Undo2,
  "chat.brief": ScrollText,
};

/** The task a breadcrumb's pane shows, as the Brief panel is asked for it: the last chat of
 *  the path, by its number. */
export function briefOfShown(crumbs: Crumbs): BriefAsk {
  const shown = crumbs.path[crumbs.path.length - 1];
  return { chat: shown.session, name: shown.name };
}

/** **A chat ended on purpose in its pane** (#5019), as its ended band draws it. */
export type EndedChat = {
  /** Its name as it ended, since the core no longer lists it. */
  name: string;
  /** **Its way back, which is Resume**, where it has one: the record a Smart close wrote, or
   *  else its conversation (a chat on a harness). */
  resumes: boolean;
  /** The record a Smart close wrote, which its band links to (spec #4750 story 62). */
  record?: { path: string; title: string };
};

/** `↳ from` the chat that asked for the task at the end of `path`, or nothing for a path of one. */
function askedFromOf(path: Crumbs): string | undefined {
  const asker = path.path[path.path.length - 2];
  return asker === undefined ? undefined : `↳ from ${asker.name}`;
}

/** A chat that lives in a pane and is not the one the pane shows, with what purlis has to say
 *  of it. */
export type HiddenChat = {
  session: number;
  /** Whose its Notices are, as each says first: `steward 4`, or a task by its whole path,
   *  `deep (a task of steward 4 › talk)` (#1508). */
  whose: string;
  /** Whether it is a task below the pane's session, not the session's own chat (#1601). */
  task: boolean;
  notices: ReactNode;
};

/**
 * **What purlis has to say on a pane, one Notice under another** (#1481), in the pane's band
 * under its header and above its terminal, its notes folded into the bell while minimised
 * (`NoticePaneRow`, #1647, #4754).
 *
 * **Of every chat that lives in the pane, whichever one it shows** (#1486, the train-1 part of
 * V100-56). A pane shows one of its chats at a time: its own, or a task
 * below it. A Notice that waits for the person belongs to one of them, and the other may be on
 * screen. So the shown chat's Notices come first, as they always read, and after them every
 * other chat's, each saying whose it is and with **Go to it** (`NoticeOf`). An answer on one of
 * those acts on its own chat: the Notice is that chat's, drawn here.
 */
export function PaneNotices({
  band,
  notices,
  asked,
  others,
  onShowChat,
  onHolds,
  onFocusBell,
}: {
  /** The pane the band is (`paneBand`), whichever component draws it (#3167). */
  band?: string;
  /** The pane's bell, told what the band holds (#4754). */
  onHolds?: (holds: BandHolds) => void;
  /** Hands the keyboard to the pane's bell. */
  onFocusBell?: () => void;
  notices: ReactNode;
  /** The questions for several of the pane's tasks at once (#1508): after the shown chat's
   *  own, before each other chat's, since each waits for the person. */
  asked?: ReactNode;
  others: readonly HiddenChat[];
  onShowChat: (session: number) => void;
}) {
  // The window's choice, kept (`paneNoticesPref.ts`), handed to the band with the bell's ear.
  const minimised = useNoticesMinimised();
  const bell = useMemo(
    () =>
      onHolds === undefined
        ? undefined
        : {
            minimised,
            minimise: () => showNotices("minimised"),
            expand: () => showNotices("expanded"),
            onHolds,
            focus: () => onFocusBell?.(),
          },
    [minimised, onHolds, onFocusBell],
  );
  return (
    <NoticePaneRow band={band} bell={bell}>
      {notices}
      {asked}
      {others.map((other) => (
        <NoticeOfChat
          key={other.session}
          session={other.session}
          whose={other.whose}
          task={other.task}
          onShowChat={onShowChat}
        >
          {other.notices}
        </NoticeOfChat>
      ))}
    </NoticePaneRow>
  );
}

/** The Notices of one chat that is not on screen: each says whose it is and goes to it. */
export function NoticeOfChat({
  session,
  whose,
  task,
  onShowChat,
  children,
}: {
  session: number;
  whose: string;
  task: boolean;
  onShowChat: (session: number) => void;
  children: ReactNode;
}) {
  const of = useMemo(
    () => ({ whose, task, onGo: () => onShowChat(session) }),
    [onShowChat, session, task, whose],
  );
  return <NoticeOf.Provider value={of}>{children}</NoticeOf.Provider>;
}

/** No chat's head facts: a pane head then says what its pane line says without them. */
const NO_HEADS = (): ChatHead | undefined => undefined;
const NO_SHELLS: ReadonlySet<number> = new Set();

/** **What a folded pane's rail shows for what it holds** (#4766): its name and its icon, and
 *  whether that icon wears its chat's one state (#5447), its needs badge among them. */
export type PaneFace = { name: string; mark: ReactNode; stated?: boolean };

/** A pane's face where the window gives none: its chat's own name, a view's, or what an empty
 *  pane would do, each with its kind's mark. */
function faceByKind(content: Content, title: string): PaneFace {
  if (content.kind === "session")
    return { name: content.chat, mark: <SquareTerminal aria-hidden="true" /> };
  if (content.kind === "view") return { name: title, mark: <Puzzle aria-hidden="true" /> };
  return { name: NEW_CHAT, mark: <Plus aria-hidden="true" /> };
}

/** The panes of a folded side, as its rail draws them: each one's face, the notes its band
 *  last held, and whether the chat it shows asks the person now. */
function railOf(
  side: Layout,
  folded: Folded,
  plane: PlaneId,
  faceOf: ((content: Content) => PaneFace | undefined) | undefined,
  title: string,
  waiting: readonly Shown[] | undefined,
): RailPane[] {
  return panes(side)
    .filter(({ pane }) => folded.panes.includes(pane))
    .map(({ pane, content }) => ({
      pane,
      ...(faceOf?.(content) ?? faceByKind(content, title)),
      holds: keptHolds(paneBand(plane, pane)),
      asks:
        content.kind === "session" &&
        (waiting ?? []).some((ask) => ask.session === content.session),
    }));
}

/** A tab's layout, as panes with a handle between each split. */
export function LayoutPanes({
  plane,
  restoring = NONE_RESTORING,
  layout,
  crumbs,
  placed,
  endedTasks,
  endedChats,
  unread,
  finished,
  explaining,
  tasksOn,
  usingOn,
  others,
  asked,
  asks,
  waiting,
  onShowInbox,
  onAnswer,
  closeOf,
  alone = false,
  onShowChat,
  focused,
  onFocus,
  offerFor,
  offers,
  heads = NO_HEADS,
  shells = NO_SHELLS,
  onPaneDoes,
  name,
  handedFrom,
  glances,
  workItems,
  byHand,
  onByHand,
  startNotes,
  onDismissStartNote,
  onRestartChat,
  blocks,
  onDismissBlock,
  onRestarted,
  onAllowed,
  restartsSaid,
  onRestartAnswer,
  offered,
  onOpenView,
  onOpenViewBeside,
  onAsk,
  onVaultChanged,
  memory,
  firstTask,
  onNewChatOn,
  split,
  runningIn,
  noWorkspace = false,
  tab,
  chatName,
  onClearPane,
  onResumeTask,
  names,
  onResumeEnded,
  path = "",
  room,
  faceOf,
  onOpenFolded,
  drawn: given,
}: {
  /** The chats the launch has still to start (`restoring.rs`): each one's pane says so, in the
   *  place its terminal takes. */
  restoring?: ReadonlySet<number>;
  /** Which plane's sessions these panes are showing. A session number belongs to a plane,
   *  and every command a pane makes carries it. */
  plane: PlaneId;
  /** The tab's layout as it is drawn: each pane holding the chat it shows (#1486). */
  layout: Layout;
  /** The breadcrumb of each pane that shows a task, by pane (#1486). */
  crumbs: Readonly<Record<number, Crumbs>>;
  /** The breadcrumb of each pane that shows its own chat and still has a path to say (#1489):
   *  a task in a pane of its own, and the session it is beside. */
  placed: Readonly<Record<number, Crumbs>>;
  /** The session of the tab whose chip the first dispatch explains, on its own pane (#1501). */
  explaining?: ChipToExplain;
  /** **What the head's state word opens** (#5341): the chat's tasks menu, around the word, for
   *  a chat that has tasks; the word as it is for one that has none. */
  tasksOn?: TasksOn;
  /** What a chat uses now, on its pane head: its skill run and its browsers (#5524). */
  usingOn?: UsingOn;
  /** **The panes left on a task that has ended** (#4757), by pane: the path it last had. Each
   *  keeps the task's last screen, read-only, under the ended band (#4756). */
  endedTasks: Readonly<Record<number, Crumbs>>;
  /** **The chats ended on purpose in a pane** (#5019), by session: End…, a Smart close whose
   *  record is written, or a task's End…. Each keeps its pane, its last screen read-only under
   *  the ended band, named as it was, with Resume where it has a way back. */
  endedChats: ReadonlyMap<number, EndedChat>;
  /** The panes whose task is not known yet (the list of chats is not read): nothing is drawn. */
  unread: ReadonlySet<number>;
  /** The finished row of each task a pane was left on when it ended, by pane (#1485). */
  finished: Readonly<Record<number, FinishedTask>>;
  /** The chats that live in each pane other than the one it shows, by pane. */
  others: Readonly<Record<number, readonly { session: number; whose: string; task: boolean }[]>>;
  /** What each pane asks for several of its tasks at once, by pane (#1508). */
  asked: Readonly<Record<number, ReactNode>>;
  /** The permission prompts this project's chats hold open on their hooks, for the Notice of
   *  a chat off screen that is stopped on one. */
  asks: readonly Shown[];
  /** This project's asks, as the registry derived them last: what a chat's pane draws its
   *  asks from (#1695). Nothing before the first read. */
  waiting?: readonly Shown[];
  /** Opens this project's Inbox: where a chat's asks its pane has no room for wait. */
  onShowInbox?: () => void;
  /**
   * An answer was pressed on a chat's Notice in a pane: its words, and the ask it answered
   * (#1700). Held by the project, not the pane, so an answer is still listed once its ask goes
   * though the pane moved on (an Allow restarts the chat in a new session).
   */
  onAnswer?: (words: string, ask: Shown) => void;
  /** The close of pane `pane`, decided for that pane and not for the one in focus. */
  closeOf: (pane: number) => Offer | undefined;
  /** **The tab has this one pane** (#5318, `tabs.hasOnePane`): no Close on it, the tab's own ×
   *  doing the same. A pane zoomed to fill its tab, drawn alone, is not. */
  alone?: boolean;
  /** Goes to a chat: what a name in a breadcrumb does, with the pane it was named in, which
   *  shows it in place (#4757). */
  onShowChat: (session: number, pane?: number) => void;
  focused: number;
  onFocus: (pane: number) => void;
  /** The catalogue, by row id. There is one list of actions and the panes read it too. */
  offerFor: (id: string) => Offer | undefined;
  /** The same catalogue, whole: what a chat's ⋯ reads, whose Ask ▸ and Move to ▸ are scans
   *  of it (#4753). */
  offers: Catalogued;
  /** What each chat's pane head says of it (#4760), by session: none where the plane has not
   *  listed it. */
  heads?: (session: number) => ChatHead | undefined;
  /** The chats that run a shell, known from the open chats before the plane lists them. */
  shells?: ReadonlySet<number>;
  onPaneDoes: (pane: number, offer: Offer | undefined) => void;
  /** The tab's name, which is the title of the view it opened on. */
  name: string;
  /** Where each handed-off chat came from, by session (charter-app#258). */
  handedFrom: Readonly<Record<number, string>>;
  /** Each chat's harness card, at a glance, by session (HP-19). */
  glances: Readonly<Record<number, HarnessGlance>>;
  /** The work item each chat works on, by session (V60). */
  workItems: Readonly<Record<number, string>>;
  /** A harness started by hand in a shell tab, by session, for its banner (ADR 0062). */
  byHand: Readonly<Record<number, ByHandNote>>;
  /** The banner answered: `open` asks for that harness as a chat, else it is put away. */
  onByHand: (session: number, open: boolean) => void;
  /** What each chat's start found to say, by session (ADR 0085). */
  startNotes: Readonly<Record<number, StartNotes>>;
  onDismissStartNote: (session: number) => void;
  /** Starts chat `session` again, resuming its conversation, in its pane (#1362). */
  onRestartChat: (session: number) => void;
  /** What each chat's sandbox blocked, by session (#1338). */
  blocks: Blocks;
  onDismissBlock: (session: number, block: ChatBlocked) => void;
  /** A chat started again in its place from a sandbox block's Notice (#1342): `was` is its old
   *  session. */
  onRestarted: (was: number, chat: OpenChat) => void;
  /** Something was allowed for chat `session`: it is owed a restart (#1342). */
  onAllowed: (session: number) => void;
  /** What each chat's pane says of its restart, by session (#1342, #1428). */
  restartsSaid: Readonly<Record<number, RestartSaid>>;
  onRestartAnswer: (session: number, act: RestartAct) => void;
  /** The views approved extensions offer, for the buttons a view draws beside itself. */
  offered: readonly ExtensionView[];
  onOpenView: (view: ViewRef, title: string) => void;
  /** Opens a view beside pane `pane` (#4769), or says why not. */
  onOpenViewBeside?: (view: ViewRef, title: string, pane?: number) => void;
  /** The operator pressed to have a waiting view in this pane asked. */
  onAsk: (pane: number) => void;
  /** A vault's tab wrote to its vault: the plane's vault list is read again. */
  onVaultChanged: () => void;
  /** What charter's own views re-read on, and what a memory's tab calls when it wrote or asks
   *  to close (SI-9b). */
  memory: {
    changed: number;
    onSaved: (from: ViewRef, memory: MemoryView) => void;
    onClose: (view: ViewRef) => void;
    /** A view that now shows something else: a Search tab asking a new query (FM-8). */
    showInstead: (from: ViewRef, to: ViewRef, title: string) => void;
  };
  /** What the first task's tab asks the plane to do (FR-28). */
  firstTask: FirstTaskDoes;
  /** Asks for a new chat on pool `pool`, in workspace `workspace` where one is named: the picker,
   *  with the pool chosen on its Runs on row (#4483). */
  onNewChatOn?: (pool: string, workspace?: string) => void;
  /** Where a view's divider was, and where the operator moved it to (FM-2); and where a
   *  split's handle was dragged to, by the split's path in the tab (#4751). */
  split: {
    of: (view: ViewRef) => number | undefined;
    moved: (view: ViewRef, split: number) => void;
    shared?: (path: string, share: number) => void;
  };
  /** How many chats run in `workspace`: what an empty pane says (#4755). */
  runningIn: (workspace: string) => number;
  /** The plane has no workspace yet: an empty pane offers Create a workspace (#4758). */
  noWorkspace?: boolean;
  /** The tab this is the layout of. Its splits are its own: another tab's never carry over
   *  (#4755). */
  tab: number;
  /** A chat's name, as its row says it: what an ended chat's band names (#4756). */
  chatName: (session: number) => string;
  /** Clear pane: pane `pane` shows nothing, and its chat goes on in the sidebar (#4756). */
  onClearPane: (pane: number) => void;
  /** Resume, on the band of pane `pane` left on a task that ended: its finished row reopened
   *  in that pane (#1485, #4756). */
  onResumeTask: (pane: number, task: FinishedTask) => void;
  /** What a reader calls each pane of the tab, by where it is (#4767, `panePlaces.ts`): worked
   *  out once at the tab's root and handed down. */
  names?: ReadonlyMap<number, string>;
  /** Resume, on the band of chat `session` ended on purpose (#5019). */
  onResumeEnded: (session: number) => void;
  /** This layout's path from its tab's root: `a` for a split's first child, `b` for its
   *  second. The tab's own layout is `""`. */
  path?: string;
  /** The room the window gives the tab's panes, once it has been laid out: a split that cannot
   *  give each side its minimum folds the side without the focus to a rail (#4766). */
  room?: Size;
  /** What a folded pane's rail shows for what it holds: its name and its icon. A view is named
   *  as its pane is titled, and anything else this does not answer by its kind. */
  faceOf?: (content: Content) => PaneFace | undefined;
  /** A press on a folded pane: it takes the focus and the keyboard, since the rail's button
   *  goes as the other side folds. Without it, the pane only takes the focus (`onFocus`). */
  onOpenFolded?: (pane: number) => void;
  /** What the tab's root drew in `room` (`paneFold.ts`), handed down to each side. */
  drawn?: Drawn;
}) {
  // **Folding is decided here, as the tab is drawn, and never written to its tree** (#4766).
  const drawn = given ?? (room === undefined ? undefined : drawnOf(layout, focused, room));
  // **Whether the chat this pane draws has ended** (#4756): read for every layout, so the hooks
  // are the same whatever the pane shows, and false for all but a chat's.
  const shown = layout.kind === "pane" && layout.content.kind === "session" ? layout.content : null;
  const ended = useChatsSelect(
    useChatsHere(),
    (states) => shown !== null && endedBy(stateOf(states, shown.session)),
  );
  if (layout.kind === "pane") {
    const content = layout.content;
    if (content.kind === "empty")
      // **A pane that shows nothing** (#4755), with the pane's own actions where a view has
      // them, so it can be split or closed from where it is.
      return (
        <div
          className="pane-frame pane-viewed"
          data-focused={layout.pane === focused || undefined}
          onPointerDown={() => onFocus(layout.pane)}
          {...dropPane(tab, layout.pane)}
        >
          {/* An empty pane has nothing to pick up, so its head is no grip (#4583). */}
          <PaneActionsOf.Provider value={paneLinesOf("empty", undefined, { alone }).actions}>
            <PaneHead
              grip={false}
              doing={
                <PaneActions
                  pane={layout.pane}
                  size={drawn?.sizes[layout.pane]}
                  offerFor={offerFor}
                  closeOf={closeOf}
                  onPaneDoes={onPaneDoes}
                />
              }
            />
          </PaneActionsOf.Provider>
          <div
            className={layout.pane === focused ? "pane view focused" : "pane view"}
            onFocus={() => onFocus(layout.pane)}
          >
            <EmptyPane
              running={runningIn(content.workspace)}
              newChat={offerFor("chat.new")}
              newShell={offerFor("shell.new")}
              onPress={(offer) => onPaneDoes(layout.pane, offer)}
              createWorkspace={noWorkspace ? offerFor("workspace.create") : undefined}
            />
          </div>
        </div>
      );
    if (content.kind === "view") {
      // **A view in a pane, with the pane's own actions** (#3147): split and close are the same
      // rows a chat's pane has, drawn by the same `PaneHead` and `PaneActions` in the same place,
      // the pane's top right, so a view can be split to start a chat beside it and closed from
      // where it is. The view carries no title of its own: its tab is its name, and the content
      // panel is that tab's panel (`tabPanelOf`). Clicking anywhere in it focuses it, which is
      // what a terminal's click does.
      const viewActions = paneLinesOf("view", undefined, { alone }).actions;
      return (
        <div
          className="pane-frame pane-viewed"
          data-focused={layout.pane === focused || undefined}
          onPointerDown={() => onFocus(layout.pane)}
          {...dropPane(tab, layout.pane)}
        >
          {/* Before the view, for `PaneFrame`'s reason: the tab order is the document's. Over
              the view's first row, which keeps the room it takes (`paneFrame.css`). */}
          <PaneActionsOf.Provider value={viewActions}>
            <PaneHead
              // **A view's grip** (#5290): in a split, its mark at the head's start, as a chat's
              // pane has its persona's. Alone in its tab it has none, so nothing stands on its
              // own first row, and it moves by its tab.
              grip={!alone}
              lead={
                alone ? undefined : (
                  <span className="pane-view-grip">
                    <PaneGrip tab={tab} pane={layout.pane}>
                      <ViewMark view={content.view} />
                    </PaneGrip>
                  </span>
                )
              }
              // A view alone in its tab has none of them (#5318): its splits are the palette's and
              // its tab's menu's.
              doing={
                viewActions.length === 0 ? null : (
                  <PaneActions
                    pane={layout.pane}
                    size={drawn?.sizes[layout.pane]}
                    offerFor={offerFor}
                    closeOf={closeOf}
                    onPaneDoes={onPaneDoes}
                  />
                )
              }
            />
          </PaneActionsOf.Provider>
          <div
            className={layout.pane === focused ? "pane view focused" : "pane view"}
            onFocus={() => onFocus(layout.pane)}
          >
            <ViewPane
              plane={plane}
              view={content.view}
              title={name}
              workspace={content.workspace === OUTSIDE ? undefined : content.workspace}
              waits={content.waits === true}
              offered={offered}
              onOpenView={onOpenView}
              pane={layout.pane}
              onOpenViewBeside={onOpenViewBeside}
              onClosePane={() => onPaneDoes(layout.pane, closeOf(layout.pane))}
              onAsk={() => onAsk(layout.pane)}
              onVaultChanged={onVaultChanged}
              offerFor={offerFor}
              onPress={(offer) => onPaneDoes(layout.pane, offer)}
              changed={memory.changed}
              onMemorySaved={memory.onSaved}
              onCloseView={memory.onClose}
              onShowInstead={memory.showInstead}
              firstTask={firstTask}
              onNewChatOn={onNewChatOn}
              split={split.of(content.view)}
              onSplit={(to) => split.moved(content.view, to)}
            />
          </div>
        </div>
      );
    }
    /** What purlis has to say of chat `session`, on this pane. By the chat, so no Notice's
     *  state is carried from one chat to another when the tab is switched (#1486). */
    const noticesOf = (session: number) => (
      <ChatNotices
        key={session}
        plane={plane}
        session={session}
        byHand={byHand[session]}
        onByHand={(open) => onByHand(session, open)}
        startNotes={startNotes[session]}
        onDismissStartNote={() => onDismissStartNote(session)}
        onRestart={() => onRestartChat(session)}
        blocks={blocks[session]}
        onDismissBlock={(block) => onDismissBlock(session, block)}
        onRestarted={(chat) => onRestarted(session, chat)}
        onAllowed={() => onAllowed(session)}
        restartSaid={restartsSaid[session]}
        onRestartAnswer={(act) => onRestartAnswer(session, act)}
        asks={asks}
        waiting={waiting}
        onShowInbox={onShowInbox}
        onAnswer={onAnswer}
        pane={layout.pane}
      />
    );
    const hidden: HiddenChat[] = (others[layout.pane] ?? []).map((other) => ({
      ...other,
      notices: noticesOf(other.session),
    }));
    // **This pane shows a task of its tab's session** (#1486). The breadcrumb says where it
    // came from, so the note of the chat that started it is not said a second time.
    const crumb = crumbs[layout.pane];
    // **Or its own chat, with a path to say** (#1489): a task in a pane of its own, and the
    // session it is beside. The same line, so each side of a split says which chat it is.
    const path = crumb ?? placed[layout.pane];
    const head = heads(content.session);
    // A task the list of chats has not been read for yet: nothing is known of it, so nothing
    // is drawn (#1486).
    if (unread.has(layout.pane))
      return <div className="pane-frame" {...dropPane(tab, layout.pane)} />;
    // **A task this pane was left on that has ended** (#4757): its last screen stays, read-only,
    // under the ended band (#4756), and its path says how it ended, as its finished row says it
    // where it has one: one word for one end.
    const endedTask = endedTasks[layout.pane];
    const row = endedTask === undefined ? undefined : finished[layout.pane];
    const endedAs =
      endedTask === undefined
        ? undefined
        : row === undefined
          ? endedState(endedTask)
          : shownOf(row);
    const shownPath = endedTask ?? path;
    // **A chat ended on purpose in this pane** (#5019): the core closed it, so its band is drawn
    // from what the window held of it as it ended. A task the pane has a path for says it above.
    const endedChat = endedTask === undefined ? endedChats.get(content.session) : undefined;
    const endedRecord = endedChat?.record;
    const doing = (
      <PaneActions
        pane={layout.pane}
        size={drawn?.sizes[layout.pane]}
        offerFor={offerFor}
        closeOf={closeOf}
        onPaneDoes={onPaneDoes}
        // Its one state button and its ⋯: rows of the list of the chat it shows (#4753, #4760).
        // A task that has ended is read by its finished row's record (#1494), and has no Brief
        // of its own once that row is gone (#4757).
        state={
          <PaneStateButton
            session={content.session}
            task={head?.chat.mode === "task"}
            offers={offers}
            brief={
              endedTask !== undefined
                ? row === undefined
                  ? undefined
                  : { dispatch: row.id, name: row.name }
                : path === undefined
                  ? undefined
                  : briefOfShown(path)
            }
            ended={endedTask !== undefined}
            onPress={(offer) => onPaneDoes(layout.pane, offer)}
          />
        }
        chat={{ session: content.session, offers }}
      />
    );
    return (
      // What a chat's pane costs to draw, for a stall to name (`slowLog.ts`, H9): the one the
      // person is in apart from the others.
      <Measured id={layout.pane === focused ? "chat pane (focused)" : "chat pane"}>
        <PaneFrame
          plane={plane}
          tasksOn={tasksOn}
          usingOn={usingOn}
          pane={layout.pane}
          tab={tab}
          session={content.session}
          focused={layout.pane === focused}
          oneJob={isOneJob(content)}
          shell={shells.has(content.session)}
          // No close on a pane showing a task: nothing on screen ends the task, and the session's
          // close is its tab's (#1486). A task's own pane keeps its minimise whatever it shows
          // (#1489): switched to a task of its own, it is still the pane that is sent back.
          closes={
            (crumb === undefined && endedTask === undefined) ||
            closeOf(layout.pane)?.does.verb === "sendBack"
          }
          alone={alone}
          crumbs={shownPath}
          said={
            endedTask === undefined
              ? undefined
              : {
                  state: endedAs === undefined ? undefined : <StateShown shown={endedAs} />,
                  gone: (session) => !hidden.some((other) => other.session === session),
                }
          }
          // **What the head says of the chat, and its right-click** (#4760): the chat's list,
          // then this pane's rows, its close its own. A task's two ways to end are rows of its
          // list (`command-table.md`, rule 4), no longer buttons beside its path (#1488).
          head={head}
          headMenu={{
            offers,
            tab,
            pane: paneRowsOf(
              layout.pane,
              tab,
              offerFor,
              closeOf,
              crumb !== undefined || endedTask !== undefined,
              alone,
            ),
            onPress: (offer) => onPaneDoes(layout.pane, offer),
          }}
          onShowChat={onShowChat}
          from={
            // **A task's foot names the chat that asked** (#5205, story 22): its path's last but one.
            // A chat with a path of one, beside its task, says what any handed-off chat says.
            (shownPath && askedFromOf(shownPath)) ?? handedFrom[content.session]
          }
          harness={glances[content.session]}
          onOpenCard={(glance) => onOpenView(harnessCardView(glance.name), glance.label)}
          workItem={workItems[content.session]}
          // What the chat waits on the person for, whatever it writes after (#4563).
          strip={<WaitingStrip session={content.session} waiting={waiting} />}
          notices={
            <>
              {/* The prompt it is stopped on (#5215): one row, whose way goes to the terminal. */}
              <TerminalPromptNotice session={content.session} waiting={waiting} />
              {noticesOf(content.session)}
              {/* The first dispatch on this machine explains the chip, on the session's own
                pane (#1501). */}
              {shownPath === undefined && (
                <ChipExplained plane={plane} session={content.session} chip={explaining} />
              )}
            </>
          }
          asked={asked[layout.pane]}
          others={hidden}
          doing={doing}
          ended={
            endedTask !== undefined ? (
              // **A task's way back is its finished row's Reopen** (#1485), in this pane, where
              // the row says it can be reopened.
              <EndedBand
                session={content.session}
                name={endedTask.path[endedTask.path.length - 1].name}
                how={endedAs === undefined ? undefined : <StateShown shown={endedAs} />}
                more={row === undefined ? undefined : qualifierOf(row)}
                report={row?.report || undefined}
                resume={row?.reopens ? () => onResumeTask(layout.pane, row) : undefined}
                onClear={() => onClearPane(layout.pane)}
              />
            ) : endedChat !== undefined ? (
              <EndedBand
                session={content.session}
                name={endedChat.name}
                resume={endedChat.resumes ? () => onResumeEnded(content.session) : undefined}
                onOpenRecord={
                  endedRecord === undefined
                    ? undefined
                    : () =>
                        onOpenView(sessionView(endedRecord.path), sessionTitle(endedRecord.title))
                }
                onClear={() => onClearPane(layout.pane)}
              />
            ) : ended && !restoring.has(content.session) ? (
              <EndedBand
                session={content.session}
                name={chatName(content.session)}
                resume={resumeOf(offerFor(restartId(content.session)), (offer) =>
                  onPaneDoes(layout.pane, offer),
                )}
                onClear={() => onClearPane(layout.pane)}
              />
            ) : undefined
          }
        >
          {/* A chat the launch has still to start has no terminal yet (`restoring.rs`). */}
          {restoring.has(content.session) ? (
            <RestoringPane name={name} />
          ) : (
            <SessionPane
              plane={plane}
              session={content.session}
              focused={layout.pane === focused}
              onFocus={() => onFocus(layout.pane)}
              menu={{
                offers: paneMenuOf(offerFor, tab),
                tab,
                onPress: (offer) => onPaneDoes(layout.pane, offer),
              }}
              readOnly={ended || endedTask !== undefined || endedChat !== undefined}
            />
          )}
        </PaneFrame>
      </Measured>
    );
  }
  // **Each side starts at the share the tab holds** (#4751), and a drag is said back by the
  // split's path, so the record keeps it and a relaunch draws the split as it was. Only a drag
  // or a resize key is the person saying how big they want it: the library reports a layout
  // on mount too, which is a measurement purlis caused.
  const share = shareOf(layout);
  const sideOf = (side: number) => `${path}${side === 0 ? "a" : "b"}`;
  // **Each pane of a split is a region named by where it is** (#4767, ruled on #4699): "Left
  // pane", "Top right pane". What it holds says its chat's name and state.
  const named = names ?? paneNamesOf(layout);
  const regionOf = (child: Layout) => {
    const said = child.kind === "pane" ? named.get(child.pane) : undefined;
    return said === undefined
      ? {}
      : { role: "region", "aria-label": `${said[0].toUpperCase()}${said.slice(1)}` };
  };
  const settled = (sizes: Record<string, number>, meta: { isUserInteraction: boolean }) => {
    if (!meta.isUserInteraction) return;
    const first = sizes[`pane-${sideOf(0)}`];
    if (typeof first === "number") split.shared?.(path, Math.round(first * 10));
  };
  /** One side of this split, drawn whole: its own panes and splits. */
  const drawSide = (side: 0 | 1) => (
    <LayoutPanes
      path={sideOf(side)}
      plane={plane}
      restoring={restoring}
      layout={layout.children[side]}
      crumbs={crumbs}
      placed={placed}
      endedTasks={endedTasks}
      endedChats={endedChats}
      unread={unread}
      finished={finished}
      explaining={explaining}
      tasksOn={tasksOn}
      usingOn={usingOn}
      others={others}
      asked={asked}
      asks={asks}
      waiting={waiting}
      onShowInbox={onShowInbox}
      onAnswer={onAnswer}
      closeOf={closeOf}
      alone={alone}
      onShowChat={onShowChat}
      focused={focused}
      onFocus={onFocus}
      offerFor={offerFor}
      offers={offers}
      heads={heads}
      shells={shells}
      onPaneDoes={onPaneDoes}
      name={name}
      handedFrom={handedFrom}
      glances={glances}
      workItems={workItems}
      byHand={byHand}
      onByHand={onByHand}
      startNotes={startNotes}
      onDismissStartNote={onDismissStartNote}
      onRestartChat={onRestartChat}
      blocks={blocks}
      onDismissBlock={onDismissBlock}
      onRestarted={onRestarted}
      onAllowed={onAllowed}
      restartsSaid={restartsSaid}
      onRestartAnswer={onRestartAnswer}
      offered={offered}
      onOpenView={onOpenView}
      onOpenViewBeside={onOpenViewBeside}
      onAsk={onAsk}
      onVaultChanged={onVaultChanged}
      memory={memory}
      firstTask={firstTask}
      onNewChatOn={onNewChatOn}
      split={split}
      runningIn={runningIn}
      noWorkspace={noWorkspace}
      tab={tab}
      faceOf={faceOf}
      onOpenFolded={onOpenFolded}
      drawn={drawn}
      chatName={chatName}
      onClearPane={onClearPane}
      onResumeTask={onResumeTask}
      onResumeEnded={onResumeEnded}
      names={named}
    />
  );
  // **A split that cannot give each side its minimum** (#4766): the side without the focus is
  // a rail beside the other, or a bar above or below it, and the other takes the rest.
  const folded = drawn?.folds[path];
  if (folded !== undefined) {
    const rail = (
      <PaneRail
        as={folded.as}
        panes={railOf(layout.children[folded.side], folded, plane, faceOf, name, waiting)}
        onOpen={onOpenFolded ?? onFocus}
      />
    );
    return (
      <div className="pane-folds" data-direction={layout.direction}>
        {folded.side === 0 && rail}
        <div className="pane-folds-kept">{drawSide(folded.side === 0 ? 1 : 0)}</div>
        {folded.side === 1 && rail}
      </div>
    );
  }
  return (
    <Group
      // **A tab's sizes are its own** (#4755): the library reads a side's size once, as it
      // mounts, so a split drawn in this place for another tab is a new one, not that tab's
      // sizes carried over.
      key={`tab-${tab}`}
      orientation={layout.direction === "row" ? "horizontal" : "vertical"}
      onLayoutChanged={settled}
    >
      {layout.children.map((child, side) => (
        /* **Keyed by which side of the split it is, not by what is in it.**
         *
         * It used to be keyed by the panes underneath (`split-pane-3`), so a pane that
         * became a split changed its own key — React took the `Panel` out of a live `Group`
         * and put a new one back, and `react-resizable-panels` threw *"Panel constraints not
         * found for index 2"* from a document listener where no `try` can reach it. That is
         * the same hazard `docs/ui-primitives.md` records for the regions, and the same fix:
         * nothing is added to or removed from a live group.
         *
         * It was reachable before the panes got their own controls — click a pane that is
         * not the newest, then split from the bar or the palette — but it took three
         * deliberate steps and nobody had. A `+` on every pane makes it one press, which is
         * how it was found.
         *
         * A split has exactly two children and they never swap, so the side IS the identity.
         */
        <Fragment key={side}>
          {side === 1 && (
            <EvenedSeparator
              sides={[`pane-${sideOf(0)}`, `pane-${sideOf(1)}`]}
              share={share}
              onEven={() => split.shared?.(path, EVEN)}
            />
          )}
          <Panel
            id={`pane-${sideOf(side)}`}
            defaultSize={`${(side === 0 ? share : WHOLE - share) / 10}%`}
            // **A separator keeps the floor a drop keeps** (#4763): about 40 columns by 8 rows
            // for each pane on this side, in pixels.
            minSize={floorAlong(child, layout.direction)}
            {...regionOf(child)}
          >
            {drawSide(side as 0 | 1)}
          </Panel>
        </Fragment>
      ))}
    </Group>
  );
}

/**
 * **A split's separator, which evens the split** (#4583): a double-click on it, or Enter while
 * it has the keyboard, puts both sides at half, and says so to the tab (`onEven`). The arrow
 * keys resize it by 5%, which is the library's own step.
 */
export function EvenedSeparator({
  sides,
  share,
  onEven,
}: {
  sides: [string, string];
  /** What the tree holds as the first side's share: a drop can change it under a live group. */
  share?: number;
  onEven: () => void;
}) {
  const group = useParentGroup();
  // **A split draws the share its tree holds** (#5291): the library reads a side's size once, as
  // it mounts, and a drop that leaves other panes on the same two sides keeps its group, so the
  // window would draw the old sizes where the preview promised the tree's. A separator's own drag
  // says back what the library already draws.
  const shareDrawn = useRef(share);
  useLayoutEffect(() => {
    if (share === undefined || shareDrawn.current === share) return;
    shareDrawn.current = share;
    const first = share / 10;
    if (group !== null && Math.abs((group.getLayout()[sides[0]] ?? first) - first) > 0.1)
      group.setLayout({ [sides[0]]: first, [sides[1]]: (WHOLE - share) / 10 });
  }, [group, share, sides]);
  const even = () => {
    group?.setLayout({ [sides[0]]: EVEN / 10, [sides[1]]: (WHOLE - EVEN) / 10 });
    onEven();
  };
  return (
    <Separator
      className="pane-split"
      aria-label="Resize the split"
      // The library's own double-click puts a side back at its first size; this one evens it.
      disableDoubleClick
      onDoubleClick={even}
      onKeyDown={(event) => {
        if (event.key === "Enter") even();
      }}
    />
  );
}

/**
 * The controls in a pane's top right corner: split it two ways, and close it.
 *
 * **The operator's, in his own words**: *"harnesses panes should each have close button and
 * spliting buttons in pane right top corner … buttons should not have texts — only tooltips on
 * hovering — so this will fully replace separate buttons Split right, Split left, Exit this pane's
 * chat buttons, and this will be clear for spliting — user will know what pane is spliting."*
 *
 * **Always drawn** (#5318, the person's ask of 2026-11-05, ADR 0111): they were once drawn only
 * under the pointer or the keyboard. Nothing appears or moves as the pointer comes, and every
 * pane's are in the tab sequence. A tab's only pane has no close, the tab's own × doing the same.
 *
 * **Two things he did not say, which the rest of this repo does:**
 *
 * - **A tooltip is not an accessible name.** `title` is what a pointer gets and a screen
 *   reader may or may not read it; `aria-label` is what a keyboard and `pressOnly()` find.
 *   Both carry the catalogue's own words, so there is still one place they are written down.
 * - **The close ends nothing** (#4756): it clears the pane, asks nothing, and has the quiet
 *   hover a split has.
 *
 * Every button is a row of the catalogue, and the row is the same one the palette lists.
 * Nothing is drawn for a row the catalogue no longer has.
 */
export function PaneDoing({
  pane,
  offerFor,
  closeOf,
  onPaneDoes,
  size,
  parted = false,
}: {
  pane: number;
  /** The pane's size as drawn (`paneFold.ts`): Split right and Split down grey out, with their
   *  reason, where a half would be under the minimum (#4766). Not known, they split. */
  size?: Size;
  offerFor: (id: string) => Offer | undefined;
  /** This pane's close, which is not the focused pane's where the two differ (#1486): a pane
   *  beside one that shows a task still closes. For a task's own pane it is the minimise
   *  (#1489), which sends the task back and ends nothing. */
  closeOf: (pane: number) => Offer | undefined;
  onPaneDoes: (pane: number, offer: Offer | undefined) => void;
  /** **The chat's actions are drawn before these** (#5203): a thin divider parts the two
   *  groups. None on a view's pane, whose controls are all its head has. */
  parted?: boolean;
}) {
  // **What the pane's model answers** (#5206, `paneHead.ts`): a narrow pane drops Split down last
  // of its head's parts, and Split right and the close never go. A task's own tab and its pane
  // beside its session are rows of its ⋯ now (`command-table.md`, rule 4), not buttons here.
  const parts = useContext(PaneActionsOf);
  const rows = [
    ...(parts.includes("splitRight") ? ["pane.split.right"] : []),
    ...(parts.includes("splitDown") ? ["pane.split.down"] : []),
    ...(parts.includes("close") ? ["pane.close"] : []),
  ];
  return (
    <div className="pane-doing">
      {/* **The divider** (#5203): the first of the pane's controls, always drawn with them
          (#5318). It parts marks a screen reader already hears one by one, so it is not said. */}
      {parted && <Divider orientation="vertical" decorative className="pane-divider" />}
      {rows.map(draw)}
    </div>
  );
  function draw(id: string) {
    const offer = offerInRoom(id === "pane.close" ? closeOf(pane) : offerFor(id), size);
    if (!offer) return null;
    const Mark = markOf(offer);
    return (
      // The quiet ghost (#3214): a pane's corner marks, quiet until the pointer is on one.
      <IconButton
        key={id}
        variant="quiet"
        size="sm"
        icon={Mark ? <Mark /> : null}
        aria-label={offer.title}
        tooltip={offer.reason || offer.note || undefined}
        // A task's minimise, as its own tab draws it (#1489): never the close's look.
        data-minimise={offer.does.verb === "sendBack" ? "" : undefined}
        isDisabled={!offer.available}
        onPress={() => onPaneDoes(pane, offer)}
      />
    );
  }
}

/** What a chat's pane's band holds, for its bell (#4754): none outside a chat's pane, where no
 *  bell is drawn (a view's pane draws the same actions). */
export const PaneBellOf = createContext<BandHolds | null>(null);

/** The pane's bell itself, for the band's Dismiss all to hand the keyboard to (#4754). */
export const PaneBellAt = createContext<RefObject<HTMLButtonElement | null> | null>(null);

/**
 * **A pane's bell** (#4754, spec #4750 stories 33, 37 to 39): in the pane's header, in a fixed
 * slot before ⋯, there with no count when there is nothing, so nothing beside it
 * moves. An icon button the size of its neighbours (#5189), its hint below the head.
 *
 * - **Its count is the pane's notes**, never its asks, the same number minimised and expanded:
 *   the family's small pill on the bell's top corner, neutral, and hot while one note is
 *   trouble.
 * - **A press minimises or expands the notices of every pane** (`paneNotices.ts`); pressed, it
 *   is drawn as pressed.
 * - **A new notice never opens the band or takes the focus**: its count is said politely, once,
 *   as "5 notices, 1 in trouble, 2 asking".
 */
export function PaneBell() {
  const holds = useContext(PaneBellOf);
  const at = useContext(PaneBellAt);
  const minimised = useNoticesMinimised();
  // Said politely, at most once a second, and never the count a pane is first drawn with: each
  // change is new words, so a notice arriving is always heard (`ui/said.ts`).
  const spoken = useSaidAtMost(holds === null ? "" : bellSays(holds));
  if (holds === null) return null;
  const count = bellCount(holds);
  return (
    <>
      {/* The pill is unheard: the bell's name says the count (#5189). */}
      <span className="pane-bell-slot">
        <IconButton
          ref={at ?? undefined}
          size="sm"
          variant="quiet"
          className="pane-bell"
          icon={<Bell aria-hidden="true" />}
          aria-label={`Pane notices: ${bellSays(holds)}`}
          aria-expanded={!minimised}
          // The band's head row says Minimise too: the same act, the same word
          // (`docs/ui-copy.md`).
          tooltip={minimised ? "Show all" : "Minimise"}
          data-trouble={holds.trouble > 0 ? "" : undefined}
          data-empty={holds.notes === 0 ? "" : undefined}
          data-way="bell"
          onPress={() => showNotices(minimised ? "expanded" : "minimised")}
        />
        {count !== "" && (
          <Count
            aria-hidden="true"
            size="sm"
            className="pane-bell-count"
            variant={holds.trouble > 0 ? "hot" : "neutral"}
          >
            {count}
          </Count>
        )}
      </span>
      {/* Polite and on its own, never a Notice's status line. */}
      <span className="sr-only" aria-live="polite" aria-atomic="true">
        {spoken}
      </span>
    </>
  );
}
