// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  Fragment,
  useCallback,
  useContext,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { Group, Panel, useParentGroup } from "react-resizable-panels";
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
import { WaitingStrip } from "./WaitingStrip";
import { paneBand } from "./noticeSettle";
import { type Blocks } from "./sandboxBlocks";
import { ChatGauge, useChatUsage } from "./ChatGauge";
import { SkillsChip } from "./UseSkill";
import { harnessCardView, isOneJob, type Layout, type ViewRef } from "./tabs";
import { EVEN, shareOf, WHOLE } from "./laidOut";
import { floorAlong } from "./paneDrop";
import { dropPane, GRIP } from "./paneDrag";
import { paneNamesOf } from "./panePlaces";
import { type Crumbs } from "./tabChats";
import { BriefButton, type BriefAsk } from "./Brief";
import { PaneCrumbs } from "./PaneCrumbs";
import { ChipExplained, type ChipToExplain } from "./ChipExplained";
import { TaskEndAsked } from "./TaskEnd";
import { SAID } from "./menuWords";
import { giveKeyboardTo } from "./paneKeyboard";
import { StateShown } from "./StateShown";
import { ViewPane } from "./Views";
import type { FirstTaskDoes } from "./FirstTaskTab";
import type { ExtensionView, Shown } from "./bindings";
import { qualifierOf, shownOf } from "./finished";
import { HarnessChip } from "./HarnessCard";
import { movedAt, stateOf, useChatsHere, useChatsSelect } from "./chatState";
import { EmptyPane } from "./EmptyPane";
import { EndedBand, endedBy, endedState, resumeOf } from "./EndedBand";
import { markOf } from "./StripMarks";
import {
  type RestartAct,
  type RestartSaid,
  type ByHandNote,
  type StartNotes,
  ChatNotices,
} from "./ChatNotices";
import { Button, IconButton } from "./ui/button/Button";
import { Bell, Reply, ScrollText, Square, Undo2 } from "lucide-react";
import { droppedAt, stateButtonOf, type ChatHead, type HeadPart } from "./paneHead";
import { PaneTerminalAt, type PaneTerminal } from "./paneTerminal";
import { stemOf } from "./menuModel";
import { ChatShownState } from "./ChatRows";
import { ChatPersonaState } from "./ChatPersonaState";
import { SandboxMark } from "./SandboxMark";
import { WrappingUp } from "./NeedsYou";
import { bellCount, bellSays, HOLDS_NOTHING, type BandHolds } from "./paneNotices";
import { showNotices, useNoticesMinimised } from "./paneNoticesPref";
import { useSaidAtMost } from "./ui/said";

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
): Catalogued {
  const close = closeOf(pane);
  const drawn = !task || close?.does.verb === "sendBack";
  return paneMenuOf(
    (id) => (id === "pane.close" ? (drawn ? close : undefined) : offerFor(id)),
    tab,
  );
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
 * - **The header is always there**, with a gauge or without, so a pane does not change height
 *   when its first usage arrives (the operator's reason in #207). Only the band comes and goes,
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
  focused = false,
  oneJob = false,
  children,
}: {
  plane: PlaneId;
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
  /** While the pane shows a task of its tab's session: the path to it (#1486). Also for a task
   *  in a pane of its own, and the session it is beside (#1489). */
  crumbs?: Crumbs;
  /** What the head says of the chat (#4760): its name, state, "harness · workspace", its
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
  /** The harness the chat runs, at a glance, in the same corner (HP-19): none for a shell. */
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
  const usage = useChatUsage(plane, session, moved, running);
  const lent = useReferenceChats();
  const frame = useRef<HTMLDivElement>(null);
  // **What a narrow pane drops, in the ruled order** (#4760, `paneHead.ts`), and whether the
  // line has room for the Skills chip too (#2775).
  const width = usePaneWidth(frame);
  const drops = droppedAt(width);
  const roomy = width === undefined || width > NARROW_REM * remPx();
  /** What the band holds, for the bell in the pane's header (#4754), and the bell itself, which
   *  the band's Dismiss all hands the keyboard to once it has cleared the band. */
  const [holds, setHolds] = useState<BandHolds>(HOLDS_NOTHING);
  const bellAt = useRef<HTMLButtonElement>(null);
  /** The terminal the pane draws, for Copy and Paste on its head's right-click (#4760). */
  const terminalAt = useRef<PaneTerminal | null>(null);
  const focusBell = useCallback(() => bellAt.current?.focus(), []);
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
        <PaneDropsOf.Provider value={drops}>
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
              and its tab wear, first on the head. */}
              {head && (
                <span className="pane-persona">
                  <ChatPersonaState
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
                head && <span className="pane-name">{head.chat.name}</span>
              )}
              {/* A task's end, or its restart, asked on its line from its ⋯ (#1488). */}
              {crumbs && (
                <TaskEndAsked
                  session={crumbs.path[crumbs.path.length - 1].session}
                  focusBack={() => giveKeyboardTo(plane, session)}
                />
              )}
              {/* **Its sandbox** (#4760): moved from the tab, which no longer is the chat. It says
              how the chat runs, as "harness · workspace" does, and a narrow pane drops it with
              that, so a task's path and the ways to end it keep their room. */}
              {head && !drops.has("where") && (
                <SandboxMark
                  level={head.sandbox?.level}
                  why={head.sandbox?.why}
                  sites={head.sandbox?.sites}
                />
              )}
              {/* **Its state**, as its row says it: the mark and its word, the word going
              first in a narrow pane and then the mark (`paneHead.ts`). */}
              {head && !crumbs && !drops.has("mark") && (
                <span className="pane-state-mark">
                  <ChatShownState
                    session={session}
                    shell={head.chat.shell}
                    report={head.chat.report}
                    outcome={head.chat.outcome}
                    harness={head.chat.harness}
                    markOnly={drops.has("markWord")}
                  />
                </span>
              )}
              {/* **Wrapping up** (ADR 0064, #4760): the breathing mark, where the strip's chip
              was. */}
              {head?.wrapping === true && (
                <span className="pane-wrapping">
                  <WrappingUp held />
                </span>
              )}
              {!drops.has("where") && (
                <>
                  {/* **harness · workspace** (#4760): the harness opens its card (HP-19). */}
                  {head && (head.chat.workspace !== "" || harnessWord(head, harness) !== "") && (
                    <span className="pane-where">
                      {harness ? (
                        <HarnessChip glance={harness} onOpen={() => onOpenCard(harness)} />
                      ) : (
                        head.chat.harness
                      )}
                      {harnessWord(head, harness) !== "" && head.chat.workspace !== "" && " · "}
                      {head.chat.workspace}
                    </span>
                  )}
                  {!head && harness && (
                    <HarnessChip glance={harness} onOpen={() => onOpenCard(harness)} />
                  )}
                  {/* The chat's skills (#2775, S8): not beside a task's path, which is the
                  fullest line a pane has, nor in a narrow pane. */}
                  {!crumbs && roomy && <SkillsChip plane={plane} session={session} />}
                  {from && <span className="pane-from">{from}</span>}
                  {workItem && <span className="pane-work-item">{workItemSaid(workItem)}</span>}
                  {/* **The gauge last** (#3142, rule 15): it is drawn once the chat's first
                  usage arrives, and last on the line its arrival moves nothing already there. */}
                  <ChatGauge usage={usage} />
                </>
              )}
            </div>
          </PaneHead>
        </PaneDropsOf.Provider>
        {/* **The band under the header** (#4754): a Notice arriving never moves the header. */}
        <PaneNotices
          band={pane === undefined ? undefined : paneBand(plane, pane)}
          notices={notices}
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
    </div>
  );
}

