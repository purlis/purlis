// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  Fragment,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { Group, Panel } from "react-resizable-panels";
import { Separator } from "./ui/panel/Separator";
import {
  type ChatBlocked,
  type HarnessGlance,
  type FinishedTask,
  type OpenChat,
  type PlaneId,
  type MemoryView,
} from "./bindings";
import {
  besideId,
  menuOn,
  ownTabId,
  OUTSIDE,
  taskCloseId,
  taskStopId,
  workItemSaid,
  type Catalogued,
  type Offer,
} from "./actions";
import { Menued } from "./Menus";
import { RestoringPane } from "./RestoringPane";
import { NONE_RESTORING } from "./restoringChats";
import { SessionPane } from "./SessionPane";
import { droppedReference, overChat, useReferenceChats } from "./references";
import { NoticeOf, NoticePaneRow } from "./Notice";
import { type Blocks } from "./sandboxBlocks";
import { ChatGauge, useChatUsage } from "./ChatGauge";
import { SkillsChip } from "./UseSkill";
import { harnessCardView, type Layout, type ViewRef } from "./tabs";
import { type Crumbs } from "./tabChats";
import { BriefButton, type BriefAsk } from "./Brief";
import { PaneCrumbs } from "./PaneCrumbs";
import { ChipExplained, type ChipToExplain } from "./ChipExplained";
import { TaskEnds } from "./TaskEnd";
import { giveKeyboardTo } from "./paneKeyboard";
import { endedState, TaskAway, type Away } from "./TaskAway";
import { StateShown } from "./StateShown";
import { ViewPane } from "./Views";
import type { FirstTaskDoes } from "./FirstTaskTab";
import type { ExtensionView, Shown } from "./bindings";
import { qualifierOf, shownOf } from "./finished";
import { HarnessChip } from "./HarnessCard";
import { movedAt, stateOf, useChatsHere, useChatsSelect } from "./chatState";
import { closesOnly, markOf } from "./StripMarks";
import {
  type RestartAct,
  type RestartSaid,
  type ByHandNote,
  type StartNotes,
  ChatNotices,
} from "./ChatNotices";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **The panes of the tab in front** (#1035, split out of `PlaneView.tsx`): the split layout
 * (`LayoutPanes`), each pane's frame and breadcrumb (`PaneFrame`), the Notices that belong to
 * a pane, and the controls in a pane's top corner (`PaneDoing`).
 */

/**
 * **The rows of a task's menu in the Chats list, for a pane to open the same menu on its
 * breadcrumb** (#1489): the catalogue's own rows, picked out by the ids that menu lists
 * (`actions.menuOn`), so the two cannot differ.
 */
export function listedMenuOf(
  session: number,
  offerFor: (id: string) => Offer | undefined,
): Catalogued {
  const { above, below } = menuOn({ on: "listed", session });
  return new Map(
    [...above, ...below].flatMap((id) => {
      const offer = offerFor(id);
      return offer === undefined ? [] : [[id, offer] as const];
    }),
  );
}

/**
 * **The rows of the panes' menu, for a terminal to list under its own** (`TerminalMenu.tsx`):
 * a right-click on a terminal opens the terminal's menu, and what the panes' menu offered
 * there before — a new tab, the splits, ending the chat — is the same catalogue rows, picked
 * out by the ids that menu lists (`actions.menuOn`).
 */
export function paneMenuOf(offerFor: (id: string) => Offer | undefined): Catalogued {
  const { above, below } = menuOn({ on: "pane" });
  return new Map(
    [...above, ...below].flatMap((id) => {
      const offer = offerFor(id);
      return offer === undefined ? [] : [[id, offer] as const];
    }),
  );
}

/**
 * One pane's frame: what purlis has to say of its chats, in a row of its own at the top
 * (`PaneNotices`, #1647), and under it the terminal, with what charter draws over it in the
 * pane's two corners — side by side with the terminal, so neither is ever a child of the
 * element xterm draws into.
 *
 * **Two corners, and neither thing in them places itself** (charter-app#193). The gauge is
 * top-left and the controls top-right — the operator: *"context status indicator in pane right
 * corner can be moved to left corner. to not make split and close buttons uggly"*. They shared
 * one row in the right corner before that, because each had been written as the thing in the
 * pane's top-right. A corner positions; its contents do not, so a third thing arriving here
 * collides in review rather than at runtime. The controls keep their hover rule; the gauge,
 * always drawn, is outside it.
 *
 * **Both corners float over the terminal and neither takes a row.** #207 gave the gauge a row
 * of its own, and a pane with a gauge was then a row shorter than one without — the operator:
 * *"its changing harness container sizes"*. A pane's size is the layout's business alone.
 *
 * **The Notices are the exception, and take a row** (#1647): they were in the start corner
 * (#1481), and three of them hid the conversation. A chip is one short line; a Notice is a
 * sentence and its answers, and what it covers is what the person has to read to answer it.
 */
export function PaneFrame({
  plane,
  session,
  crumbs,
  ending,
  crumbMenu,
  onShowChat,
  from,
  harness,
  onOpenCard,
  workItem,
  notices,
  asked,
  others,
  doing,
  focused = false,
  children,
}: {
  plane: PlaneId;
  session: number;
  /** Whether the keyboard is in this pane: a split says which of its panes has it (#2914). */
  focused?: boolean;
  /** While the pane shows a task of its tab's session: the path to it (#1486). Also for a task
   *  in a pane of its own, and the session it is beside (#1489). */
  crumbs?: Crumbs;
  /** While it does: the two ways to end that task, beside the path (#1488). The only ending
   *  control a pane showing a task has. */
  ending?: ReactNode;
  /** The menu a right-click on a task's breadcrumb opens: its row's in the Chats list. */
  crumbMenu?: { session: number; offers: Catalogued; onPress: (offer: Offer) => void };
  /** Goes to a chat: what a name in the breadcrumb does, and a hidden chat's Notice. */
  onShowChat: (session: number) => void;
  /** Where a handed-off chat came from, `↳ from steward 3 · ops`, in the chat's own corner. */
  from?: string;
  /** The harness the chat runs, at a glance, in the same corner (HP-19): none for a shell. */
  harness?: HarnessGlance;
  /** Opens that harness's card tab. */
  onOpenCard: (glance: HarnessGlance) => void;
  /** The work item this chat works on, `Work item: <key>`, in the same corner (V60). */
  workItem?: string;
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
  const usage = useChatUsage(plane, session, moved, running);
  const lent = useReferenceChats();
  const frame = useRef<HTMLDivElement>(null);
  const roomy = useRoomy(frame);
  return (
    <div
      ref={frame}
      className="pane-frame"
      data-focused={focused || undefined}
      // A reference dropped on the chat's pane is typed into it, unsent (FM-9).
      onDragOver={overChat}
      onDrop={(event) => {
        const r = droppedReference(event);
        if (r === undefined || lent === undefined) return;
        event.preventDefault();
        lent.hand(r, session, "add");
      }}
    >
      {/* **The Notices first**: they are the top of the pane, and the first thing read and
          tabbed to (#1647). */}
      <PaneNotices notices={notices} asked={asked} others={others} onShowChat={onShowChat} />
      <div className="pane-body">
        {/* **The corners before the terminal, in the document**, although they are drawn over
          it: the tab order is the document's, and a terminal keeps Tab for its shell, so a
          control written after it is one Tab never reaches (charter-app#189). The pane's own
          controls are in its top corner, which is where reading order puts them anyway. */}
        <div className="pane-corner at-start">
          {/* The chat at a glance, on one row. */}
          <div className="pane-chips">
            {/* **Which chat this is, while the tab shows a task** (#1486): first in the line, and
              in no row of its own. */}
            {crumbs &&
              (crumbMenu === undefined ? (
                <PaneCrumbs crumbs={crumbs} onShow={onShowChat} />
              ) : (
                <Menued
                  on={{ on: "listed", session: crumbMenu.session }}
                  offers={crumbMenu.offers}
                  onPress={crumbMenu.onPress}
                >
                  <PaneCrumbs crumbs={crumbs} onShow={onShowChat} />
                </Menued>
              ))}
            {/* **Brief** (#1494): what the task on screen was sent. Right after the breadcrumb,
              before anything else the line holds for a task. */}
            {crumbs && <BriefButton of={briefOfShown(crumbs)} />}
            {crumbs && ending}
            <ChatGauge usage={usage} />
            {/* The chat's skills and its skill budget, and Use skill… for it (#2775). */}
            {/* Not beside a task's breadcrumb: that line is the fullest a pane has, and the
                state in it is read whole before anything else (`pane-crumbs.e2e.ts`); nor in a
                narrow pane, for the same line. The palette and the tab's menu still have Use
                skill… there. */}
            {!crumbs && roomy && <SkillsChip session={session} />}
            {harness && <HarnessChip glance={harness} onOpen={() => onOpenCard(harness)} />}
            {from && <span className="pane-from">{from}</span>}
            {workItem && <span className="pane-work-item">{workItemSaid(workItem)}</span>}
          </div>
        </div>
        <div className="pane-corner at-end">{doing}</div>
        {children}
      </div>
    </div>
  );
}

/** The widest a pane may be and still be narrow (#2775): 40rem, past which its top line has
 *  room for the Skills chip beside everything else it holds. */
const NARROW_REM = 40;

/** Whether the pane frame in `frame` is wider than {@link NARROW_REM}, as it is resized: true
 *  where it cannot be measured, as in a test with no layout. */
function useRoomy(frame: RefObject<HTMLDivElement | null>): boolean {
  const [roomy, setRoomy] = useState(true);
  useLayoutEffect(() => {
    const element = frame.current;
    if (element === null || typeof ResizeObserver === "undefined") return;
    const measure = () => {
      const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
      setRoomy(element.getBoundingClientRect().width > NARROW_REM * rem);
    };
    measure();
    const watching = new ResizeObserver(measure);
    watching.observe(element);
    return () => watching.disconnect();
  }, [frame]);
  return roomy;
}

/** The task a breadcrumb's pane shows, as the Brief panel is asked for it: the last chat of
 *  the path, by its number. */
export function briefOfShown(crumbs: Crumbs): BriefAsk {
  const shown = crumbs.path[crumbs.path.length - 1];
  return { chat: shown.session, name: shown.name };
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
 * **What purlis has to say on a pane, one Notice under another** (#1481), in the pane's own row
 * above its terminal, two at a time and the rest behind "+N more" (`NoticePaneRow`, #1647).
 *
 * **Of every chat that lives in the pane, whichever one it shows** (#1486, the train-1 part of
 * V100-56). A session's tab shows one of its chats at a time: the session's own, or a task
 * below it. A Notice that waits for the person belongs to one of them, and the other may be on
 * screen. So the shown chat's Notices come first, as they always read, and after them every
 * other chat's, each saying whose it is and with **Go to it** (`NoticeOf`). An answer on one of
 * those acts on its own chat: the Notice is that chat's, drawn here.
 */
export function PaneNotices({
  notices,
  asked,
  others,
  onShowChat,
}: {
  notices: ReactNode;
  /** The questions for several of the pane's tasks at once (#1508): after the shown chat's
   *  own, before each other chat's, since each waits for the person. */
  asked?: ReactNode;
  others: readonly HiddenChat[];
  onShowChat: (session: number) => void;
}) {
  return (
    <NoticePaneRow>
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

/** A tab's layout, as panes with a handle between each split. */
export function LayoutPanes({
  plane,
  restoring = NONE_RESTORING,
  layout,
  crumbs,
  placed,
  away,
  finished,
  explaining,
  others,
  asked,
  asks,
  waiting,
  onShowInbox,
  onAnswer,
  closeOf,
  onBack,
  onShowChat,
  focused,
  onFocus,
  offerFor,
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
  onAsk,
  onVaultChanged,
  memory,
  firstTask,
  split,
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
  /** The panes that show a task they cannot draw, by pane, and why (`TaskAway`). */
  away: Readonly<Record<number, Away>>;
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
  /** Pane `pane` goes back to its session's own chat. */
  onBack: (pane: number) => void;
  /** Goes to a chat: what a name in a breadcrumb does. */
  onShowChat: (session: number) => void;
  focused: number;
  onFocus: (pane: number) => void;
  /** The catalogue, by row id. There is one list of actions and the panes read it too. */
  offerFor: (id: string) => Offer | undefined;
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
  /** Where a view's divider was, and where the operator moved it to (FM-2). */
  split: {
    of: (view: ViewRef) => number | undefined;
    moved: (view: ViewRef, split: number) => void;
  };
}) {
  if (layout.kind === "pane") {
    const content = layout.content;
    if (content.kind === "view") {
      // **A view in a pane, with the pane's own corner** — split and close are the same rows a
      // chat's pane has, so a view can be split to start a chat beside it and closed from where
      // it is. Clicking anywhere in it focuses it, which is what a terminal's click does.
      return (
        <div
          className="pane-frame"
          data-focused={layout.pane === focused || undefined}
          onPointerDown={() => onFocus(layout.pane)}
        >
          {/* Before the view, for `PaneFrame`'s reason: the tab order is the document's. */}
          <div className="pane-corner at-end">
            <PaneDoing
              pane={layout.pane}
              offerFor={offerFor}
              closeOf={closeOf}
              onPaneDoes={onPaneDoes}
            />
          </div>
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
              onAsk={() => onAsk(layout.pane)}
              onVaultChanged={onVaultChanged}
              offerFor={offerFor}
              onPress={(offer) => onPaneDoes(layout.pane, offer)}
              changed={memory.changed}
              onMemorySaved={memory.onSaved}
              onCloseView={memory.onClose}
              onShowInstead={memory.showInstead}
              firstTask={firstTask}
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
    const doing = (
      <PaneDoing
        pane={layout.pane}
        offerFor={offerFor}
        closeOf={closeOf}
        onPaneDoes={onPaneDoes}
        // No close on a pane showing a task: nothing on screen ends the task, and the
        // session's close is its tab's (#1486).
        task={crumb !== undefined || away[layout.pane] !== undefined}
        // The task it shows inside its session's tab, which can be given a tab of its own or
        // opened beside its session from here (#1489).
        shows={crumb === undefined ? undefined : content.session}
      />
    );
    const gone = away[layout.pane];
    if (gone !== undefined) {
      // **A task this pane shows and cannot draw** (#1486): no terminal, so nothing typed
      // reaches a chat, and the pane says what it shows and why, with the way back.
      if (gone.why === "unread") return <div className="pane-frame" />;
      // How it ended, as its finished row says it where it has one: one word for one end.
      const row = finished[layout.pane];
      const ended = row === undefined ? endedState(gone.crumbs) : shownOf(row);
      const last = gone.crumbs.path[gone.crumbs.path.length - 1].session;
      return (
        <div
          className="pane-frame"
          data-focused={layout.pane === focused || undefined}
          onPointerDown={() => onFocus(layout.pane)}
        >
          {/* As `PaneFrame` draws them: a row of their own, above what the pane shows. */}
          <PaneNotices
            notices={null}
            asked={asked[layout.pane]}
            others={hidden}
            onShowChat={onShowChat}
          />
          <div className="pane-body">
            <div className="pane-corner at-start">
              <div className="pane-chips">
                <PaneCrumbs
                  crumbs={gone.crumbs}
                  onShow={onShowChat}
                  // One that is still running says what it is doing; one that ended, how.
                  state={
                    gone.why === "ended" && ended !== undefined ? (
                      <StateShown shown={ended} />
                    ) : undefined
                  }
                  gone={(session) => !hidden.some((other) => other.session === session)}
                />
                {/* A task that has ended is read by its finished row's record (#1494), and has
                  no Brief here once that row is gone; one still running, by its chat. */}
                {(gone.why !== "ended" || row !== undefined) && (
                  <BriefButton
                    of={
                      gone.why === "ended" && row !== undefined
                        ? { dispatch: row.id, name: row.name }
                        : briefOfShown(gone.crumbs)
                    }
                  />
                )}
              </div>
            </div>
            <div className="pane-corner at-end">{doing}</div>
            <TaskAway
              away={gone}
              focused={layout.pane === focused}
              report={row?.report}
              finished={row && { state: ended, more: qualifierOf(row) }}
              onBack={() => onBack(layout.pane)}
              onOpenAlone={() => onShowChat(last)}
            />
          </div>
        </div>
      );
    }
    return (
      <PaneFrame
        plane={plane}
        session={content.session}
        focused={layout.pane === focused}
        crumbs={path}
        // **The two ways to end the task this pane shows**, beside its path (#1488): on a
        // session's pane switched to a task, and on a task's own pane, in a tab of its own or
        // beside its session (#1489). Never on the session's own one-name path beside a task.
        ending={
          path !== undefined &&
          path.path[path.path.length - 1].mode === "task" && (
            <TaskEnds
              session={path.path[path.path.length - 1].session}
              stop={offerFor(taskStopId(path.path[path.path.length - 1].session))}
              close={offerFor(taskCloseId(path.path[path.path.length - 1].session))}
              // Keep gives the keyboard back to the task's terminal.
              focusBack={() => giveKeyboardTo(plane, content.session)}
            />
          )
        }
        // Right-click on a task's breadcrumb is its row's menu in the Chats list (#1489).
        crumbMenu={
          path !== undefined && path.path[path.path.length - 1].mode === "task"
            ? {
                session: path.path[path.path.length - 1].session,
                offers: listedMenuOf(path.path[path.path.length - 1].session, offerFor),
                onPress: (offer) => onPaneDoes(layout.pane, offer),
              }
            : undefined
        }
        onShowChat={onShowChat}
        from={path === undefined ? handedFrom[content.session] : undefined}
        harness={glances[content.session]}
        onOpenCard={(glance) => onOpenView(harnessCardView(glance.name), glance.label)}
        workItem={workItems[content.session]}
        notices={
          <>
            {noticesOf(content.session)}
            {/* The first dispatch on this machine explains the chip, on the session's own
                pane (#1501). */}
            {path === undefined && (
              <ChipExplained plane={plane} session={content.session} chip={explaining} />
            )}
          </>
        }
        asked={asked[layout.pane]}
        others={hidden}
        doing={doing}
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
              offers: paneMenuOf(offerFor),
              onPress: (offer) => onPaneDoes(layout.pane, offer),
            }}
          />
        )}
      </PaneFrame>
    );
  }
  return (
    <Group orientation={layout.direction === "row" ? "horizontal" : "vertical"}>
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
          {side === 1 && <Separator aria-label="Resize the split" />}
          <Panel>
            <LayoutPanes
              plane={plane}
              restoring={restoring}
              layout={child}
              crumbs={crumbs}
              placed={placed}
              away={away}
              finished={finished}
              explaining={explaining}
              others={others}
              asked={asked}
              asks={asks}
              waiting={waiting}
              onShowInbox={onShowInbox}
              onAnswer={onAnswer}
              closeOf={closeOf}
              onBack={onBack}
              onShowChat={onShowChat}
              focused={focused}
              onFocus={onFocus}
              offerFor={offerFor}
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
              onAsk={onAsk}
              onVaultChanged={onVaultChanged}
              memory={memory}
              firstTask={firstTask}
              split={split}
            />
          </Panel>
        </Fragment>
      ))}
    </Group>
  );
}

/**
 * The controls in a pane's top right corner: split it two ways, and end its chat.
 *
 * **The operator's, in his own words**: *"harnesses panes should each have close button and
 * spliting buttons in pane right top corner — and its visible when hovering harness only …
 * buttons should not have texts — only tooltips on hovering — so this will fully replace
 * separate buttons Split right, Split left, Exit this pane's chat buttons, and this will be
 * clear for spliting — user will know what pane is spliting."*
 *
 * **Three things he did not say, which the rest of this repo does:**
 *
 * - **A tooltip is not an accessible name.** `title` is what a pointer gets and a screen
 *   reader may or may not read it; `aria-label` is what a keyboard and `pressOnly()` find.
 *   Both carry the catalogue's own words, so there is still one place they are written down.
 * - **Hover-only is invisible without a pointer**, so these are drawn for the FOCUSED pane as
 *   well as the hovered one. `App.css` has the rule and the reason it is `visibility` rather
 *   than `opacity`: an invisible button that can still be clicked is `End this pane's chat`
 *   under a stray press.
 * - **The close is the danger colour**, as the tab's `×` and the bar's button were, because
 *   it does the same thing. It asks first now (`EndingChat`), which is new and is not a
 *   licence for it to look lighter.
 *
 * Every button is a row of the catalogue, and the row is the same one the palette lists.
 * Nothing is drawn for a row the catalogue no longer has.
 */
export function PaneDoing({
  pane,
  offerFor,
  closeOf,
  onPaneDoes,
  task = false,
  shows,
}: {
  pane: number;
  offerFor: (id: string) => Offer | undefined;
  /** This pane's close, which is not the focused pane's where the two differ (#1486): a pane
   *  beside one that shows a task still closes. For a task's own pane it is the minimise
   *  (#1489), which sends the task back and ends nothing. */
  closeOf: (pane: number) => Offer | undefined;
  onPaneDoes: (pane: number, offer: Offer | undefined) => void;
  /** The pane shows a task of its tab's session (#1486): it draws no close. A close here
   *  would read as ending the task, and what it would end is the session under it. */
  task?: boolean;
  /** That task, where the pane can draw it: the pane offers it a tab of its own, and a pane
   *  beside its session (#1489). The task's own rows, not the focused pane's. */
  shows?: number;
}) {
  // **A task's own pane keeps its minimise whatever it shows** (#1489): switched to a task of
  // its own, it is still the pane that is sent back. Only a close is not drawn over a task.
  const minimise = closeOf(pane)?.does.verb === "sendBack";
  const rows = [
    ...(shows === undefined ? [] : [ownTabId(shows), besideId(shows)]),
    "pane.split.right",
    "pane.split.down",
    ...(task && !minimise ? [] : ["pane.close"]),
  ];
  // **The pane's top line ends before these controls, whatever their number** (#1486): their
  // width is measured and said to the pane's frame (`--pane-controls`, `App.css`), so the line
  // reserves the room the controls drawn here take, not a count somebody remembered.
  const at = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const controls = at.current;
    const frame = controls?.closest<HTMLElement>(".pane-frame");
    if (!controls || !frame) return;
    const say = () => {
      const width = controls.offsetWidth;
      // Nothing is laid out (a test's document): the stylesheet's own figure stands.
      if (width > 0) frame.style.setProperty("--pane-controls", `${width}px`);
    };
    say();
    if (typeof ResizeObserver === "undefined") return;
    const watching = new ResizeObserver(say);
    watching.observe(controls);
    return () => watching.disconnect();
  }, [rows.length]);
  return (
    <div className="pane-doing" ref={at}>
      {rows.map((id) => {
        const offer = id === "pane.close" ? closeOf(pane) : offerFor(id);
        if (!offer) return null;
        const Mark = markOf(offer);
        return (
          <Tooltip
            key={id}
            tip={offer.reason || (offer.note ? `${offer.title} — ${offer.note}` : offer.title)}
          >
            <button
              className={
                offer.id === "pane.close" && !closesOnly(offer.does) ? "ends-a-chat" : undefined
              }
              // A task's minimise, as its own tab draws it (#1489): never the close's look.
              data-minimise={offer.does.verb === "sendBack" ? "" : undefined}
              // WebKit leaves a `<button>` out of the tab sequence unless this is written down
              // (`docs/ui-primitives.md`). Only the focused pane's are drawn (`App.css`), so only
              // they are stops: a hidden control is not one.
              tabIndex={0}
              disabled={!offer.available}
              aria-label={offer.title}
              onClick={() => onPaneDoes(pane, offer)}
            >
              {Mark && <Mark />}
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
}
