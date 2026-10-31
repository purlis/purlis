// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { AnchoredMenu, Menu, MenuItem, MenuSeparator, Submenu } from "./ui/menu/Menu";
import { MenuTip, useRested } from "./ui/menu/MenuTip";
import {
  Check,
  ChevronRight,
  Circle,
  FileText,
  Hand,
  Minus,
  PanelRight,
  Pause,
  Reply,
  SquareArrowOutUpRight,
  SquareTerminal,
  X,
  type LucideIcon,
} from "lucide-react";
import type { Offer } from "./actions";
import { backId, besideId, ownTabId } from "./actions";
import { onAMac } from "./tabKeys";
import { briefTitle, useOpenBrief, type BriefAsk } from "./Brief";
import { answerTitle, useOpenAnswer, type AnswerAsk } from "./AnswerPanel";
import { ChatDoingLine } from "./ChatRowActivity";
import { ChatShownState } from "./ChatRows";
import { sameList, useChatsHere, useChatsSelect } from "./chatState";
import { sinceSaid } from "./chatsList";
import type { ChatRow } from "./chatsTree";
import { PersonaMark } from "./PersonaMark";
import { useStateSince, type StateClock } from "./stateClock";
import { StateShown } from "./StateShown";
import {
  GRACE_MS,
  REST_DRIFT,
  REST_MS,
  chipSaid,
  countsOf,
  kindsOf,
  menuOf,
  needsSaid,
  wearsChip,
  type Ended,
  type Line,
  type Needing,
} from "./tabTasks";
import { TASK_BUCKETS, TASK_BUCKET_DRAWN, tasksIn, type TaskBucket } from "./taskBuckets";
import type { TasksUsed } from "./bindings";
import { askOf, tokensOfLine, tokensSaid, type UsedAsk, type UsedReader } from "./tasksUsed";
import { property } from "./theme/theme";
import { fitCountsOn } from "./wholeWays";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { Button, IconButton } from "./ui/button/Button";
import { Twist } from "./ui/disclosure/Disclosure";
import { tabPart } from "./ui/tabs/Tabs";

/** The chip's hand: in the needs-you colour, at the words' size, drawn on the icon itself
 *  (#3264), as each count's mark is, so the button's class says only where it stands. */
const HAND: CSSProperties = {
  width: "1em",
  height: "1em",
  color: `var(${property("needs-you.base")})`,
};

/** Each count's mark: its shape as it is drawn (`StateShown` draws the same four for a row). */
const MARKS: Readonly<Record<TaskBucket, LucideIcon>> = {
  working: Circle,
  waiting: Pause,
  failed: X,
  done: Check,
};

type Props = {
  /** The id of the counts button: what the tab is described by, so a screen reader on the
   *  tab hears that it has tasks. */
  id: string;
  /** The session's name, as its tab says it: whose tasks these are. */
  name: string;
  /** The chats of the tab, as a tree (`tabChats.chatsOfTab`). */
  rows: readonly ChatRow[];
  /** Its tasks that have ended and still have a line. */
  ended: readonly Ended[];
  /** The chat the tab shows now. */
  current: number | undefined;
  /** Its chats waiting for the person and not on screen, the longest waiting first. */
  needs: readonly Needing[];
  /** How many times the window has asked for a tab's menu from the keyboard, while this tab's
   *  is the one asked for, and 0 while another's is. A rise opens it, or closes it if it is
   *  open; a fall to 0 closes it, so one menu is open at a time. */
  asked: number;
  /** How long each chat has been in its state, as this window saw it. */
  clock: StateClock;
  /** Whether a tab is being carried along the strip: no menu opens under a drag. */
  dragging: () => boolean;
  /** Goes to a chat: the tab is switched to it, and its terminal takes the keyboard. */
  onShow: (session: number) => void;
  /** Moves a task (#1489): to a tab of its own, beside its session, or back out of either.
   *  Left out, a task's line offers none of them. */
  onPlace?: (session: number, where: Place) => void;
  /** The menu's footer, for what later says it: the limits that bind (#1498). Nothing is
   *  drawn for a slot nobody fills. */
  limits?: ReactNode;
  /**
   * Reads what the tab's chats used (#1500): each line's tokens, on its hover, and the total
   * line at the menu's foot. Read as the menu opens and when a chat of it changes state while
   * it is open, never on a timer. Left out, the menu says nothing of tokens.
   */
  used?: UsedReader;
  /**
   * The two rows that end task `session`, from the window's catalogue (#1488,
   * `actions.taskEndIds`): Stop and get its report, then Close now. Read as the menu is
   * drawn. Left out, the menu offers no way to end a task.
   */
  ends?: (session: number) => readonly Offer[];
  /** Presses one of those rows: the window asks its second step, and nothing ends on the
   *  press (`TaskEnd.tsx`). */
  onPress?: (offer: Offer) => void;
  /**
   * Stop all tasks of session `session`, from the window's catalogue (#1498,
   * `actions.stopAllId`): the last row of "End a task", under a line of its own. Pressed
   * through `onPress`, so the window asks its one question first. Left out, or not available,
   * it is not drawn.
   */
  stopAll?: (session: number) => Offer | undefined;
  /** Opens the Activity of chat `session`, the tab's own (#1495): what it and its tasks said
   *  to each other. Left out, the menu has no Activity line. */
  onActivity?: (session: number) => void;
};