/**
 * **A pane's header** (#3142): one slim row under the band and over the terminal, the chat at a
 * glance at its start (its `children`, the `.pane-chips` line: the breadcrumb, the skills, the
 * harness, the gauge; written where it is drawn, so the line's guard reads it) and the pane's
 * actions at its end (`PaneActions`). A row of its own, so neither covers what the terminal
 * draws; the same row in every pane with a chat, so panes side by side line up. With no
 * `children` it is the actions alone at the top right of a pane, as a view's pane draws them
 * (#3147).
 */
export function PaneHead({
  doing,
  menu,
  children,
  grip = true,
}: {
  doing: ReactNode;
  /** Puts the head's right-click around it (#4760): the chat's list, then the pane's rows. */
  menu?: (head: ReactNode) => ReactNode;
  children?: ReactNode;
  /** Whether a press on it, outside its controls, picks the pane up to drag (#4763). */
  grip?: boolean;
}) {
  const drawn = (
    <div className="pane-head" {...(grip ? { [GRIP]: "" } : {})}>
      {children}
      {doing}
    </div>
  );
  return menu === undefined ? drawn : menu(drawn);
}

/**
 * **A pane's actions** (#3142, #3147, #4760): on a chat's pane its one state button, its ⋯ and
 * its bell (#4754), then split it side by side, split it stacked, and close it, as one cluster
 * in its own place, the top right of the pane (`.pane-actions`, `paneFrame.css`). A chat's pane
 * draws it at the end of its header; a view's pane draws the same component with no chat, so
 * every pane's split and close look and behave as one. What it offers is the catalogue's own
 * rows, and the window's rule for when they are drawn.
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
  return (
    <div className="pane-actions">
      {/* **The state button, then ⋯, then the bell** (#4760, spec #4750 story 54), each always
          drawn: a chat's part of the head, before the pane's own controls. The bell keeps a
          fixed slot with its count or none (#4754); the controls after it are drawn only under
          the pointer or the keyboard (`.pane-doing`, `App.css`), and keep their room hidden, so
          nothing beside them moves when a notice arrives or they are drawn. */}
      {state}
      {chat !== undefined && (
        <ChatMenuButton
          session={chat.session}
          offers={chat.offers}
          onPress={(offer) => props.onPaneDoes(props.pane, offer)}
        />
      )}
      <PaneBell />
      <PaneDoing {...props} />
    </div>
  );
}

/** The harness a pane head names, by its card's word or the core's: none for a shell. */
function harnessWord(head: ChatHead, glance: HarnessGlance | undefined): string {
  return glance?.title ?? head.chat.harness ?? "";
}

/** The parts of its head a pane drops for its width (#4760, `paneHead.droppedAt`): none where it
 *  is not measured, and none in a view's pane, whose head is its actions alone. */
export const PaneDropsOf = createContext<ReadonlySet<HeadPart>>(new Set());

/** The widest a pane may be and still be narrow (#2775): 40rem, past which its top line has
 *  room for the Skills chip beside everything else it holds. */
const NARROW_REM = 40;

/** The root font size, in pixels: what a rem is here. */
function remPx(): number {
  return Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
}

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
  const drops = useContext(PaneDropsOf);
  if (ended)
    return brief === undefined ? null : (
      <BriefButton of={brief} word={drops.has("buttonWord") ? undefined : SAID.brief} />
    );
  const pick = stateButtonOf({ session, task, working, has: (id) => offers.has(id) });
  const offer = pick === undefined ? undefined : offers.get(pick.id);
  if (pick === undefined || offer === undefined) return null;
  const worded = !drops.has("buttonWord");
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
 * V100-56). A session's tab shows one of its chats at a time: the session's own, or a task
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