/** Where a task's line in a tab's menu offers to move it (#1489). */
export type Place = "own" | "beside" | "back";

/** The catalogue's row for moving task `session` to `where`: the one its line presses. */
export function placeRowId(session: number, where: Place): string {
  return where === "own"
    ? ownTabId(session)
    : where === "beside"
      ? besideId(session)
      : backId(session);
}

const ROW_FACTS = [
  "session",
  "name",
  "persona",
  "workspace",
  "level",
  "placed",
  "shell",
  "report",
  "outcome",
  "asking",
  "harness",
] as const;

function sameRows(one: readonly ChatRow[], other: readonly ChatRow[]): boolean {
  return (
    one === other ||
    (one.length === other.length &&
      one.every((row, at) => ROW_FACTS.every((fact) => row[fact] === other[at][fact])))
  );
}

const ENDED_FACTS = [
  "key",
  "session",
  "asker",
  "name",
  "persona",
  "qualifier",
  "folds",
  "bucket",
  "elsewhere",
  "report",
] as const;

function sameEnded(one: readonly Ended[], other: readonly Ended[]): boolean {
  return (
    one === other ||
    (one.length === other.length &&
      one.every((task, at) => {
        const against = other[at];
        return (
          ENDED_FACTS.every((fact) => task[fact] === against[fact]) &&
          task.shown.kind === against.shown.kind &&
          task.shown.word === against.shown.word
        );
      }))
  );
}

function sameNeeds(one: readonly Needing[], other: readonly Needing[]): boolean {
  return (
    one === other ||
    (one.length === other.length &&
      one.every((chat, at) => chat.session === other[at].session && chat.name === other[at].name))
  );
}

/** Whether a chip would be drawn the same: held on what it says, not on the lists' identity,
 *  which the window builds again whenever any tab changes. */
function sameChip(was: Props, now: Props): boolean {
  return (
    was.id === now.id &&
    was.name === now.name &&
    was.current === now.current &&
    was.asked === now.asked &&
    was.clock === now.clock &&
    was.dragging === now.dragging &&
    was.onShow === now.onShow &&
    was.onPlace === now.onPlace &&
    was.limits === now.limits &&
    was.used === now.used &&
    was.ends === now.ends &&
    was.onPress === now.onPress &&
    was.stopAll === now.stopAll &&
    was.onActivity === now.onActivity &&
    sameRows(was.rows, now.rows) &&
    sameEnded(was.ended, now.ended) &&
    sameNeeds(was.needs, now.needs)
  );
}

/**
 * **The chip a session's tab wears for its tasks** (#1487, V100-32, V100-33, V100-37): how
 * many are working, waiting, failed and finished (`taskCounts.ts`), each count with its
 * state's shape, and the hand when a chat of the tab is waiting for the person off screen. It
 * opens the menu of the tab's chats. A tab with no tasks draws nothing here, and is the tab it
 * always was.
 *
 * **Beside the tab's button and not inside it**: the tab is a button, and a button holds no
 * button. The hand is its own button for the same reason: pressing it goes to the chat that
 * waits, and pressing the counts opens the menu.
 *
 * **It reads its own chats' states** (SC-3), so a task that moves redraws this chip, and only
 * when a state changes. The strip around it is not drawn again.
 *
 * **Three ways in, and each closes its own way.**
 * - **A press**, and **the keyboard** (the window's key, Down on the tab, the palette): the
 *   menu has the keyboard, and stays until Escape, a pick, a press outside, or the same ask
 *   again. A pointer wandering off closes neither: the next keys would go to the terminal.
 * - **A rest**: the pointer, moved onto the counts by the person's hand and then still for
 *   {@link REST_MS}. That menu takes no keyboard, so whoever was typing goes on typing, and it
 *   also closes {@link GRACE_MS} after the pointer has left both the chip and the menu.
 *
 * **A rest is only ever the person's.** The engine sends "the pointer came onto this" when a
 * chip appears, widens or slides under a pointer that has not moved. So coming on arms
 * nothing: the rest starts at the first move to a new point, with no button held. A pointer
 * passing over is on the chip for less than a rest, and nothing opens under a drag.
 */
export const TabTasks = memo(function TabTasks(props: Props) {
  if (!wearsChip(props.rows, props.ended, props.needs)) return null;
  return <Chip {...props} />;
}, sameChip);

/** How a menu came to be open: what decides who has the keyboard and what closes it. */
type OpenedBy = "press" | "rest" | "keys";

type Point = { x: number; y: number };

const NO_REPORTS: ReadonlySet<string> = new Set();