/** A tab's layout, as panes with a handle between each split. */
export function LayoutPanes({
  plane,
  restoring = NONE_RESTORING,
  layout,
  crumbs,
  placed,
  endedTasks,
  unread,
  finished,
  explaining,
  others,
  asked,
  asks,
  waiting,
  onShowInbox,
  onAnswer,
  closeOf,
  onShowChat,
  focused,
  onFocus,
  offerFor,
  offers,
  heads = NO_HEADS,
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
  tab,
  chatName,
  onClearPane,
  onResumeTask,
  names,
  path = "",
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
  /** **The panes left on a task that has ended** (#4757), by pane: the path it last had. Each
   *  keeps the task's last screen, read-only, under the ended band (#4756). */
  endedTasks: Readonly<Record<number, Crumbs>>;
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
  /** This layout's path from its tab's root: `a` for a split's first child, `b` for its
   *  second. The tab's own layout is `""`. */
  path?: string;
}) {
  // The pane is one side of a split. Read here: a chat's pane names its breadcrumb `path`.
  const inSplit = path !== "";
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
          <PaneHead
            grip={false}
            doing={
              <PaneActions
                pane={layout.pane}
                offerFor={offerFor}
                closeOf={closeOf}
                onPaneDoes={onPaneDoes}
              />
            }
          />
          <div
            className={layout.pane === focused ? "pane view focused" : "pane view"}
            onFocus={() => onFocus(layout.pane)}
          >
            <EmptyPane
              running={runningIn(content.workspace)}
              newChat={offerFor("chat.new")}
              onPress={(offer) => onPaneDoes(layout.pane, offer)}
              inSplit={inSplit}
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
      return (
        <div
          className="pane-frame pane-viewed"
          data-focused={layout.pane === focused || undefined}
          onPointerDown={() => onFocus(layout.pane)}
          {...dropPane(tab, layout.pane)}
        >
          {/* Before the view, for `PaneFrame`'s reason: the tab order is the document's. Over
              the view's first row, which keeps the room it takes (`paneFrame.css`). */}
          <PaneHead
            doing={
              <PaneActions
                pane={layout.pane}
                offerFor={offerFor}
                closeOf={closeOf}
                onPaneDoes={onPaneDoes}
              />
            }
          />
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
    const doing = (
      <PaneActions
        pane={layout.pane}
        offerFor={offerFor}
        closeOf={closeOf}
        onPaneDoes={onPaneDoes}
        // No close on a pane showing a task: nothing on screen ends the task, and the
        // session's close is its tab's (#1486).
        task={crumb !== undefined || endedTask !== undefined}
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
        oneJob={isOneJob(content)}
      />
    );
    return (
      <PaneFrame
        plane={plane}
        pane={layout.pane}
        tab={tab}
        session={content.session}
        focused={layout.pane === focused}
        oneJob={isOneJob(content)}
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
          ),
          onPress: (offer) => onPaneDoes(layout.pane, offer),
        }}
        onShowChat={onShowChat}
        from={shownPath === undefined ? handedFrom[content.session] : undefined}
        harness={glances[content.session]}
        onOpenCard={(glance) => onOpenView(harnessCardView(glance.name), glance.label)}
        workItem={workItems[content.session]}
        // What the chat waits on the person for, whatever it writes after (#4563).
        strip={<WaitingStrip session={content.session} waiting={waiting} />}
        notices={
          <>
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
            readOnly={ended || endedTask !== undefined}
          />
        )}
      </PaneFrame>
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
            <LayoutPanes
              path={sideOf(side)}
              plane={plane}
              restoring={restoring}
              layout={child}
              crumbs={crumbs}
              placed={placed}
              endedTasks={endedTasks}
              unread={unread}
              finished={finished}
              explaining={explaining}
              others={others}
              asked={asked}
              asks={asks}
              waiting={waiting}
              onShowInbox={onShowInbox}
              onAnswer={onAnswer}
              closeOf={closeOf}
              onShowChat={onShowChat}
              focused={focused}
              onFocus={onFocus}
              offerFor={offerFor}
              offers={offers}
              heads={heads}
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
              tab={tab}
              chatName={chatName}
              onClearPane={onClearPane}
              onResumeTask={onResumeTask}
              names={named}
            />
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
  onEven,
}: {
  sides: [string, string];
  onEven: () => void;
}) {
  const group = useParentGroup();
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
 *   than `opacity`: an invisible button that can still be clicked is a press nobody meant.
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
  task = false,
  oneJob = false,
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
  /** A one-job program's pane (#4761): no close, since it goes when its program exits. Its
   *  edges still take a split. */
  oneJob?: boolean;
}) {
  // **A task's own pane keeps its minimise whatever it shows** (#1489): switched to a task of
  // its own, it is still the pane that is sent back. Only a close is not drawn over a task.
  const minimise = closeOf(pane)?.does.verb === "sendBack";
  // **A narrow pane drops Split down last of its head's parts** (#4760, `paneHead.ts`); Split
  // right and the close never go. A task's own tab and its pane beside its session are rows of
  // its ⋯ now (`command-table.md`, rule 4), not buttons here.
  const drops = useContext(PaneDropsOf);
  const rows = [
    "pane.split.right",
    ...(drops.has("splitDown") ? [] : ["pane.split.down"]),
    ...((task && !minimise) || oneJob ? [] : ["pane.close"]),
  ];
  return <div className="pane-doing">{rows.map(draw)}</div>;
  function draw(id: string) {
    const offer = id === "pane.close" ? closeOf(pane) : offerFor(id);
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
 * slot before split and close, there with no count when there is nothing, so nothing beside it
 * moves.
 *
 * - **Its count is the pane's notes**, never its asks, the same number minimised and expanded;
 *   neutral, and in the trouble colour while one note is trouble.
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
  return (
    <>
      <Button
        ref={at ?? undefined}
        size="sm"
        variant="quiet"
        className="pane-bell"
        icon={<Bell aria-hidden="true" />}
        aria-label={`Pane notices: ${bellSays(holds)}`}
        aria-expanded={!minimised}
        // The band's head row says Minimise too: the same act, the same word (`docs/ui-copy.md`).
        tooltip={minimised ? "Show all" : "Minimise"}
        data-trouble={holds.trouble > 0 ? "" : undefined}
        data-empty={holds.notes === 0 ? "" : undefined}
        data-way="bell"
        onPress={() => showNotices(minimised ? "expanded" : "minimised")}
      >
        {bellCount(holds)}
      </Button>
      {/* Polite and on its own, never a Notice's status line. */}
      <span className="sr-only" aria-live="polite" aria-atomic="true">
        {spoken}
      </span>
    </>
  );
}