function Chip({
  id,
  name,
  rows,
  ended,
  current,
  needs,
  asked,
  clock,
  dragging,
  onShow,
  onPlace,
  limits,
  used,
  ends,
  onPress,
  stopAll,
  onActivity,
}: Props) {
  // One reading of the store for the chip and its menu: each row's state, redrawn only when
  // one of them changes.
  const kinds = useChatsSelect(useChatsHere(), (states) => kindsOf(states, rows), sameList);
  const counts = countsOf(rows, kinds, ended);
  // The Brief panel of a task (#1494), where the window has one to open.
  const openBrief = useOpenBrief();
  // The Answer dialog of a task asking its asking chat (#1551), where the window has one.
  const openAnswer = useOpenAnswer();
  const [open, setOpen] = useState(false);
  const [unfolded, setUnfolded] = useState(false);
  /** The ended lines whose reports are open, by the line's key. */
  const [reports, setReports] = useState(NO_REPORTS);
  // How it was opened: a ref for the handlers, which act in the turn it is set, and state for
  // what is drawn by it.
  const by = useRef<OpenedBy | undefined>(undefined);
  const [how, setHow] = useState<OpenedBy>();
  // Whether it is open, for the effect that answers the window's ask in the turn it lands.
  const isOpen = useRef(false);
  const resting = useRef<number | undefined>(undefined);
  const leaving = useRef<number | undefined>(undefined);
  /** Where the pointer was last seen over the counts, and where its rest is counted from. */
  const seenAt = useRef<Point | undefined>(undefined);
  const restedAt = useRef<Point | undefined>(undefined);
  /** Where the keyboard was when the menu opened: where it goes back to on a close. */
  const cameFrom = useRef<Element | null>(null);
  const pressedFrom = useRef<Element | null | undefined>(undefined);
  const picked = useRef(false);
  const content = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);

  const notResting = useCallback(() => {
    window.clearTimeout(resting.current);
    resting.current = undefined;
    restedAt.current = undefined;
  }, []);
  const notLeaving = useCallback(() => {
    window.clearTimeout(leaving.current);
    leaving.current = undefined;
  }, []);
  useEffect(
    () => () => {
      notResting();
      notLeaving();
    },
    [notLeaving, notResting],
  );

  const show = useCallback((opened: OpenedBy, from: Element | null) => {
    by.current = opened;
    setHow(opened);
    cameFrom.current = from;
    picked.current = false;
    isOpen.current = true;
    setOpen(true);
  }, []);
  const hide = useCallback(() => {
    notResting();
    notLeaving();
    isOpen.current = false;
    setOpen(false);
    setUnfolded(false);
    setReports(NO_REPORTS);
  }, [notLeaving, notResting]);

  // **The window asked for a tab's menu from the keyboard** (the catalogue's row, its key, or
  // Down on the tab). The count only grows, and is this tab's only while this tab's menu is
  // the one asked for. So a rise is an ask of this tab: it opens, or closes if it is open, as
  // a second press does. A fall to 0 is an ask of another tab: one menu at a time.
  const answered = useRef(asked);
  useEffect(() => {
    const was = answered.current;
    answered.current = asked;
    if (asked === was) return;
    notResting();
    if (asked < was || isOpen.current) hide();
    else show("keys", document.activeElement);
  }, [asked, hide, notResting, show]);

  /** The pointer moved over the counts: a rest is counted from where it stopped. */
  const moved = (event: ReactPointerEvent) => {
    const at = { x: event.clientX, y: event.clientY };
    const last = seenAt.current;
    seenAt.current = at;
    // The engine's own "it is still here", sent when the layout moves under a pointer that
    // has not: the same point again. Not the person's hand, and it arms nothing.
    if (last !== undefined && last.x === at.x && last.y === at.y) return;
    // A button is down: a selection being dragged over the strip, or the press that begins a
    // tab's drag. Not a pointer coming to rest.
    if (event.buttons !== 0 || event.pointerType === "touch" || isOpen.current || dragging()) {
      notResting();
      return;
    }
    const from = restedAt.current;
    if (
      from !== undefined &&
      Math.abs(at.x - from.x) <= REST_DRIFT &&
      Math.abs(at.y - from.y) <= REST_DRIFT
    )
      return;
    notResting();
    restedAt.current = at;
    resting.current = window.setTimeout(() => {
      resting.current = undefined;
      if (dragging() || isOpen.current) return;
      show("rest", document.activeElement);
    }, REST_MS);
  };
  /** The pointer left the chip or the menu: a menu its rest opened closes soon. */
  const leaveSoon = () => {
    notResting();
    seenAt.current = undefined;
    // Only a menu the rest opened. A pressed or a key-opened menu has the keyboard, and the
    // keys typed after the pointer drifts off must find it still there.
    if (!isOpen.current || by.current !== "rest") return;
    notLeaving();
    leaving.current = window.setTimeout(hide, GRACE_MS);
  };

  // **The keyboard's own open lands on the chat the tab shows**, or its first line: the menu
  // takes the keyboard on itself as it opens, and then on that line.
  useEffect(() => {
    if (!open || how !== "keys") return;
    const frame = window.requestAnimationFrame(() => {
      const list = content.current;
      const on =
        list?.querySelector<HTMLElement>("[data-current]") ??
        list?.querySelector<HTMLElement>('[role^="menuitem"]');
      on?.focus({ preventScroll: true });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, how]);

  const { lines, finished } = menuOf(rows, kinds, ended, current);
  const figures = useUsed(used, open, [...lines, ...finished], kinds);
  // The fold is open where the chat on screen is in it, so its line is never hidden.
  const folded = !unfolded && !finished.some((line) => line.current);
  const quiet = how === "rest";
  const pick = (line: Line): boolean => {
    if (line.row === undefined) {
      // No chat to go to: the task has ended. Its line opens its report, where it has one,
      // and the menu stays with the keyboard where it is.
      if (line.report !== undefined)
        setReports((was) => {
          const next = new Set(was);
          if (!next.delete(line.key)) next.add(line.key);
          return next;
        });
      return false;
    }
    picked.current = true;
    onShow(line.row.session);
    return true;
  };
  const said = chipSaid(name, counts);
  // **Each count whole or not at all**, and only once the name has given up what it can:
  // measured, as the tab strip is (`wholeWays.ts`).
  const anyTasks = tasksIn(counts) > 0;
  useLayoutEffect(() => {
    if (!anyTasks || trigger.current === null) return;
    return fitCountsOn(trigger.current);
  }, [anyTasks]);
  /** The tab's open tasks, each with the rows that end it: what "End a task" lists. Never a
   *  pane's own chat, which is no task, and never a task that has ended. */
  const endable =
    ends === undefined || onPress === undefined
      ? []
      : lines.flatMap((one) =>
          one.row !== undefined && one.row.mode === "task"
            ? [{ key: one.key, offers: ends(one.row.session) }]
            : [],
        );
  /** The tab's own session: whose tasks Stop all ends (#1498) and whose Activity this menu
   *  opens (#1495). */
  const own = rows.find((row) => row.level === 1)?.session;
  const all =
    stopAll === undefined || onPress === undefined || own === undefined ? undefined : stopAll(own);
  const allOffer = all?.available === true ? all : undefined;
  const line = (one: Line) => (
    <TaskLine
      key={one.key}
      line={one}
      clock={clock}
      tokens={tokensSaid(tokensOfLine(figures, one))}
      reportOpen={reports.has(one.key)}
      onPick={pick}
      onBrief={
        openBrief === undefined
          ? undefined
          : (ask) => {
              // The panel takes the keyboard: the menu is done.
              picked.current = true;
              hide();
              openBrief(ask);
            }
      }
      onAnswer={
        openAnswer === undefined
          ? undefined
          : (ask) => {
              // The dialog takes the keyboard: the menu is done.
              picked.current = true;
              hide();
              openAnswer(ask);
            }
      }
      onPlace={
        onPlace === undefined
          ? undefined
          : (session, where) => {
              // The task is gone to where it was sent, with the keyboard: the menu is done.
              picked.current = true;
              onPlace(session, where);
              hide();
            }
      }
    />
  );

  return (
    <span className="tab-tasks" data-open={open || undefined}>
      {/* **A chat of this tab is waiting for you and is not on screen** (V100-37): the hand,
          and a press goes to the one that has waited longest. The tab stays there after. */}
      {needs.length > 0 && (
        // The quiet ghost (#3264), its hand in the needs-you colour, in a box of its own that
        // draws the keyboard's ring inside it (`.tab-tasks-mark`).
        <span className="tab-tasks-mark">
          <IconButton
            variant="quiet"
            size="sm"
            icon={<Hand aria-hidden="true" style={HAND} />}
            className="tab-tasks-hand"
            data-mark="needs-you"
            tabIndex={-1}
            aria-label={`${needsSaid(needs.map((chat) => chat.name))}. Go to ${needs[0].name}`}
            onPress={() => onShow(needs[0].session)}
          />
        </span>
      )}
      {tasksIn(counts) > 0 && (
        <>
          {/* **The chip's pill** (#3264): the sunken ground the counts stand on, and the open
              menu's, drawn here; the counts themselves are the button family's quiet ghost. */}
          <span className="tab-tasks-pill">
            <Button
              variant="quiet"
              size="sm"
              id={id}
              ref={trigger}
              className="tab-tasks-counts"
              // Not a Tab stop, as a tab's `×` is not: the strip is one stop. The keyboard opens
              // this menu with its key, with Down on the tab, or from the palette. No `title`: a
              // tooltip would come up over the menu the same rest opened.
              tabIndex={-1}
              aria-label={said}
              aria-haspopup="menu"
              aria-expanded={open}
              // The click toggles it, not the pointer going down: not every way of pressing a
              // button sends a pointer event (`ShowMore` has the measurement).
              onPointerDown={(event) => {
                event.preventDefault();
                notResting();
                pressedFrom.current = document.activeElement;
              }}
              // A press that did not end in a click (a right press, one let go elsewhere) is
              // over: where the keyboard was then says nothing of the next press.
              onPointerUp={() => {
                window.setTimeout(() => (pressedFrom.current = undefined));
              }}
              onPointerCancel={() => {
                pressedFrom.current = undefined;
              }}
              // A press by any pointer, or a screen reader's, which sends none.
              onPress={() => {
                const from = pressedFrom.current ?? document.activeElement;
                pressedFrom.current = undefined;
                notResting();
                notLeaving();
                if (dragging()) return;
                if (open && by.current === "rest") {
                  // A press on a menu the rest opened keeps it, and gives it the keyboard.
                  by.current = "press";
                  setHow("press");
                  content.current?.focus({ preventScroll: true });
                } else if (open) hide();
                else show("press", from === trigger.current ? null : from);
              }}
              // Coming on arms nothing (see `TabTasks`): it is only where the pointer is.
              onPointerEnter={(event) => {
                notLeaving();
                seenAt.current = { x: event.clientX, y: event.clientY };
              }}
              onPointerMove={moved}
              onPointerLeave={leaveSoon}
            >
              {TASK_BUCKETS.filter((bucket) => counts[bucket] > 0).map((bucket) => {
                const Mark = MARKS[bucket];
                return (
                  <span
                    key={bucket}
                    className="count"
                    data-count={bucket}
                    aria-hidden="true"
                    // A count on a tab, which a theme's `tabs.count.*` reaches (`ui/tabs`).
                    {...tabPart("count")}
                  >
                    <Mark style={{ color: `var(${property(TASK_BUCKET_DRAWN[bucket].token)})` }} />
                    {counts[bucket]}
                  </span>
                );
              })}
            </Button>
          </span>
          <AnchoredMenu
            triggerRef={trigger}
            isOpen={open}
            // Escape, a press outside and a pick close it; the pointer's ways in are above.
            onOpenChange={(next) => {
              if (!next) hide();
            }}
            // A rest takes no keyboard; a press puts it on the menu, and the keyboard's own
            // open on the chat the tab shows (below), or the first line.
            autoFocus={how !== "rest"}
            onClosed={() => {
              // A pick puts the keyboard in the chat picked (`onShow`). Otherwise it goes back
              // to where it was before the menu, which for a rest is where it never left.
              const back = cameFrom.current;
              if (picked.current || !(back instanceof HTMLElement) || !back.isConnected) return;
              const at = document.activeElement;
              if (at === null || at === document.body || !at.isConnected) back.focus();
            }}
          >
            <Menu
              listRef={content}
              // Named by the chip that opened it: "Tasks of steward 4: …". Said here because the
              // chip's id is the window's (the tab is described by it).
              aria-labelledby={id}
              className="tasks-menu"
              takesNoKeyboard={quiet}
              onPointerEnter={notLeaving}
              onPointerLeave={leaveSoon}
              footer={
                (limits != null || figures?.total != null) && (
                  <div className="tasks-menu-foot">
                    {limits != null && <div className="tasks-menu-limits">{limits}</div>}
                    {/* What the tab's chats used (#1500, V100-43): one line, added up and
                        spelled by the core, with what it adds up as its hint. No money. */}
                    {figures?.total != null && (
                      <Tooltip tip={figures.total.explained}>
                        <div className="tasks-menu-totals" data-testid="tasks-used">
                          {figures.total.said}
                        </div>
                      </Tooltip>
                    )}
                  </div>
                )
              }
            >
              {lines.map(line)}
              {(endable.some((task) => task.offers.length > 0) || allOffer !== undefined) && (
                /* **Ending a task from here** (#1488): the two rows the catalogue has for each
                   open task, by their own titles, so this menu, a row's menu, the breadcrumb
                   and the palette say one thing. One line of this menu, which opens to them:
                   a way to end a task is never one stray press from a way to go to it. A
                   press ends nothing: the window asks its second step. */
                <Submenu
                  className="tasks-menu tasks-menu-ends"
                  aria-label="End a task"
                  row={
                    <MenuItem
                      id="end-a-task"
                      className="tasks-menu-row tasks-menu-end"
                      textValue="End a task"
                    >
                      <span className="name">End a task</span>
                    </MenuItem>
                  }
                >
                  {endable.flatMap((task) =>
                    task.offers.map((offer) => (
                      <MenuItem
                        key={offer.id}
                        id={offer.id}
                        className="tasks-menu-row"
                        textValue={offer.title}
                        // A row that cannot run stays in the arrows' way and says why.
                        aria-disabled={offer.available ? undefined : true}
                        shouldCloseOnSelect={offer.available}
                        onAction={() => {
                          if (!offer.available) return;
                          // The keyboard goes to the question the press opens, not back to
                          // where it was before the menu.
                          picked.current = true;
                          onPress?.(offer);
                        }}
                      >
                        <span className="name">{offer.title}</span>
                        {!offer.available && <span className="where"> {offer.reason}</span>}
                      </MenuItem>
                    )),
                  )}
                  {allOffer !== undefined && (
                    /* **Stop all tasks** (#1498, V100-53): its own row, last, under a line,
                       since it ends every task above it. The session keeps running. A press
                       stops nothing: the window asks once, naming how many. */
                    <>
                      <MenuSeparator />
                      <MenuItem
                        id="stop-all-tasks"
                        className="tasks-menu-row tasks-menu-stop-all"
                        textValue="Stop all tasks"
                        onAction={() => {
                          picked.current = true;
                          onPress?.(allOffer);
                        }}
                      >
                        <span className="name">Stop all tasks</span>
                      </MenuItem>
                    </>
                  )}
                </Submenu>
              )}
              {own !== undefined && onActivity !== undefined && (
                /* **The session's Activity** (#1495, V100-44): what its chat and its tasks
                   said to each other, in a tab of its own. It only reads. */
                <ActivityRow
                  id={`${id}-activity`}
                  quiet={quiet}
                  onPick={() => {
                    picked.current = true;
                    onActivity(own);
                  }}
                />
              )}
              {finished.length > 0 && (
                <MenuItem
                  id="finished"
                  className="tasks-menu-row tasks-menu-fold"
                  aria-expanded={!folded}
                  textValue="Finished"
                  // Unfolds in place: the menu stays.
                  shouldCloseOnSelect={false}
                  // Right opens it and Left closes it, as on a row of a tree.
                  onKeyDown={(event) => {
                    if (event.key === "ArrowRight" && folded) setUnfolded(true);
                    else if (event.key === "ArrowLeft" && !folded) setUnfolded(false);
                    else return;
                    event.preventDefault();
                  }}
                  onAction={() => setUnfolded(folded)}
                >
                  <Twist open={!folded} />
                  <span className="name">Finished ({finished.length})</span>
                </MenuItem>
              )}
              {!folded && finished.map(line)}
            </Menu>
          </AnchoredMenu>
        </>
      )}
    </span>
  );
}

/** What the menu's Activity row opens: its tooltip and its description. */
const ACTIVITY_SAYS = "What its chats said to each other, in a tab of its own.";

/**
 * The menu's Activity row. What it opens is its tooltip and its description, not a line beside
 * its word (the menus' rule 2, `docs/ui-primitives.md`): the menus' own tooltip (`MenuTip`),
 * shown once the keyboard has rested on the row, or the pointer in a menu that took no keyboard.
 */
function ActivityRow({
  id,
  quiet,
  onPick,
}: {
  /** The id of the row's description. */
  id: string;
  /** Whether the menu took no keyboard: the pointer's coming and going say whether it rests. */
  quiet: boolean;
  onPick: () => void;
}) {
  const { rested, rest, wake, hold } = useRested(true);
  return (
    <MenuItem
      id="activity"
      className="tasks-menu-row tasks-menu-activity"
      textValue="Activity"
      aria-label="Activity"
      aria-describedby={id}
      {...rest}
      // In a menu that took no keyboard the row is never focused, so the pointer's own coming
      // and going say whether it rests here.
      onHoverStart={quiet ? rest.onFocus : undefined}
      onHoverEnd={quiet ? wake : undefined}
      onAction={() => {
        wake();
        onPick();
      }}
    >
      <span className="name">Activity</span>
      <span id={id} hidden>
        {ACTIVITY_SAYS}
      </span>
      {rested && (
        <MenuTip side="right" onClose={wake} hold={hold}>
          {ACTIVITY_SAYS}
        </MenuTip>
      )}
    </MenuItem>
  );
}

/** The deepest step in a line is drawn at. A task of a task of a task of a task is rare, and
 *  one further in would leave its name no room. */
const DEEPEST = 6;

/**
 * One chat of the tab, as its menu lists it (V100-19, V100-41): its persona's mark, its name,
 * the workspace it works in where that is not its session's, its state as a word that is never
 * cut, and how long it has been in it. Indented under the chat that asked for it, and said to
 * be asked by it where that is not the session itself. The chat the tab shows now is marked,
 * and said to be.
 *
 * **A line with a chat goes to it. A line with none opens its report**, in place and as text,
 * as its finished row in the Chats list does: a task that has ended has nothing to be shown,
 * and its line is still reached by the arrows and read like any other.
 */
function TaskLine({
  line,
  clock,
  tokens,
  reportOpen,
  onPick,
  onBrief,
  onAnswer,
  onPlace,
}: {
  line: Line;
  clock: StateClock;
  /** What the line's chat used, as its hover says it (#1500); nothing before it is read. */
  tokens: string | undefined;
  reportOpen: boolean;
  /** Answers whether the menu is done with: it went to a chat. */
  onPick: (line: Line) => boolean;
  /** Opens the Brief panel of the line's task (#1494). */
  onBrief?: (ask: BriefAsk) => void;
  /** Opens the Answer dialog of the line's task, where it asks its asking chat (#1551). */
  onAnswer?: (ask: AnswerAsk) => void;
  /** Moves the line's task, where it is an open task and the menu was handed the way. */
  onPlace?: (session: number, where: Place) => void;
}) {
  const { row } = line;
  // **Its Brief** (#1494): an open task by its chat, an ended one by its dispatch record while
  // it has a finished row. A session's own chat was sent nothing.
  const brief: BriefAsk | undefined =
    onBrief === undefined
      ? undefined
      : row !== undefined
        ? row.mode === "task"
          ? { chat: row.session, name: line.name }
          : undefined
        : line.key.startsWith(FINISHED_KEY)
          ? { dispatch: line.key.slice(FINISHED_KEY.length), name: line.name }
          : undefined;
  // **Where a task is drawn, from its own line** (#1489, V100-38): an open task only. The
  // session's own chat is its tab, and an ended task is nowhere.
  const moves = onPlace !== undefined && row !== undefined && row.mode === "task" ? row : undefined;
  // **Its question, to answer** (#1551): an open task paused on a question to its asking chat.
  const answer: AnswerAsk | undefined =
    onAnswer !== undefined && row !== undefined && row.mode === "task" && row.asking !== null
      ? { chat: row.session, name: line.name }
      : undefined;
  return (
    <>
      <MenuItem
        id={line.key}
        className="tasks-menu-row"
        // How far in its line stands: one step per chat between it and the session.
        data-level={Math.min(line.level, DEEPEST)}
        // What the menu's own open lands on and the stylesheet weighs. Said in words below:
        // `aria-current` in a menu is read unevenly, and both would say it twice.
        data-current={line.current ? "" : undefined}
        aria-expanded={line.report === undefined ? undefined : reportOpen}
        textValue={line.name}
        // The line's one hint: its whole name, cut short where the menu is narrow, then what it
        // used.
        tooltip={tokens === undefined ? line.name : `${line.name}\n${tokens}`}
        // What its task is doing is its description on demand (#1551), as the Chats list's row
        // has it, and its report while that is open. An id that names no element, while
        // nothing is said, describes it with nothing.
        aria-describedby={
          [
            row === undefined ? undefined : menuDoingId(row.session),
            reportOpen && line.report !== undefined ? reportId(line.key) : undefined,
          ]
            .filter(Boolean)
            .join(" ") || undefined
        }
        // **The keyboard's way to move a task from its line** (#1489): Enter goes to it, and
        // Enter with a modifier opens it elsewhere, as a link is opened in a new tab. ⌘ or
        // Ctrl is a tab of its own; Alt is beside its session. Before the menu's own Enter.
        aria-keyshortcuts={moves === undefined ? undefined : "Control+Enter Meta+Enter Alt+Enter"}
        onKeyDown={(event) => {
          const where: Place | undefined =
            event.metaKey || event.ctrlKey ? "own" : event.altKey ? "beside" : undefined;
          if (
            moves === undefined ||
            onPlace === undefined ||
            event.key !== "Enter" ||
            where === undefined
          )
            return;
          event.preventDefault();
          if (!event.repeat) onPlace(moves.session, where);
        }}
        // A line with a chat goes to it and the menu is done; one with none opens its report in
        // place, and the menu stays with the keyboard on this line.
        shouldCloseOnSelect={line.row !== undefined}
        onAction={() => {
          onPick(line);
        }}
      >
        <span className="now" aria-hidden="true">
          {line.current && <ChevronRight />}
        </span>
        {line.persona === null ? (
          <SquareTerminal className="node-icon" aria-hidden="true" />
        ) : (
          <PersonaMark persona={line.persona} />
        )}
        {/* The spaces are for whoever reads the row as text: a screen reader says the name,
            then the state, and not the two run together. The row lays them out itself. */}
        <span className="name">{line.name}</span>
        {line.current && <span className="hidden-words">, shown now</span>}
        {/* Who asked for it, where the indent is all that draws it: said, since an indent is
            not read; and drawn too in the fold, where every line stands at one step. */}
        {line.askedBy !== null && (
          <span className={line.level > 2 ? "hidden-words" : "asked"}>
            , asked by {line.askedBy}
          </span>
        )}
        {line.elsewhere !== null && <span className="where"> in {line.elsewhere}</span>}
        {/* A task with a pane of its own is still this tab's (V100-38), and says where it is:
            picking it brings that forward. */}
        {line.placed !== undefined && (
          <span className="where placed" data-placed={line.placed}>
            {line.placed === "tab" ? ", in its own tab" : ", beside it"}
          </span>
        )}{" "}
        {row !== undefined ? (
          <ChatShownState
            session={row.session}
            shell={row.shell}
            report={row.report}
            outcome={row.outcome}
            asking={row.asking}
            harness={row.harness}
          />
        ) : (
          line.ended !== undefined && <StateShown shown={line.ended} />
        )}
        {line.qualifier !== undefined && <span className="where"> {line.qualifier}</span>}
        {row !== undefined && <Since clock={clock} session={row.session} />}
        {/* What it is doing (#1493): one dim line, cut short; nothing while it is not
            working. It reads its own chat, so this line is not drawn again for it. */}
        {row !== undefined && (
          <span className="doing">
            <ChatDoingLine session={row.session} id={menuDoingId(row.session)} />
          </span>
        )}
        {(brief !== undefined || answer !== undefined) && moves === undefined && (
          <span className="tasks-menu-places" aria-hidden="true">
            {brief !== undefined && <BriefPlace ask={brief} onBrief={onBrief} />}
            {answer !== undefined && <AnswerPlace ask={answer} onAnswer={onAnswer} />}
          </span>
        )}
        {moves !== undefined && onPlace !== undefined && (
          /* **The pointer's way to move a task from its line** (#1489), drawn at the line's
             end while the pointer is on it: to a tab of its own and beside its session, or
             back for one that has a pane of its own. **Not in the accessibility tree**, as a
             list row's fold is not: a button inside a menu's item is reached by no arrow, so
             the keyboard's way is the line's own keys above, and the task's menu in the Chats
             list. */
          <span className="tasks-menu-places" aria-hidden="true">
            {line.placed !== "tab" && (
              <PlaceButton
                says={`Move ${line.name} to its own tab`}
                keys={onAMac() ? "⌘Enter" : "Ctrl+Enter"}
                onPress={() => onPlace(moves.session, "own")}
              >
                <SquareArrowOutUpRight />
              </PlaceButton>
            )}
            {line.placed !== "beside" && (
              <PlaceButton
                says={`Open ${line.name} beside its session`}
                keys={onAMac() ? "⌥Enter" : "Alt+Enter"}
                onPress={() => onPlace(moves.session, "beside")}
              >
                <PanelRight />
              </PlaceButton>
            )}
            {line.placed !== undefined && (
              <PlaceButton
                says={`Send ${line.name} back into this tab`}
                onPress={() => onPlace(moves.session, "back")}
              >
                <Minus />
              </PlaceButton>
            )}
            {/* Last: it reads, where the others move the task (#1494). */}
            {brief !== undefined && <BriefPlace ask={brief} onBrief={onBrief} />}
            {/* Last, at the line's very end: what the task waits on (#1551). */}
            {answer !== undefined && <AnswerPlace ask={answer} onAnswer={onAnswer} />}
          </span>
        )}
        {reportOpen && line.report !== undefined && (
          /* **Its report, opened under its words**: text, as its finished row in the Chats
             list draws it, and the line's description while it is open. */
          <span
            className="tasks-menu-report"
            id={reportId(line.key)}
            data-level={Math.min(line.level, DEEPEST)}
          >
            {/* Text nodes, every one: a report is a chat's words, and is never markup here. */}
            <span className="report-text">{line.report}</span>
          </span>
        )}
      </MenuItem>
    </>
  );
}

/** The id of an ended task's report, open under its line. */
function reportId(key: string): string {
  return `tasks-menu-report-${key.replace(/[^\w-]/g, "-")}`;
}

/** The id of what a menu line's task is doing, which the line is described by (#1551): its
 *  own, as an id is one element's. */
function menuDoingId(session: number): string {
  return `tasks-menu-doing-${session}`;
}

/** The key of an ended task's line that has a finished row: `finished:<dispatch id>`. */
const FINISHED_KEY = "finished:";

/** The Brief button at the end of a task's line (#1494): the pointer's way to it from here;
 *  the keyboard's is the task's row menu, its finished row and the palette. */
function BriefPlace({ ask, onBrief }: { ask: BriefAsk; onBrief?: (ask: BriefAsk) => void }) {
  if (onBrief === undefined) return null;
  return (
    <PlaceButton
      says={briefTitle(ask.name)}
      // The keyboard's way to the same panel, since this button is the pointer's alone.
      also={
        "chat" in ask
          ? "Also in its row's menu in the Chats list"
          : "Also on its finished row in the Chats list"
      }
      onPress={() => onBrief(ask)}
    >
      <FileText />
    </PlaceButton>
  );
}

/** The Answer button at the end of an asking task's line (#1551): the pointer's way to its
 *  dialog from here; the keyboard's is the task's row menu and the palette. */
function AnswerPlace({ ask, onAnswer }: { ask: AnswerAsk; onAnswer?: (ask: AnswerAsk) => void }) {
  if (onAnswer === undefined) return null;
  return (
    <PlaceButton
      says={answerTitle(ask.name)}
      also="Also in its row's menu in the Chats list"
      onPress={() => onAnswer(ask)}
    >
      <Reply />
    </PlaceButton>
  );
}

/**
 * One of the buttons at the end of a task's line. Its press is its own: it never reaches the
 * line, which would go to the task (a menu's row presses itself on a click and on a pointer
 * coming up over it).
 */
function PlaceButton({
  says,
  keys,
  also,
  onPress,
  children,
}: {
  says: string;
  /** The line's own key for the same thing, said in the tooltip: where the keyboard's way to
   *  it is learned. None for a move the line has no key for. */
  keys?: string;
  /** Where else the same thing is, for a button with no key on its line. */
  also?: string;
  onPress: () => void;
  children: ReactNode;
}) {
  const own = (event: { stopPropagation: () => void }) => event.stopPropagation();
  // The quiet ghost (#3214): a mark at the end of the line, quiet until the pointer is on it. Its
  // tooltip is its name, and under it the line's own key, or where else the same thing is.
  return (
    <IconButton
      variant="quiet"
      size="sm"
      className="tasks-menu-place"
      icon={children}
      aria-label={says}
      tooltip={keys !== undefined ? `${keys} on its line` : also}
      tabIndex={-1}
      data-says={says}
      onPointerDown={own}
      onPointerUp={own}
      onPress={onPress}
    />
  );
}

/** How long a chat has been in its state, where this window saw it come into it: the Chats
 *  list's own clock and its own words (`stateClock.ts`, `chatsList.sinceSaid`). */
function Since({ clock, session }: { clock: StateClock; session: number }) {
  const seconds = useStateSince(clock, session);
  if (seconds === null) return null;
  return (
    <>
      {" "}
      <Tooltip tip="How long it has been in this state, as this window saw it">
        <span className="since">{sinceSaid(seconds)}</span>
      </Tooltip>
    </>
  );
}

/**
 * **What the open menu's chats used** (#1500): asked as the menu opens, and again when a chat
 * of it changes state while it is open (a turn ending is when a harness's figure moves). Never
 * on a timer, and never while the menu is closed. An answer to an ask that was replaced is
 * dropped.
 */
function useUsed(
  read: UsedReader | undefined,
  open: boolean,
  lines: readonly Line[],
  kinds: readonly (string | undefined)[],
): TasksUsed | undefined {
  const [figures, setFigures] = useState<TasksUsed>();
  const asked = JSON.stringify(askOf(lines));
  const moved = kinds.join(",");
  useEffect(() => {
    if (read === undefined || !open) return;
    let gone = false;
    void read(JSON.parse(asked) as UsedAsk).then((used) => {
      if (!gone && used !== undefined) setFigures(used);
    });
    return () => {
      gone = true;
    };
  }, [read, open, asked, moved]);
  // The last answer stays drawn until the next lands, so a line's figure does not blink out
  // as a state moves; a line it does not name says nothing.
  return open ? figures : undefined;
}
