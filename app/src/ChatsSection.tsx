// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { CHAT_ROW, PickUpHere } from "./paneDrag";
import {
  isValidElement,
  memo,
  useCallback,
  useContext,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { AnchoredPopover } from "./ui/popover/Popover";
import { Bell, Ellipsis, Hand, LayoutGrid, Plus, Search } from "lucide-react";
import {
  BROWSERS_SHOW,
  besideId,
  RENAMES_ON_F2,
  renameId,
  ONLY_A_TASK_OPENS_BESIDE,
  taskStopId,
  DISPATCHES_SHOW,
  type Catalogued,
  type Offer,
  type TaskEndWay,
} from "./actions";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "./ui/menu/Menu";
import { TaskEndConfirm, type TaskEndInline } from "./TaskEnd";
import type { AtLimit, FinishedTask } from "./bindings";
import { HelpersSaid } from "./ExplorerChats";
import { ChatDoingLine } from "./ChatRowActivity";
import { aCardIsUp, cardDown, sessionCardIsUp, cardUp, skipsTheRest, useCardUp } from "./chatCard";
import { ChatShownState } from "./ChatRows";
import { ChatPersonaState, useShownState } from "./ChatPersonaState";
import { Grip } from "./ui/grip/Grip";
import { sameList, useChatsHere, useChatsSelect, type ChatStates } from "./chatState";
import { useTokensOnHover } from "./tasksUsed";
import {
  arranged,
  asking,
  filters,
  found,
  liveBelow,
  matchesFinished,
  sessionOrder,
  rowTimeSaid,
  sinceSaid,
  stamped,
  type Filter,
  type Rank,
} from "./chatsList";
import { useChatsListPrefs } from "./chatsListPrefs";
import { inScope, keepScope, keptScope, SCOPES, settleScope, type Scope } from "./chatsScope";
import { Choice } from "./settings/components";
import { ScopeSwitch } from "./ScopeSwitch";
import { foldsToAdopt, keepFolds, keptFolds } from "./chatFolds";
import {
  ASKED_BY_YOU,
  cameFromSaid,
  handedOff,
  handedOffSaid,
  needing,
  parentsIn,
  unfolded,
  type ChatRow,
} from "./chatsTree";
import { finishedRows, isFinishedLineId } from "./FinishedTasks";
import type { TaskFacts } from "./shownState";
import { standingOfRows } from "./sessionTasks";
import { useTaskCount, useTaskCountSaid } from "./TasksBelow";
import { browserRows, type BrowserShown } from "./ChatBrowsersShown";
import { HeadingOffer } from "./PanelSection";
import { AllChatsSwitch } from "./AllChats";
import { tasksIn } from "./taskBuckets";
import { catalogueMenu, moreForSaid } from "./Menus";
import { menuIsEmpty } from "./menuModel";
import { askForAMenu } from "./ui/menu/ContextMenu";
import { useRevealedTask, type Reveal } from "./revealTask";
import { stateClock, useNow, useStateSince, type StateClock } from "./stateClock";
import { onAMac } from "./mac";
import { askSettingsLink } from "./settings/links";
import { DISPATCH } from "./settings/dispatch";
import { Button, IconButton } from "./ui/button/Button";
import { SandboxMark, type SandboxSaid } from "./SandboxMark";
import { WrappingUp } from "./NeedsYou";
import { TextField } from "./ui/field/Field";
import { PanelSection as Section, PanelTitle } from "./ui/panel/Panel";
import { Count } from "./ui/badge/Badge";
import { useQuestionStanding, useQuestionsHere } from "./QuestionsHere";
import {
  askingBelow,
  askingSaid,
  questionWords,
  waitingOnYou,
  type QuestionStanding,
} from "./taskQuestions";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { Tree, TreeRow, TreeSection, useRowDescription, type Key } from "./ui/tree/Tree";
import { RowBelow, RowLabel, RowLead, RowTrail } from "./ui/list/Row";
import { useWindowLeft } from "./ui/windowLeft";
import { useHeldStill } from "./heldStill";
import { property } from "./theme/theme";

/** What a chat's state is drawn from (`ChatShownState`): its row's and its card's alike. */
type ShownFacts = {
  session: number;
  shell: boolean;
  report: TaskFacts["report"] | null;
  outcome: string | null;
  harness: string | null;
};

/** The hand of a chat below that needs you, in the needs-you colour at the words' size. */
const HAND: CSSProperties = {
  width: "1em",
  height: "1em",
  color: `var(${property("needs-you.base")})`,
};

/** A row's quiet mark (#4759), the faint hand of a task that can't report asks or the bell of a
 *  note in the Notices centre: at the words' size, in the row's quiet text colour
 *  (`ui-text-secondary`), never the needs-you colour. */
const QUIET_MARK: CSSProperties = { width: "1em", height: "1em" };

/** What a row says of a chat it announced and the person has not looked at (#4759). */
export const NEW_SAID = "new";

/** What a row's mark for a note in the Notices centre says (#4759). */
export const NOTED_SAID = "A notice about it is in the Notices centre";

/** What the faint hand on a row says (#4759). */
export const CANT_REPORT_SAID = "Can't report asks: look at it from time to time";

/** No rows of the catalogue: what a section drawn on its own, in a test, offers. */
const NO_OFFERS: Catalogued = new Map();

/** No finished tasks: what a section drawn without them lists. */
/** No row is on screen: what the clock's reader is told once the list is gone. */
const NOTHING_DRAWN: ReadonlySet<number> = new Set();

const NONE_FINISHED: ReadonlyMap<number, FinishedTask[]> = new Map();
const NO_TASKS: readonly FinishedTask[] = [];
const NOT_REOPENED = () => Promise.resolve<string | undefined>(undefined);
const NOT_CLEARED = () => undefined;

/** The filter's chips: the rank each asks for (`chatsList.rankOf`), as each is said. */
const CHIPS: readonly { rank: Rank; says: string }[] = [
  { rank: 0, says: "needs you" },
  { rank: 1, says: "working" },
];

/** The chips as the set's options: a rank's value is its digit. */
const CHIP_OPTIONS = CHIPS.map((chip) => ({ value: String(chip.rank), label: chip.says }));

/** Where a chat that needs you is, that the scope leaves out, as the line under the filter
 *  says it (#1679). */
const OUTSIDE_SCOPE: Record<Exclude<Scope, "all">, { one: string; many: string }> = {
  workspace: { one: "in another workspace", many: "in other workspaces" },
};

/** What the list is drawn from that the chats' own moves change: held still while the pointer
 *  or the keyboard is in the list. */
type Moving = {
  /** The sessions, in the order they stand. */
  order: readonly number[];
  /** The sessions with a chat under them that is not over, which are open by themselves. */
  live: readonly number[];
  /** The chats the filter asks for, or nothing while it asks for none. */
  asked: readonly number[] | null;
  /** Every chat listed: one that arrives while the list is held is drawn when it is let go. */
  listed: readonly number[];
};

const sameAsked = (one: readonly number[] | null, other: readonly number[] | null) =>
  one === other || (one !== null && other !== null && sameList(one, other));

/** No restart has anything to say. */
const NO_RESTARTS: Readonly<Record<number, string>> = {};

/** No chat that needs you has had the rows above it opened yet. */
const NONE_OPENED: ReadonlySet<number> = new Set();

/** A chat with no browser (#2024). */
const NO_BROWSER_ROWS: readonly BrowserShown[] = [];

/** No chat ids. */
const NO_IDS: ReadonlySet<string> = new Set();

/** No folds set by hand. */
const NO_FOLDS: ReadonlyMap<number, boolean> = new Map();

/** What a key on a row asks for beside opening it. */
type Asked = "beside" | "stop" | "rename";

/** The catalogue's row each of a chat row's own keys presses. */
const ROW_OF_ASKED: Readonly<Record<Asked, (session: number) => string>> = {
  beside: besideId,
  stop: taskStopId,
  rename: renameId,
};

/** **What a chat's row says of its dispatches waiting on memory** (#1617): how many. */
function onMemoryWords(waiting: number): string {
  return waiting === 1 ? "1 dispatch waits on memory" : `${waiting} dispatches wait on memory`;
}

/** The whole sentence, on the line's hover, in the row's card and its description: what they
 *  wait for, and where they are named. */
export const ON_MEMORY_SAID =
  "This machine is short on memory: they start by themselves once it frees, and the chat that asked is told when each starts or gives up. The Dispatches tab lists them under Not started.";

/**
 * **The running chats of the focused workspace, in one tree** (#1447, #1655), in the left
 * region above the explorer. The explorer answers what is running at each place in this
 * workspace; this answers who is doing what, and which chat started which.
 *
 * **It follows the workspace in the strip** (#1655, ADR 0038's 2026-10-10 amendment): a tree is
 * listed where its top row works, with every task below it wherever that task works
 * (`chatsScope.ts`), and a chat started at the plane root is the root's. **A switch at the top
 * picks the scope** (#1679): This tab lists the chats of the tab in front, as the tab's chip
 * counts them; Workspace, the default, the focused workspace's; All, every workspace's. The pick
 * is kept for the project. A chat the scope leaves out that needs you is never left out
 * silently: the line under the filter names it and goes to it, as it does for one the filter
 * hides, and the title bar's queue lists it as ever.
 *
 * **A row is a way to the chat.** Pressing one, or Enter on it, shows its chat (`onOpen`, the
 * one way a row opens): a task has no tab of its own, and is shown inside the tab of the
 * session that asked for it (#1486). Space opens a task beside the session that asked for it,
 * split from that chat's pane (#1489), and Delete on a task asks to stop it: both are rows of the
 * window's catalogue, and one that cannot run says why here.
 *
 * **At fifty chats** (#1499). The sessions that need you stand first, then the ones at work,
 * then the rest, and a session stands where the most urgent chat under it would; the chats
 * under a session stay in the order they were started. A session with chats under it is open
 * while one of them is not over, and folds by itself when all are, saying how its finished
 * tasks ended on its own row; **a fold set by hand wins** until it is set again. A filter over
 * the list finds a chat by its name, persona, workspace or state, keeps the rows above a match,
 * and says how many it hides. A row is one line, and the rest of what it knows is its card
 * (#1675, `Row`).
 *
 * **No row is moved under a resting pointer.** While the pointer is over the list, or the
 * keyboard is in it, the order, the folds the list makes by itself, the filter's answer and
 * the chats that arrive are held as they were, and are brought up to date when both have left,
 * or when the window stops being the one in use. A row is the same height whatever it comes to
 * say, and what the section says about the filter or a key has a line of its own that is
 * always there. What the person does themselves (a fold, a word typed in the filter) is
 * applied at once. **One thing is not held: a chat that ends.** Its row goes, and the rows
 * below it move up, because a row kept for a chat that is gone would be a way to nothing.
 *
 * **A chat that needs you is never filtered away silently.** Where the filter hides one, the
 * line under the filter says so and has a button that goes to it. And a filter opens every row
 * above what it found, whatever fold was set by hand, which is back when the filter is cleared.
 *
 * **A row says its chat's state by its mark, and the word in its card** (#1484, #1675):
 * `ChatShownState`, which the explorer's rows draw too.
 *
 * **The hand rolls up** (#1448). A chat that needs you wears it on its own row, as its state's
 * mark, and so does every row above it, where it is a button that goes to that chat. A row
 * with chats under it folds, and a folded row still wears the hand for what it hides, so a
 * fold never hides a chat that needs you. **A task that comes to need you opens the rows above
 * it** (#1675), once, whatever fold was set by hand.
 *
 * **A handoff is a row of its own at the top, never under the chat it came from** (#1492,
 * V100-69): the work moved, and its chat is a session with its own tab. Its card says `from
 * <chat>`, and that chat's card says `handed off to <chat>`; its menu goes there. Both name the
 * other chat as its own row does, so a rename is followed. **A task the person asked for
 * themselves is marked `asked by you`** in its card (V100-70).
 *
 * **A task that has finished stays under the chat that asked** (#1485), as a finished entry
 * and not a chat: its program has ended (`FinishedTasks`). They are drawn under their chat's
 * row, after the chats still running under it, and are hidden with them when the row is folded.
 *
 * **A row's menu stops its chat** (#1448): Stop, and Stop with everything below it, from the
 * window's one catalogue, as a tab's menu has them. **A task's has its own two rows instead**
 * (#1488): Stop and get its report, and Close now.
 *
 * **A chat moving redraws its own marks and no row** (SC-3). The rows are held, on plain
 * values, and each state mark reads its own chat's state, so fifty chats cost one mark per
 * move. The hands are read here from the queue, which keeps its identity until it changes, and
 * the order, the folds and the filter's answer are each read through the store's selector, so
 * the list is drawn again only when one of them changes: a reorder is the one thing that moves
 * rows.
 */
export function ChatsSection({
  rows: every,
  here,
  front,
  wrapping,
  sandboxOf,
  besides,
  noted,
  onOpen,
  offers = NO_OFFERS,
  onPress,
  stopping,
  restarts = NO_RESTARTS,
  finished = NONE_FINISHED,
  onClearFinished = NOT_CLEARED,
  onReopen = NOT_REOPENED,
  clock: given,
  onDrawn,
  reveal,
  onRevealed,
  onLookFinished,
  ending,
  onEndTask,
  onEndConfirm,
  onChanges,
  browsers,
  kept,
  onBrowser,
  anyBrowser = false,
  heading,
  onAllChats,
}: {
  rows: readonly ChatRow[];
  /** The workspace in the strip, by the word a row says for where it works: the rows listed
   *  are the trees that started there (#1655). Left out, every workspace's. */
  here?: string;
  /** The chat in front, whose row is the current one: the focused pane's (#4757). */
  front?: number;
  /** The chats in the other panes of the tab in front (#4757): their rows are marked faintly,
   *  so a press that would jump to one is told before it is pressed. */
  besides?: ReadonlySet<number>;
  /** The chats in no pane with a note in the Notices centre (#4759): their rows are marked. */
  noted?: ReadonlySet<number>;
  /** A row was pressed: go to that chat. A task is shown in the pane it returns to (#1486).
   *  The one way a row opens. */
  onOpen: (session: number) => void;
  /** The catalogue as it stands, by id: what each row's menu reads its rows from. */
  offers?: Catalogued;
  /** Carries out a row of a menu. */
  onPress?: (offer: Offer) => void;
  /** The chats being stopped, whose rows say so. */
  stopping?: ReadonlySet<number>;
  /** The chats wrapping up, being smart-closed (ADR 0064): each row breathes (#4760). */
  wrapping?: ReadonlySet<number>;
  /** How much each chat's sandbox holds (#2071), which its row says (#4760). */
  sandboxOf?: (session: number) => SandboxSaid | null;
  /** What a restart asked for from a row says, by chat: why it was refused, or why it waits
   *  (#1462). Said on the row of a chat with no tab, which has no pane to say it on. */
  restarts?: Readonly<Record<number, string>>;
  /** Each chat's finished tasks, by its number (#1485). */
  finished?: ReadonlyMap<number, FinishedTask[]>;
  /** Clear finished: takes those rows away, and nothing else. */
  onClearFinished?: (ids: string[]) => void;
  /** Reopens a finished task as an ordinary chat; answers why not, where it could not. */
  onReopen?: (task: FinishedTask) => Promise<string | undefined>;
  /**
   * The project's clock of how long each chat has been in its state, where the window holds
   * one (#1487): the window reads it, so it runs while this list is not drawn, and a tab's
   * menu says the same times. Left out, the list keeps a clock of its own.
   */
  clock?: StateClock;
  /** Tells whoever reads that clock which chats' rows are on screen (`StateClock.read`): this
   *  list's rows as they are drawn, and none once it is gone. */
  onDrawn?: (drawn: ReadonlySet<number>) => void;
  /** A finished task to bring into view: one that failed, when the person goes to its
   *  needs-you item (#1491). */
  reveal?: Reveal;
  /** What became of `reveal`: its row was shown and has the keyboard, or it could not be
   *  shown and the list said so. Told once for each asking. */
  onRevealed?: (reveal: Reveal, shown: boolean) => void;
  /** The person opened a finished row to read it: a failed one has then been looked at. */
  onLookFinished?: (task: FinishedTask) => void;
  /** The second step of ending a task, while it is asked on that task's row (#1488). */
  ending?: TaskEndInline;
  /** A way to end a task was pressed on its row: its menu, or Delete. Asked about on the row. */
  onEndTask?: (session: number, way: TaskEndWay) => void;
  /** The second step was answered: end it, or keep it. */
  onEndConfirm?: (yes: boolean) => void;
  /** Opens what a finished task changed, in a tab of its own (#1511). */
  onChanges?: (task: FinishedTask) => void;
  /** Each chat's browsers, by its number (#2024): rows under its own. */
  browsers?: ReadonlyMap<number, readonly BrowserShown[]>;
  /** The pinned browsers whose chat closed (#2022): rows of their own, after the chats. */
  kept?: readonly BrowserShown[];
  /** The project has a browser the Browsers grid draws (#2027): its button is on the heading. */
  anyBrowser?: boolean;
  /** A browser's row was pressed: open its view. No rows are drawn without it. */
  onBrowser?: (shown: BrowserShown) => void;
  /** **What its title says** (R123, #2917): the workspace and, after it, the project. Absent,
   *  "Chats". The section is named Chats either way. */
  heading?: ReactNode;
  /** **The switch to All chats** (#6013): every chat in every open project, a mode of this
   *  sidebar. Without it, the head has no switch. */
  onAllChats?: () => void;
}) {
  const prefs = useChatsListPrefs();
  const chats = useChatsHere();
  /** Which chats are listed (#1679): the person's own pick, kept for the project. */
  const [picked, setPicked] = useState<Scope>(() => keptScope(chats.plane));
  const pick = (to: Scope) => {
    setPicked(to);
    keepScope(chats.plane, to);
  };
  // A pick web storage kept before the layout file did goes into the file, once (#1696).
  useEffect(() => settleScope(chats.plane), [chats.plane]);
  /** The scope the list is drawn in: every chat where the caller names no workspace. */
  const scope: Scope = here === undefined ? "all" : picked;
  const rows = useMemo(
    () => (scope === "all" || here === undefined ? every : inScope(every, here)),
    [every, here, scope],
  );
  const [text, setText] = useState("");
  const [ranks, setRanks] = useState<readonly Rank[]>([]);
  /** The state chips' group, by the set's id for it. */
  const chipsId = useId();
  const filter = useMemo<Filter>(() => ({ text, ranks }), [text, ranks]);
  const filtering = filters(filter);
  /** The folds the person set, by chat: true is folded. Kept across a reload of the window by
   *  number, and across a launch by the chat's id (`chatFolds.ts`, #1459, #1687). A chat that
   *  is not here folds and opens by itself. */
  const [hand, setHand] = useState<ReadonlyMap<number, boolean>>(() => keptFolds(chats.plane));
  /** The listed chats' ids, by number: what their folds are kept by across a launch. */
  const listedIds = useMemo(
    () => new Map(every.flatMap((row) => (row.id ? [[row.session, row.id] as const] : []))),
    [every],
  );
  /** The ids whose kept folds this list has put back already, for the project they are of. A
   *  chat's kept fold is put back once, as it is first listed; from then on it is the person's. */
  const [adopted, setAdopted] = useState<{ plane?: string; ids: ReadonlySet<string> }>(() => ({
    plane: chats.plane,
    ids: new Set(),
  }));
  const adoptedIds = adopted.plane === chats.plane ? adopted.ids : NO_IDS;
  if ([...listedIds.values()].some((id) => !adoptedIds.has(id))) {
    const back = foldsToAdopt(chats.plane, listedIds, adoptedIds);
    setAdopted({ plane: chats.plane, ids: new Set([...adoptedIds, ...listedIds.values()]) });
    if (back.size > 0)
      setHand((was) => {
        const set = new Map(was);
        for (const [session, shut] of back) if (!set.has(session)) set.set(session, shut);
        return set;
      });
  }
  /** The folds the person set while a filter is on, which last as long as the filter does: a
   *  filter opens every row above what it found, whatever `hand` says, and leaves `hand` be. */
  const [handFiltered, setHandFiltered] = useState<ReadonlyMap<number, boolean>>(NO_FOLDS);
  if (!filtering && handFiltered.size > 0) setHandFiltered(NO_FOLDS);
  /** Why a key pressed on a row did nothing, until the next key. */
  const [said, setSaid] = useState<string>();

  /** The finished tasks the filter asks for, by the chat that asked for them. */
  const finishedFound = useMemo(() => {
    const by = new Map<number, FinishedTask[]>();
    if (!filtering) return by;
    for (const [session, tasks] of finished) {
      const asked = tasks.filter((task) => matchesFinished(task, filter));
      if (asked.length > 0) by.set(session, asked);
    }
    return by;
  }, [finished, filter, filtering]);

  // What the chats' own moves change, each read through the selector: a move that changes
  // none of them draws nothing here.
  // Every row's state from the one pass (`standingOfRows`), so the order, the folds and the
  // filter read the word each row draws, `waiting on n tasks` included (#1491).
  const kindsOf = (states: ChatStates) => {
    const stood = standingOfRows(states, rows);
    return (row: ChatRow) => stood.get(row.session)?.shown?.kind;
  };
  const order = useChatsSelect(
    chats,
    (states) => sessionOrder(rows, kindsOf(states), prefs.grouped),
    sameList,
  );
  const live = useChatsSelect(chats, (states) => liveBelow(rows, kindsOf(states)), sameList);
  const asked = useChatsSelect(
    chats,
    (states) => {
      if (!filtering) return null;
      const stood = standingOfRows(states, rows);
      // A chat's number asks for it alone (#6213); words find finished tasks too.
      const { byNumber, asks } = asking(rows, filter);
      return rows
        .filter(
          (row) =>
            asks(row, stood.get(row.session)?.shown) ||
            (!byNumber && finishedFound.has(row.session)),
        )
        .map((row) => row.session);
    },
    sameAsked,
  );
  const listed = useMemo(() => rows.map((row) => row.session), [rows]);
  /** **How many chats each chip would leave** (#3484, the prototype's `Needs you 2`): the scope's
   *  chats that the typed words find, in that chip's rank, read as the filter reads them. */
  const chipCounts = useChatsSelect(
    chats,
    (states) => {
      const stood = standingOfRows(states, rows);
      return CHIPS.map((chip) => {
        const { asks } = asking(rows, { text, ranks: [chip.rank] });
        return rows.filter((row) => asks(row, stood.get(row.session)?.shown)).length;
      });
    },
    sameList,
  );
  const chipOptions = useMemo(
    () =>
      CHIP_OPTIONS.map((option, at) => ({
        ...option,
        // None where there are none, as the prototype draws an empty chip.
        count: chipCounts[at] === 0 ? undefined : chipCounts[at],
      })),
    [chipCounts],
  );

  // **Held still while the pointer is over the list or the keyboard is in it** (V100-47): no
  // row moves under a click. Taken as the pointer or the keyboard comes in, let go when both
  // have left, or the window stops being the one in use: the shared hold (`useHeldStill`,
  // #3594), which looks for the focus after every redraw. Off while there is no chat to list:
  // the list is not drawn then, so nothing would say the pointer left it. **While a row's card
  // is up, a pointer leaving the list is let go only after the card's own wait** (#3773): the
  // card stands a step beside its row, over the view's inset, which is not the list, so the
  // pointer crossing to it leaves the list for that moment; what the card is drawn in is the
  // list's, so arriving there keeps the hold.
  const { holding: resting, held: holdBy } = useHeldStill<HTMLDivElement>(
    [rows, finished],
    rows.length > 0,
    toTheCard,
  );
  // **What the person used last, the pointer or the keyboard** (#1675): a row's card comes up
  // on the keyboard resting on it, and not on a focus a press left there.
  const pointed = useRef(false);
  useEffect(() => {
    const press = () => {
      pointed.current = true;
    };
    const key = () => {
      pointed.current = false;
    };
    document.addEventListener("pointerdown", press, true);
    document.addEventListener("keydown", key, true);
    return () => {
      document.removeEventListener("pointerdown", press, true);
      document.removeEventListener("keydown", key, true);
    };
  }, []);
  const pointedLast = useCallback(() => pointed.current, []);
  const [held, setHeld] = useState<Moving | null>(null);
  const moving: Moving = { order, live, asked, listed };
  // **A workspace focused or a scope picked is the person's doing** (#1655, #1679): its rows
  // are theirs to see at once, as a filter's are, and not held as the last ones were.
  const listing = scope === "all" ? null : `workspace:${here}`;
  const [heldScope, setHeldScope] = useState(listing);
  if (heldScope !== listing) {
    setHeldScope(listing);
    setHeld(resting ? moving : null);
  } else if (resting && held === null) setHeld(moving);
  if (!resting && held !== null) setHeld(null);
  const now = resting && held !== null ? held : moving;
  /** The filter was changed by the person: its answer is theirs to see at once. */
  const refilter = (how: () => void) => {
    how();
    setHeld(null);
  };
  const clear = () =>
    refilter(() => {
      setText("");
      setRanks([]);
    });

  /** What each chat's finished tasks come to: whether one of them stands alone, which keeps
   *  its chat open. A folded row counts them with its tasks (`TasksFolded`, #1491), and its
   *  card says how each ended. */
  const ended = useMemo(
    () =>
      new Map(
        [...finished]
          .filter(([, tasks]) => tasks.length > 0)
          .map(([session, tasks]) => [session, { alone: tasks.some((task) => !task.folds) }]),
      ),
    [finished],
  );
  /** The rows the list is drawn from: every chat, less the ones that arrived while it is
   *  held. A chat that ended is not in `rows`, and is not kept. */
  const steady = useMemo(() => {
    if (now.listed === listed) return rows;
    const known = new Set(now.listed);
    return rows.filter((row) => known.has(row.session));
  }, [rows, listed, now.listed]);
  /** The rows in the order they stand, less what the filter hides. */
  const base = useMemo(() => {
    const inOrder = arranged(steady, now.order);
    return stamped(now.asked === null ? inOrder : found(inOrder, new Set(now.asked)));
  }, [steady, now.order, now.asked]);
  const parents = useMemo(() => parentsIn(base), [base]);
  /**
   * Whether each row's own rows are drawn under it, for a row that has some (a chat or a
   * finished task). **A fold set by hand wins over the folds the list makes** (V100-48):
   * without one, a row is open while a chat under it is not over or a finished task of its
   * own stands alone. **A filter wins over both**: every row above what it found is open, so
   * what it found is drawn, and only a fold set while it is on shuts one again.
   */
  const opens = useMemo(() => {
    const alive = new Set(now.live);
    const by = new Map<number, boolean>();
    for (const row of base) {
      const { session } = row;
      // **A chat's browsers are rows under it** (#2024, #3198): a chat with one folds, and one
      // whose browsers are all it holds is open until it is folded by hand.
      const browsing = onBrowser !== undefined && (browsers?.get(session)?.length ?? 0) > 0;
      if (!parents.has(session) && !ended.has(session) && !browsing) continue;
      const onlyBrowsers = !parents.has(session) && !ended.has(session);
      const set = now.asked !== null ? handFiltered.get(session) : hand.get(session);
      by.set(
        session,
        set !== undefined
          ? !set
          : onlyBrowsers ||
              (now.asked !== null
                ? parents.has(session) || finishedFound.has(session)
                : alive.has(session) || ended.get(session)?.alone === true),
      );
    }
    return by;
  }, [
    base,
    parents,
    ended,
    hand,
    handFiltered,
    finishedFound,
    now.live,
    now.asked,
    browsers,
    onBrowser,
  ]);
  const folded = useMemo(
    () => new Set([...opens].filter(([, open]) => !open).map(([session]) => session)),
    [opens],
  );
  const drawn = useMemo(() => unfolded(base, folded), [base, folded]);
  /** **How many rows each workspace's name stands over** (#3484), by the session it is drawn
   *  above: the rows drawn until the next workspace's, as the frame's group counts its rows. */
  const groupCounts = useMemo(() => {
    const of = new Map<number, number>();
    let head: ChatRow | undefined;
    for (const row of drawn) {
      if (row.level === 1 && row.workspace !== head?.workspace) head = row;
      if (head !== undefined) of.set(head.session, (of.get(head.session) ?? 0) + 1);
    }
    return of;
  }, [drawn]);
  const queue = useChatsSelect(chats, (states) => states.needsYou);
  /** The project's task questions, as the asks registry lists them (#4564). */
  const questions = useQuestionsHere();
  /** **Who waits on the person**: the core's queue, and each task question that has reached
   *  them. A question with its asking chat is in neither, so it draws no hand above its row
   *  and opens no fold. */
  const needsYou = useMemo(() => {
    const reached = waitingOnYou(questions).filter((session) => !queue.includes(session));
    return reached.length === 0 ? queue : [...queue, ...reached];
  }, [queue, questions]);
  const leads = useMemo(() => needing(rows, needsYou), [rows, needsYou]);
  /** How many task questions are with their asking chat under each row: a folded row says so. */
  const askingUnder = useMemo(() => askingBelow(rows, questions), [rows, questions]);
  const byNumber = useMemo(() => new Map(rows.map((row) => [row.session, row])), [rows]);
  /**
   * **A task that comes to need you opens the rows above it** (#1675, B-6), so it is never
   * hunted for: once, as it starts to ask, the folds set by hand over it are taken off, and the
   * list's own folds already leave open a row with a chat under it that is not over. A fold set
   * again afterwards is the person's and holds, with the hand rolled up on it (#1448).
   */
  const [opened, setOpened] = useState<ReadonlySet<number>>(NONE_OPENED);
  // A chat counts as opened for once it has a row here: one the board says asks before the
  // list has its row is opened for when the row comes. One that stops asking is forgotten,
  // so its next ask opens its rows again.
  const asks = needsYou.filter((session) => !opened.has(session) && byNumber.has(session));
  if (asks.length > 0 || [...opened].some((session) => !needsYou.includes(session))) {
    setOpened(new Set(needsYou.filter((session) => opened.has(session) || byNumber.has(session))));
    const above = new Set<number>();
    for (const asking of asks) {
      let at = byNumber.get(asking);
      while (at !== undefined && at.level > 1 && at.parent !== null) {
        above.add(at.parent);
        at = byNumber.get(at.parent);
      }
    }
    const unfold = (was: ReadonlyMap<number, boolean>) => {
      if (![...above].some((session) => was.get(session) === true)) return was;
      const set = new Map(was);
      for (const session of above) if (set.get(session) === true) set.delete(session);
      return set;
    };
    if (above.size > 0) {
      setHand(unfold);
      setHandFiltered(unfold);
    }
  }
  /** Where each chat's work went by a handoff, the newest first (#1492). */
  const went = useMemo(() => handedOff(rows), [rows]);
  /**
   * **The chats that need the person and that the filter hides**, longest waiting first
   * (#1499): their own rows and every row above them are filtered out, so no hand is drawn
   * for them anywhere in the list. Read off the queue as it stands, never held.
   */
  const hiddenNeeding = useMemo(() => {
    if (now.asked === null) return [];
    const kept = new Set(base.map((row) => row.session));
    return needsYou.filter((session) => byNumber.has(session) && !kept.has(session));
  }, [base, byNumber, needsYou, now.asked]);
  /** Every workspace's rows by number, for what is said of a chat this workspace's list
   *  leaves out (#1655). */
  const everyByNumber = useMemo(() => new Map(every.map((row) => [row.session, row])), [every]);
  /** **The chats that need the person in the workspaces not listed** (#1655), longest waiting
   *  first: never left out silently. */
  const elsewhereNeeding = useMemo(
    () =>
      rows === every
        ? []
        : needsYou.filter((session) => everyByNumber.has(session) && !byNumber.has(session)),
    [rows, every, needsYou, everyByNumber, byNumber],
  );

  // How long each chat has been in its state, as this window saw it: read off every chat, drawn
  // or not, so a row that was folded away says the same time when it is drawn again. One clock
  // per project: chats are numbered per project.
  const { store, plane } = chats;
  /** **Where a limit is changed** (#1498): Settings › Project › Dispatch, which a row at its
   *  task limit links to. */
  const limits = useCallback(() => {
    if (plane !== undefined) askSettingsLink(plane, { group: DISPATCH });
  }, [plane]);
  const own = useMemo(() => {
    void plane;
    return stateClock();
  }, [plane]);
  const clock = given ?? own;
  const onScreen = useMemo(() => new Set(drawn.map((row) => row.session)), [drawn]);
  useEffect(() => {
    // The window's clock is read by the window, which is told what is on screen here.
    if (given !== undefined) return;
    const read = () => clock.read(store.statesFor(plane), rows, Date.now(), onScreen);
    read();
    return store.subscribe(read);
  }, [store, plane, clock, given, rows, onScreen]);
  useEffect(() => {
    if (given === undefined || onDrawn === undefined) return;
    onDrawn(onScreen);
    return () => onDrawn(NOTHING_DRAWN);
  }, [given, onDrawn, onScreen]);

  // What the person folded outlives a reload of the window (#1459), and a relaunch (#1687).
  useEffect(() => keepFolds(plane, hand, listedIds), [plane, hand, listedIds]);
  const fold = useCallback(
    (session: number, shut: boolean) => {
      (filtering ? setHandFiltered : setHand)((was) => {
        if (was.get(session) === shut) return was;
        const set = new Map(was);
        set.set(session, shut);
        return set;
      });
    },
    [filtering],
  );
  // **A way to end a task pressed on its row is asked about on that row** (#1488): the
  // window is told where the press was made. Every other row of a menu is carried out as it is.
  const press = useCallback(
    (offer: Offer) => {
      if (offer.does.verb === "endTask" && onEndTask !== undefined)
        onEndTask(offer.does.session, offer.does.way);
      else onPress?.(offer);
    },
    [onEndTask, onPress],
  );
  const confirm = useCallback((yes: boolean) => onEndConfirm?.(yes), [onEndConfirm]);
  const act = useCallback(
    (session: number, what: Asked) => {
      // Delete on a task asks to stop it and get its report (#1488, V100-17): the task's own
      // row of the catalogue, which asks first where the task is mid-turn. **Space opens the
      // row's own task beside its session** (#1489): that task's row of the catalogue, so what
      // opens is the row the key was pressed on. A chat that is not a task has neither row,
      // and Space says why.
      // **F2 opens Rename chat** (#5343), the platform's rename key on a focused item: its row.
      const offer = offers.get(ROW_OF_ASKED[what](session));
      if (offer === undefined) {
        if (what === "beside") setSaid(ONLY_A_TASK_OPENS_BESIDE);
        return;
      }
      if (!offer.available) {
        setSaid(offer.reason);
        return;
      }
      press(offer);
    },
    [offers, press],
  );
  const section = useRef<HTMLElement>(null);
  // **A row asked for outside the scope widens it** (#1655, #1679), once for each asking, to
  // the narrowest scope that lists it, so the row can be shown: the switch says so, and the
  // person picks again. It is not the person's pick, so it is not kept.
  const [widenedFor, setWidenedFor] = useState<number>();
  const outsideScope =
    reveal !== undefined &&
    reveal.at !== widenedFor &&
    scope !== "all" &&
    here !== undefined &&
    !byNumber.has(reveal.asker) &&
    everyByNumber.has(reveal.asker);
  if (outsideScope) {
    setWidenedFor(reveal.at);
    setPicked("all");
  }
  useRevealedTask(section, reveal, rows, {
    fold,
    shut: (session) => opens.get(session) === false,
    // A finished row, or a chat's own, that the filter does not ask for. And a chat's own row
    // asked for by the explorer's line (#1490) under any filter: the line counted the tasks
    // under it, and a filter that kept the row could still hide those.
    hides: (asked) =>
      filtering &&
      (asked.task === undefined ||
        !base.some((row) => row.session === asked.asker) ||
        !(finishedFound.get(asked.asker) ?? []).some((task) => task.name === asked.task)),
    unfilter: (name) => {
      clear();
      setSaid(`The filter was taken off to show ${name}.`);
    },
    // The row was shown, or could not be: said to whoever asked, once (#1491).
    shown: (asked) => onRevealed?.(asked, true),
    missed: (asked) => {
      setSaid(`${asked.task ?? "That chat"} has no row to show here.`);
      onRevealed?.(asked, false);
    },
  });
  /** The row the second step is asked on, where it is drawn. */
  const second = ending?.where === "row" ? ending : undefined;
  /** The keyboard left the list: what the last key said is taken down with it. */
  const left = (event: FocusEvent<HTMLDivElement>) => {
    holdBy.onBlur(event);
    const to = event.relatedTarget;
    if (to instanceof Node && event.currentTarget.contains(to)) return;
    setSaid(undefined);
  };
  /** The rows as the tree holds them: each with the rows under it (#3198). */
  const tops = useMemo(() => nested(base), [base]);
  /** **Which "Finished (n)" lines are open** (#1485): this window's own, each folded to start
   *  with, by the chat they are of. */
  const [finishedOpen, setFinishedOpen] = useState<ReadonlySet<string>>(NO_KEYS);
  /** **What the tree draws open** (#3198): every chat `opens` says is, the open "Finished (n)"
   *  lines, and the pinned browsers' row, which never folds. */
  const expanded = useMemo(() => {
    const keys = new Set<Key>([PINNED, ...finishedOpen]);
    for (const [session, open] of opens) if (open) keys.add(session);
    return keys;
  }, [opens, finishedOpen]);
  /** A row was opened or folded, by its twist, the arrows or a press: a chat's fold is set by
   *  hand (`fold`), and a "Finished (n)" line's is this window's. */
  const unfold = (keys: Set<Key>) => {
    for (const [session, open] of opens) if (keys.has(session) !== open) fold(session, open);
    const lines = new Set([...keys].filter(isFinishedLineId));
    if (lines.size !== finishedOpen.size || [...lines].some((key) => !finishedOpen.has(key)))
      setFinishedOpen(lines);
  };
  /**
   * **A row's own keys, before the tree's** (#3198): Space opens a task beside its session,
   * F2 renames its chat, and Delete (Backspace on a Mac) asks to stop a task, each the
   * catalogue's row, which asks first or says why not (`act`). Space is React Aria's choosing
   * key, and nothing is chosen in this tree. A key held down is one press. Only a chat's row's,
   * and only on the row: a key on a control in it is that control's.
   */
  const rowKey = (event: KeyboardEvent<HTMLElement>): boolean => {
    const row = event.target instanceof HTMLElement ? event.target : null;
    if (row?.getAttribute("role") !== "row" || row.dataset.session === undefined) return false;
    const plain = !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
    const asked: Asked | undefined = !plain
      ? undefined
      : event.key === " "
        ? "beside"
        : event.key === "F2"
          ? "rename"
          : row.dataset.task !== undefined &&
              (event.key === "Delete" || (event.key === "Backspace" && onAMac()))
            ? "stop"
            : undefined;
    if (asked === undefined) return false;
    if (!event.repeat) act(Number(row.dataset.session), asked);
    return true;
  };
  /** **A chat's row, with the rows under it** (#3198): its browsers (#2024), the chats it asked
   *  for, then its finished tasks (#1485), which fold away with it. */
  const chatRow = (node: Nested): ReactNode => {
    const { row } = node;
    const lead = leads.get(row.session);
    const asker = row.parent === null ? undefined : byNumber.get(row.parent);
    const handed = went.get(row.session);
    const under = [
      ...(onBrowser === undefined
        ? []
        : browserRows({
            browsers: browsers?.get(row.session) ?? NO_BROWSER_ROWS,
            onOpen: onBrowser,
            offers,
            onPress: press,
          })),
      ...node.under.map(chatRow),
      ...finishedRows({
        session: row.session,
        tasks:
          (now.asked !== null ? finishedFound.get(row.session) : finished.get(row.session)) ??
          NO_TASKS,
        onClear: onClearFinished,
        onReopen,
        onLook: onLookFinished,
        onChanges,
        offers,
        onPress: press,
      }),
    ];
    return (
      <Row
        key={row.session}
        session={row.session}
        name={row.name}
        persona={row.persona}
        workspace={row.workspace}
        task={row.mode === "task"}
        branch={row.branch}
        shell={row.shell}
        report={row.report}
        outcome={row.outcome}
        harness={row.harness}
        runsOn={row.runsOn}
        // A handoff says the chat it came from, by what that chat is called now, or was when
        // it closed. A task says it only once its asker has closed: under its asker's row, the
        // nesting says it.
        from={row.mode === "handoff" ? (asker?.name ?? row.from) : row.orphaned ? row.from : null}
        byYou={row.byYou === true}
        handedToName={handed?.[0].name ?? null}
        handedMore={handed === undefined ? 0 : handed.length - 1}
        askerWaiting={row.askerWaiting === true}
        unseen={row.unseen === true}
        cantReport={row.cantReport === true}
        tab={row.tab}
        current={row.session === front}
        beside={besides?.has(row.session) ?? false}
        noted={noted?.has(row.session) ?? false}
        open={opens.get(row.session) ?? null}
        needs={lead === undefined ? null : lead}
        askingUnder={askingUnder.get(row.session) ?? 0}
        needsName={
          lead === undefined || lead === row.session ? null : (byNumber.get(lead)?.name ?? null)
        }
        stopping={stopping?.has(row.session) ?? false}
        wrapping={wrapping?.has(row.session) ?? false}
        sandbox={sandboxOf?.(row.session) ?? null}
        restartSaid={row.tab ? null : (restarts[row.session] ?? null)}
        asks={second?.session === row.session ? second.says : null}
        answer={second?.session === row.session ? second.answer : null}
        busy={second?.session === row.session && second.busy}
        trouble={second?.session === row.session ? (second.trouble ?? null) : null}
        onConfirm={confirm}
        atLimit={row.atLimit ?? null}
        waitingOnMemory={row.waitingOnMemory ?? null}
        onLimits={limits}
        offers={offers}
        clock={clock}
        onOpen={onOpen}
        onPress={press}
        pointedLast={pointedLast}
      >
        {under.length > 0 ? under : undefined}
      </Row>
    );
  };
  /** The catalogue's new chat, which the view's title line offers (#3342). */
  const newChat = offers.get("chat.new");
  /** The rows the title line's More offers (#3342), those the catalogue offers here. */
  const more = MORE_ROWS.flatMap((id) => {
    const offer = offers.get(id);
    return offer === undefined ? [] : [offer];
  });
  const hidden = steady.length - base.length;
  /** Where a chat the scope leaves out is, as the line says it (#1679). */
  const outside = scope === "all" ? undefined : OUTSIDE_SCOPE[scope];
  /** The first chat the scope leaves out that needs you, the longest waiting. */
  const firstOutside =
    elsewhereNeeding.length > 0 ? everyByNumber.get(elsewhereNeeding[0]) : undefined;
  /** Where the Go button goes: a chat the filter hides that needs you, else one the scope
   *  leaves out that does (#1655, #1679). */
  /** The first chat the filter hides that needs you, the longest waiting. */
  const filteredFirst = hiddenNeeding.length > 0 ? byNumber.get(hiddenNeeding[0]) : undefined;
  const hiddenFirst =
    filteredFirst !== undefined
      ? { row: filteredFirst, why: "which needs you and the filter hides" }
      : firstOutside !== undefined && outside !== undefined
        ? { row: firstOutside, why: `which needs you ${outside.one}` }
        : undefined;
  const goesTo = hiddenFirst?.row;
  /** What the line says of the chats the scope leaves out: only one that needs you, by its
   *  name, whatever the scope (#1679). */
  const elsewhere =
    firstOutside === undefined || outside === undefined
      ? ""
      : elsewhereNeeding.length === 1
        ? `${firstOutside.name} needs you, ${outside.one}.`
        : `${firstOutside.name} and ${elsewhereNeeding.length - 1} more need you, ${outside.many}.`;
  /** What the line under the filter says of it. A chat that needs the person comes first. */
  const hides = !filtering
    ? elsewhere
    : [
        // Named, as the scope's are (#1679): the first is the one Go goes to.
        filteredFirst === undefined
          ? ""
          : hiddenNeeding.length === 1
            ? `${filteredFirst.name} needs you, and the filter hides it.`
            : `${filteredFirst.name} and ${hiddenNeeding.length - 1} more need you, and the filter hides them.`,
        base.length === 0
          ? "No chat matches the filter."
          : hidden === 0
            ? "The filter hides no chat."
            : `The filter hides ${hidden} of ${steady.length} chats.`,
        elsewhere,
      ]
        .filter((one) => one !== "")
        .join(" ");
  return (
    <Section
      view
      ref={section}
      className="chats-section ui-text-body"
      data-testid="chats-section"
      aria-label="Chats"
      // **The Chats view is a view of the panel family** (#3342, the sidebar-views prototype's
      // Q1 A): its title line, then the filter, then the scope, then the chips, held at the top
      // on the panel's own fill while the rows scroll under them.
      top={
        <>
          <div className="chats-top">
            {/* The view's one title, as the sidebar-views prototype's Q1 A draws it (#3342):
                13px, strong, in the primary tier and its own case. */}
            <PanelTitle id="chats-title" className="side-title">
              {heading ?? "Chats"}
            </PanelTitle>
            {/* **All chats** (#6013): the switch to every chat in every open project. */}
            {onAllChats !== undefined && (
              <AllChatsSwitch on={false} onChange={(on) => on && onAllChats()} />
            )}
            {/* **Every browser, live** (#2027): the Browsers grid, while it has a tile. */}
            {anyBrowser && (
              <HeadingOffer offer={offers.get(BROWSERS_SHOW)} onPress={press} mark={LayoutGrid} />
            )}
            {/* **What the filter hides, and why a key did nothing, on the title's line** (#1499,
              DS-11): both status lines are always in the accessibility tree, so what they come
              to say is announced, and they stand beside the title, so saying it moves no row
              and they take no line of their own while they have nothing to say. */}
            {every.length > 0 && (
              <div className="chats-notes ui-text-secondary">
                {hiddenFirst !== undefined && goesTo !== undefined && (
                  <Button
                    size="sm"
                    data-leads-to={goesTo.session}
                    aria-label={`Go to ${goesTo.name}, ${hiddenFirst.why}`}
                    tooltip={`Go to ${goesTo.name}, ${hiddenFirst.why}`}
                    onPress={() => onOpen(goesTo.session)}
                    icon={<Hand aria-hidden="true" />}
                    className="chats-hidden-go"
                  >
                    Go
                  </Button>
                )}
                <Tooltip tip={hides || undefined}>
                  <p
                    className={said === undefined ? "chats-hidden" : "chats-hidden away"}
                    role="status"
                  >
                    {hides}
                  </p>
                </Tooltip>
                <Tooltip tip={said}>
                  <p className="chats-said" role="status">
                    {said ?? ""}
                  </p>
                </Tooltip>
              </div>
            )}
            {/* The view's own action on its title's line (Q1 A): the catalogue's new chat, named
                for what it makes here; the strip's own `+` is the same row, as New tab. */}
            {newChat !== undefined && (
              <IconButton
                size="sm"
                aria-label="New chat"
                tooltip={newChat.available ? "New chat" : newChat.reason}
                isDisabled={!newChat.available}
                onPress={() => press(newChat)}
                icon={<Plus className="node-icon" aria-hidden="true" />}
              />
            )}
            {/* **More** (Q1 A, #3342): the catalogue's other ways to start or look at chats
                here, a new shell and the dispatches, and Settings; each its row, run as a
                press of it is. */}
            {more.length > 0 && (
              <MenuTrigger>
                <IconButton
                  size="sm"
                  aria-label="More"
                  tooltip="More"
                  icon={<Ellipsis className="node-icon" aria-hidden="true" />}
                />
                <Menu
                  aria-label="More for chats"
                  onAction={(key) => {
                    const offer = more.find((one) => one.id === key);
                    if (offer?.available) press(offer);
                  }}
                >
                  {more.map((offer) => (
                    <MenuItem
                      key={offer.id}
                      id={offer.id}
                      textValue={offer.title}
                      isDisabled={!offer.available}
                    >
                      {/* A row it cannot run says why under its words, as New chat's tooltip
                          does. */}
                      <MenuWords
                        said={offer.title}
                        under={offer.available ? undefined : offer.reason}
                      />
                    </MenuItem>
                  ))}
                </Menu>
              </MenuTrigger>
            )}
          </div>
          {every.length > 0 && (
            <div className="chats-filter" role="search" aria-label="Filter the chats">
              {/* **The box's text starts where a top-level chat's name does** (DS-11): its
                glass stands in the column the rows' state marks stand in. */}
              <TextField
                className="chats-filter-box"
                type="search"
                icon={<Search className="chats-filter-glass" aria-hidden="true" />}
                keepsFocus="chats-filter"
                aria-label="Filter chats by name, persona, workspace or state"
                placeholder="Filter chats"
                value={text}
                onChange={(typed) => refilter(() => setText(typed))}
                onKeyDown={(event) => {
                  if (event.key !== "Escape" || !filtering) return;
                  event.preventDefault();
                  event.stopPropagation();
                  clear();
                }}
              />
            </div>
          )}
          {here !== undefined && every.length > 0 && (
            <ScopeSwitch label="Show the chats of" scopes={SCOPES} scope={picked} onPick={pick} />
          )}
          {every.length > 0 && (
            /* The chips: the set's (#626), ONE Tab stop with the arrows between them, named as a
             group so each box is heard as what it narrows the list to. */
            <Choice
              ids={{ id: chipsId, label: "Show only" }}
              kind="chips"
              options={chipOptions}
              checked={new Set(ranks.map(String))}
              onCheckedChange={(value, on) => {
                const rank = CHIPS.find((chip) => String(chip.rank) === value)?.rank;
                if (rank === undefined) return;
                refilter(() =>
                  setRanks((was) =>
                    on
                      ? was.includes(rank)
                        ? was
                        : [...was, rank]
                      : was.filter((one) => one !== rank),
                  ),
                );
              }}
            />
          )}
        </>
      }
    >
      {/* What each restart asked for from a row says, announced as it comes (#1462), beside
          the tree and not in it (#3759), and there whether or not the list draws a row
          (#3886): the rows draw it, and say it. Of every chat listed, whatever the scope, the
          filter and the folds draw: a line comes with its words, never with its row. */}
      <RestartsSaid rows={every} restarts={restarts} />
      {rows.length === 0 ? (
        <p className="empty ui-text-secondary">
          {every.length === 0
            ? "No chats are running in this project."
            : "No chats are running here. All lists the others."}
        </p>
      ) : (
        <div
          {...holdBy}
          className="chats-list"
          onBlur={left}
          // Before the row's own keys: what the last key left said is taken down by the next,
          // unless that is Space again; the hold hears the key as the keyboard being here.
          onKeyDownCapture={(event) => {
            if (event.key !== " ") setSaid(undefined);
            holdBy.onKeyDownCapture(event);
          }}
          onKeyDown={(event) => {
            // Escape in the list takes the filter off, as it does in the filter's own box, and
            // like that one it is taken here: nothing behind the list acts on it too.
            if (event.key !== "Escape" || !filtering) return;
            event.preventDefault();
            event.stopPropagation();
            clear();
          }}
        >
          {drawn.length > 0 && (
            /* **The tree family's tree** (#3198, #2911): a chat's row holds the rows under it,
               its browsers, the chats it asked for and its finished tasks, which fold with it.
               Its one Tab stop is the chat in front (`stopAt`) until the keyboard has been in;
               Space, F2 and Delete are the rows' own keys, heard before the tree's. */
            <Tree
              aria-label="Chats of this project"
              size="sm"
              stopAt={front}
              expandedKeys={expanded}
              onExpandedChange={unfold}
              onRowKey={rowKey}
            >
              {grouped(tops, prefs.grouped).map((group) =>
                group.workspace === null ? (
                  group.nodes.map(chatRow)
                ) : (
                  // **A workspace's sessions stand together** (V100-47, a setting): the tree
                  // family's group, under the frame's group head (#3484), its name and count.
                  <TreeSection
                    key={`group:${group.nodes[0].row.session}`}
                    id={`group:${group.nodes[0].row.session}`}
                    className="chats-group"
                    aria-label={group.workspace}
                    named
                    title={
                      <>
                        <span className="chats-group-name">{group.workspace}</span>
                        <span data-count="">{groupCounts.get(group.nodes[0].row.session)}</span>
                      </>
                    }
                  >
                    {group.nodes.map(chatRow)}
                  </TreeSection>
                ),
              )}
              {/* **Pinned browsers whose chat closed** (#2022): the workspace's now, after
                  every chat, each by what its chat was called, handed to a chat or stopped
                  from its menu. One row over them, which is always open. */}
              {onBrowser !== undefined && kept !== undefined && kept.length > 0 && (
                <TreeRow
                  id={PINNED}
                  textValue="Pinned browsers"
                  className="chat-browsers"
                  alwaysOpen
                  content={<RowLabel className="session">Pinned browsers</RowLabel>}
                >
                  {browserRows({
                    browsers: kept,
                    onOpen: onBrowser,
                    offers,
                    onPress: press,
                    handTo: listed,
                  })}
                </TreeRow>
              )}
            </Tree>
          )}
        </div>
      )}
    </Section>
  );
}

/** A chat's row as the tree holds it: the row, and the rows under it. */
type Nested = { row: ChatRow; under: Nested[] };

/** **The rows nested** (#3198, V100-47): the list keeps a flat array of rows with levels
 *  (`base`), held still as it is; the tree is handed each row with the rows under it. */
function nested(rows: readonly ChatRow[]): Nested[] {
  const top: Nested[] = [];
  const above: Nested[] = [];
  for (const row of rows) {
    const node: Nested = { row, under: [] };
    while (above.length > 0 && above[above.length - 1].row.level >= row.level) above.pop();
    (above.length === 0 ? top : above[above.length - 1].under).push(node);
    above.push(node);
  }
  return top;
}

/** **The sessions in their workspaces' groups** (V100-47's grouped setting, #3484): each run of
 *  sessions of one workspace, or all of them in none. */
function grouped(
  tops: readonly Nested[],
  by: boolean,
): { workspace: string | null; nodes: Nested[] }[] {
  if (!by) return [{ workspace: null, nodes: [...tops] }];
  const groups: { workspace: string; nodes: Nested[] }[] = [];
  for (const node of tops) {
    const last = groups[groups.length - 1];
    if (last?.workspace === node.row.workspace) last.nodes.push(node);
    else groups.push({ workspace: node.row.workspace, nodes: [node] });
  }
  return groups;
}

/** The pinned browsers' row's key (#2022). */
const PINNED = "pinned";

/** No "Finished (n)" line is open. */
const NO_KEYS: ReadonlySet<string> = new Set();

/** What the Chats title line's More holds (#3342), by catalogue id: a new shell, the project's
 *  dispatches, and Settings. */
const MORE_ROWS = ["shell.new", DISPATCHES_SHOW, "settings.show"] as const;

/**
 * How long a row is rested on, by the pointer or the keyboard, before its card comes up
 * (#1675): a pointer running down the list, or the arrows going through it, bring up nothing.
 */
export const CARD_DELAY_MS = 500;

/**
 * How long a card the pointer brought up stays once the pointer leaves its row (#1675): long
 * enough to cross to the card, so its words can be pointed at and read (WCAG 1.4.13, hoverable).
 */
export const CARD_LEAVE_MS = 150;

export { CARD_SKIP_MS } from "./chatCard";

/** How long the list's hold waits for a pointer that left it: the card's own wait while a card is
 *  up, which the pointer may be crossing to, and none while none is. */
function toTheCard(): number {
  return aCardIsUp() ? CARD_LEAVE_MS : 0;
}

/** `said` after what was `before`, each its own sentence where there are two. */
function sentences(before: string, said: string): string {
  if (before === "") return said;
  const stop = (one: string) => (/[.!?]$/.test(one) ? one : `${one}.`);
  return `${stop(before)} ${stop(said)}`;
}

/**
 * **What a restart asked for from a row says, to a screen reader** (#1462, #3759, #3886): one
 * status, mounted with the section and empty until a restart says something, since a region
 * that arrives already full is not announced by every screen reader; only its text changes. A
 * line for each chat listed with a restart said, in the chats' numbers' order, and only a line
 * that comes is announced (`aria-atomic="false"`), not every line again. **Built from every
 * chat listed, not the rows drawn** (the review of #3935): a fold, the filter or the scope
 * drawing a row again would add its line again, and a screen reader would announce old words
 * as new. Beside the tree, which may own its rows alone: the row draws the words under it, for
 * the eye, and says them in its description and its card.
 */
function RestartsSaid({
  rows,
  restarts,
}: {
  rows: readonly ChatRow[];
  restarts: Readonly<Record<number, string>>;
}) {
  const said = rows
    .filter((row) => !row.tab && restarts[row.session] !== undefined)
    .sort((one, other) => one.session - other.session);
  return (
    <div className="hidden-words" role="status" aria-label="Restarts" aria-atomic="false">
      {said.map((row) => (
        <p key={row.session}>{restartLine(row.name, restarts[row.session])}</p>
      ))}
    </div>
  );
}

/** A restart's line in the restarts' status: its words as they are where they name the chat,
 *  else led by which chat's restart it was. */
function restartLine(name: string, words: string): string {
  return words.includes(name) ? words : `Restart of ${name}. ${words}`;
}

/**
 * **Whether a chat's row is drawn as it was** (SC-3, #3198): its plain values, and the rows under
 * it, which are drawn anew with every draw of the list, the same rows with the same values. So a
 * chat moving draws only the rows whose own facts, or whose rows' facts, changed: a row of the
 * tree holds its rows, and a row drawn again is the tree's collection built again.
 */
function sameRow(was: RowProps, now: RowProps): boolean {
  for (const key of Object.keys(now) as (keyof RowProps)[])
    if (key !== "children" && !Object.is(was[key], now[key])) return false;
  return (
    Object.keys(was).length === Object.keys(now).length && sameNodes(was.children, now.children)
  );
}

/** Two drawings of rows: the same rows, by their kind and key, with the same values. */
function sameNodes(was: ReactNode, now: ReactNode): boolean {
  if (Object.is(was, now)) return true;
  if (Array.isArray(was) && Array.isArray(now))
    return was.length === now.length && was.every((one, at) => sameNodes(one, now[at]));
  if (!isValidElement(was) || !isValidElement(now)) return false;
  if (was.type !== now.type || was.key !== now.key) return false;
  const one = was.props as Record<string, unknown>;
  const other = now.props as Record<string, unknown>;
  const keys = Object.keys(other);
  return (
    keys.length === Object.keys(one).length &&
    keys.every((key) =>
      key === "children"
        ? sameNodes(one[key] as ReactNode, other[key] as ReactNode)
        : Object.is(one[key], other[key]),
    )
  );
}

/** What a row says (#3886), each by its kind: what its card keys its lines by. */
type Said = { kind: "needs" | "limit" | "memory" | "restart"; line: string };

/**
 * One chat's row, **a row of the tree family** (#3198), with the rows under it as its children.
 * Held on plain values, so a leaf whose own facts did not change is not drawn again.
 *
 * **One line** (#1675, B-3): its persona's mark with its state on it, its name, and at its end
 * what it says beside them. It is named by its chat's name (`textValue`, what typing finds), and
 * its description says the rest, after it: its persona and its state, its marks, and what its
 * lines under it say (`RowHeard`), from a part that is drawn again for them, and the row is
 * not. The name is cut short where the row is narrow. A folded row with tasks under it says how
 * many in a small count (`TasksFolded`), and an open one says nothing: its tasks are its rows.
 *
 * **What stands beside it is in it** (#3198): the hand of a chat below that needs you and its
 * ⋯ at its end, its task limit, its dispatches waiting on memory, what its restart says and the
 * second step of ending it under its line. Each control of the row is reached by → from it, as
 * a row's controls are in the tree family's grid, and Tab leaves the tree.
 *
 * **Everything else is the row's card** (`ChatCard`): its state in words, what it is doing,
 * its persona, harness and workspace, how long it has been in its state, its tasks, its own
 * branch, where its work went or came from, and a task's tokens. The card is the popover family's
 * (`AnchoredPopover`), held up by the row and standing beside it, with the tooltip's role: it
 * comes up under a pointer that rests and on the keyboard resting on the row, goes on Escape, and
 * is what the row is described by while it is up. It takes the row's element, the row's hover
 * and the row's focus (#3198).
 */
function ChatRowDrawn({
  session,
  name,
  persona,
  workspace,
  task,
  branch,
  shell,
  report,
  outcome,
  harness,
  runsOn,
  from,
  byYou,
  handedToName,
  handedMore,
  askerWaiting,
  unseen,
  cantReport,
  noted,
  tab,
  current,
  beside,
  open,
  needs,
  askingUnder,
  needsName,
  stopping,
  wrapping,
  sandbox,
  restartSaid,
  asks,
  answer,
  busy,
  trouble,
  onConfirm,
  atLimit,
  waitingOnMemory,
  onLimits,
  offers,
  clock,
  onOpen,
  onPress,
  pointedLast,
  children,
}: {
  session: number;
  name: string;
  persona: string | null;
  workspace: string;
  /** A task, which Delete asks to stop. */
  task: boolean;
  /** The branch of its own a task works on, where it was given one. */
  branch: string | null;
  shell: boolean;
  /** What its state is derived from beside the board's word for it (`ChatShownState`). */
  report: TaskFacts["report"] | null;
  outcome: string | null;
  harness: string | null;
  /** What it runs on (`ListedChat.runsOn`), said in its card (#1673). */
  runsOn?: string | null;
  /** The chat it came from, by name: a handoff's, or a task's whose asker has closed. Nothing
   *  for a chat drawn under its parent. */
  from: string | null;
  /** A task the person asked for themselves, from the ⋯ menu of the chat that asked (V100-70). */
  byYou: boolean;
  /** The newest open chat its work was handed off to, by name; nothing for a chat that
   *  handed nothing off. */
  handedToName: string | null;
  /** How many other open chats it handed off to. */
  handedMore: number;
  /** That chat waits to start after a launch, and has not closed (#1513). */
  askerWaiting: boolean;
  /** Announced and not looked at yet (#4759): it says "new" until it has been on screen. */
  unseen: boolean;
  /** A task whose harness can't report asks (#4759): the faint hand stays on it. */
  cantReport: boolean;
  /** In no pane, with a note in the Notices centre (#4759): its row is marked. */
  noted: boolean;
  tab: boolean;
  current: boolean;
  /** On screen in another pane of the tab in front (#4757): a press on it jumps there. */
  beside: boolean;
  /** Whether its rows are drawn under it, for a row that has some; nothing for a leaf. */
  open: boolean | null;
  /** The chat its hand leads to: itself, a chat below it, or none when it wears no hand. */
  needs: number | null;
  /** How many task questions under it are with their asking chat (#4564): said while folded. */
  askingUnder: number;
  /** That chat's name, when it is a chat below this one. */
  needsName: string | null;
  stopping: boolean;
  /** Being smart-closed (ADR 0064): the breathing mark, where its tab's chip was (#4760). */
  wrapping: boolean;
  /** Its sandbox, moved from its tab to its row and its pane head (#4760). */
  sandbox: SandboxSaid | null;
  /** What its restart says, where it has no pane to say it on (#1462). */
  restartSaid: string | null;
  /** The second step of ending this task, while it is asked here: what is asked, and the
   *  button that does it. Nothing while it is not. */
  asks: string | null;
  answer: string | null;
  busy: boolean;
  /** The core's refusal of the answer, which stays on the row. */
  trouble: string | null;
  onConfirm: (yes: boolean) => void;
  /** Its last dispatch was refused for a limit and no slot has freed since (#1498): the
   *  number, and the core's sentence of which limit and where it is changed. */
  atLimit: AtLimit | null;
  /** How many of its dispatches wait until this machine has memory to spare (#1617); nothing
   *  where none does. */
  waitingOnMemory: number | null;
  /** Opens Settings where the limits are changed. */
  onLimits: () => void;
  offers: Catalogued;
  clock: StateClock;
  onOpen: (session: number) => void;
  onPress: (offer: Offer) => void;
  /** Whether the pointer, and not the keyboard, was what the person used last. */
  pointedLast: () => boolean;
  /** The rows under it: its browsers, the chats it asked for, and its finished tasks. */
  children?: ReactNode;
}) {
  /** Its question's words may give way at the row's end (#4564), as "Stopping…" may: the trail
   *  shrinks for them. */
  const questioned = useQuestionStanding(session) !== undefined;
  /** The row itself: what its card stands beside, and what takes the keyboard back when the
   *  second step is answered Keep. */
  const self = useRef<HTMLDivElement>(null);
  /** Placement mode's pick-up, for the row's grip (#4764, #5290): it never changes. */
  const pickUp = useContext(PickUpHere);
  /** **The row's grip** (#5290): the persona's mark it stands in, the same function from one
   *  drawing of the row to the next, so the mark (a memo) is not drawn again for it. */
  const grip = useMemo(
    () =>
      pickUp === undefined
        ? undefined
        : (mark: ReactNode) => (
            <Grip onPress={() => pickUp({ kind: "chat", session, chat: "" }, self.current)}>
              {mark}
            </Grip>
          ),
    [pickUp, session],
  );
  // A task's tokens, in its card (#1500): read as the pointer or the keyboard rests on the
  // row, never polled.
  const used = useTokensOnHover({ chat: session });
  const state: ShownFacts = { session, shell, report, outcome, harness };
  // **What the row says** (#3658, #3759, #3886): a chat below it needs you, its task limit, its
  // dispatches waiting on memory and where they are named, and what its restart says. Said in
  // the row's description, after its persona and state, and drawn in its card, which describes
  // the row in its place while it is up.
  const sayings: (Said | null)[] = [
    needs !== null && needs !== session
      ? { kind: "needs", line: `${needsName ?? "A chat"} below it needs you` }
      : null,
    atLimit === null ? null : { kind: "limit", line: atLimit.said },
    waitingOnMemory !== null && waitingOnMemory > 0
      ? { kind: "memory", line: sentences(onMemoryWords(waitingOnMemory), ON_MEMORY_SAID) }
      : null,
    restartSaid === null ? null : { kind: "restart", line: restartSaid },
  ];
  const said = sayings.filter((one): one is Said => one !== null);
  // **The card** (#1675). Up after the pointer rests on the row, or the keyboard does, for
  // `CARD_DELAY_MS`: a pointer running down the list, or the arrows going down the tree, bring
  // up none. Down when the pointer leaves, the keyboard moves on, a press lands, or Escape,
  // or when another row's comes up: one card at a time in the window (`chatCard.ts`, #1687).
  const carded = useCardUp(session);
  const cardId = useId();
  const resting = useRef<number | undefined>(undefined);
  /** The pointer is on the row or its card. */
  const pointerOn = useRef(false);
  useEffect(
    () => () => {
      window.clearTimeout(resting.current);
      cardDown(session);
    },
    [session],
  );
  // A task's tokens are read while its card is up, however it came down: another row's card
  // coming up takes this one down without a word to this row.
  const { onPointerEnter: readTokens, onPointerLeave: stopReading } = used;
  useEffect(() => {
    if (!task) return;
    if (carded) readTokens();
    else stopReading();
  }, [task, carded, readTokens, stopReading]);
  /** The pointer or the keyboard left, or a press landed, before the card came up. */
  const unrest = () => {
    window.clearTimeout(resting.current);
    resting.current = undefined;
  };
  const card = (up: boolean) => {
    unrest();
    if (up) cardUp(session);
    else cardDown(session);
  };
  /** Brings the card up once the row has been rested on for `CARD_DELAY_MS`; at once for the
   *  pointer while another card is up or just went down (`chatCard.skipsTheRest`). */
  const rest = (by: "pointer" | "keyboard") => {
    unrest();
    if (by === "pointer" && skipsTheRest()) card(true);
    else resting.current = window.setTimeout(() => card(true), CARD_DELAY_MS);
  };
  /** The keyboard came to the row. Only where the keyboard was used last: a focus a press
   *  gave the row, or gave back to it as a menu it pressed in closed, is no rest of it. */
  const rested = () => {
    if (pointedLast()) unrest();
    else rest("keyboard");
  };
  // The handlers below decide by the window's record of the card (`sessionCardIsUp`), never by
  // `carded`: the wait can bring the card up after the row last drew, and a handler of that
  // drawing would think it down and leave it up (#6073).
  /** The pointer or the keyboard left the row: no card, now or after the wait. */
  const away = () => card(false);
  /** The pointer left the row or its card: the card stays a moment, so the pointer can cross
   *  to it, and stays up while the pointer is on it. */
  const leave = () => {
    pointerOn.current = false;
    if (!sessionCardIsUp(session)) return unrest();
    unrest();
    resting.current = window.setTimeout(() => card(false), CARD_LEAVE_MS);
  };
  // Gone from the window, which an engine may not say on the row or its card (#4628). Only a
  // pointer that was on them: a card the keyboard raised stays.
  useWindowLeft((why) => {
    if (why === "pointer" && pointerOn.current) leave();
  });
  /** Its lines under its words: what binds it, what waits, what its restart says, and the
   *  second step of ending it. */
  const memory = waitingOnMemory !== null && waitingOnMemory > 0;
  const confirming = asks !== null && answer !== null;
  const below = atLimit !== null || memory || restartSaid !== null || confirming;
  return (
    <TreeRow
      id={session}
      textValue={name}
      ref={self}
      className="chat"
      current={current}
      // **What the card says, while it is up, describes the row** (#3886): handed to the row,
      // which hears it with the rest of what describes it and takes back only it (#6147).
      describedBy={carded ? cardId : undefined}
      data-session={session}
      data-tab={tab}
      data-beside={beside || undefined}
      data-task={task || undefined}
      // Dragged onto a pane or the strip, it lands there (#4763); a press stays a press. And F2
      // is its own (`RENAMES_ON_F2`), never the palette's.
      {...({ [CHAT_ROW]: String(session), [RENAMES_ON_F2]: "" } as Record<
        `data-${string}`,
        string
      >)}
      // **Its one list** (#4753): a right-click, Shift+F10 or the menu key, and its ⋯.
      menu={catalogueMenu({ on: "chat", session }, offers, onPress)}
      onAction={() => {
        // A press focuses the row too: that is no rest of the keyboard.
        away();
        onOpen(session);
      }}
      // A touch is no hover: React Aria tells only a pointer's.
      onHoverStart={() => {
        pointerOn.current = true;
        rest("pointer");
      }}
      onHoverEnd={leave}
      // The row's own focus, and not a control's in it: the keyboard resting on the row.
      onFocus={rested}
      onBlur={away}
      content={
        <>
          {/* Its persona's mark with its state on it (#1484, #3342), first on the line; the
              row says both in its description (`RowHeard`). */}
          <RowLead>
            <RowMark persona={persona} state={state} clock={clock} grip={grip} />
          </RowLead>
          {/* Cut short where the row is narrow; whole in the card, and to a screen reader,
              which is told the row's name. */}
          <RowLabel className="session ui-text-body">{name}</RowLabel>
          {/* Its words at its end may give way, a question's (#4564) and "Stopping…" (#3198):
              the time and the marks stay whole, so a narrow row keeps its time in it. */}
          <RowTrail data-gives-way={questioned || stopping || undefined}>
            {open === false && (
              <>
                <TasksFolded session={session} />
                {askingUnder > 0 && (
                  <span className="question-asking ui-text-meta">{askingSaid(askingUnder)}</span>
                )}
              </>
            )}
            {stopping && <span className="stopping ui-text-meta">Stopping…</span>}
            {/* **Not looked at yet** (#4759): a handoff, or a task that can't report asks,
                started in no pane and was announced; the row says so until it is on screen. */}
            {unseen && (
              <span className="ui-text-meta" data-mark="new">
                {NEW_SAID}
              </span>
            )}
            {/* **Can't report asks** (#4759, ADR 0090 as amended 2026-11-02): the title bar's
                faint hand, kept on the row of a task whose harness can't say when it waits. */}
            {cantReport && (
              <Tooltip tip={CANT_REPORT_SAID}>
                <span
                  className="ui-text-secondary"
                  data-mark="cant-report"
                  role="img"
                  aria-label={CANT_REPORT_SAID}
                >
                  <Hand aria-hidden="true" style={QUIET_MARK} />
                </span>
              </Tooltip>
            )}
            {/* **A note in the Notices centre** (#4759): a chat in no pane has no band, so what
                it says is listed there, and its row says it has something to read. */}
            {noted && (
              <Tooltip tip={NOTED_SAID}>
                <span
                  className="ui-text-secondary"
                  data-mark="noted"
                  role="img"
                  aria-label={NOTED_SAID}
                >
                  <Bell aria-hidden="true" style={QUIET_MARK} />
                </span>
              </Tooltip>
            )}
            {/* **Wrapping up, and its sandbox** (#4760): the row says what the strip's chip
                and the tab said before; Cancel smart close is first in its list. */}
            <WrappingUp held={wrapping} />
            <SandboxMark level={sandbox?.level} why={sandbox?.why} sites={sandbox?.sites} />
            <QuestionOrTime clock={clock} session={session} />
            {needs !== null && needs !== session && (
              /* **A chat below this one needs you** (#1448): the hand, and a press goes to that
                 chat, since a press on the row goes to this row's. A control of the row's
                 (#3198), reached by → from it; the title bar's list goes there too. */
              <IconButton
                variant="quiet"
                size="sm"
                icon={<Hand aria-hidden="true" style={HAND} />}
                className="rolled-up"
                data-mark="needs-you"
                data-leads-to={needs}
                excludeFromTabOrder
                aria-label={`Go to ${needsName ?? "the chat"} below ${name}, which needs you`}
                onPress={() => onOpen(needs)}
              />
            )}
            {/* **The row's ⋯** (#4753, the person's ruling on #4585): the chat's one list, the
                menu a right-click, Shift+F10 or the menu key on the row opens, asked for at the
                row so the two are never two menus. Drawn on hover and while the row has the
                keyboard, in room kept for it, so nothing moves. */}
            {!menuIsEmpty({ on: "chat", session }, offers) && (
              <span className="chat-more">
                <IconButton
                  size="sm"
                  variant="ghost"
                  aria-label={moreForSaid(name)}
                  excludeFromTabOrder
                  icon={<Ellipsis aria-hidden="true" />}
                  onPress={() => {
                    if (self.current !== null) askForAMenu(self.current);
                  }}
                />
              </span>
            )}
          </RowTrail>
          {below && (
            <RowBelow className="chat-below">
              {atLimit !== null && (
                /* **At its task limit** (#1498, V100-54): said where the limit binds, on a
                   refusal, until a slot frees; with the number and the way to where it is
                   changed, a control of the row's (#3198). The words are the row's
                   description's too, so they are the eye's here. */
                <span className="at-limit ui-text-secondary" data-testid={`at-limit-${session}`}>
                  <Tooltip tip={atLimit.said}>
                    <span aria-hidden="true">{atLimit.row}</span>
                  </Tooltip>
                  <Button
                    variant="link"
                    onPress={onLimits}
                    className="at-limit-settings"
                    excludeFromTabOrder
                  >
                    Dispatch settings
                  </Button>
                </span>
              )}
              {memory && (
                /* **Its dispatches waiting on memory** (#1617): in the at-limit line's place and
                   style, as the Dispatches tab's Not started list names them. The row says it. */
                <Tooltip tip={ON_MEMORY_SAID}>
                  <span
                    className="at-limit ui-text-secondary"
                    data-testid={`on-memory-${session}`}
                    aria-hidden="true"
                  >
                    {onMemoryWords(waitingOnMemory)}
                  </span>
                </Tooltip>
              )}
              {restartSaid !== null && (
                /* **A restart asked for from this row, refused or waiting** (#1462): in the
                   core's words, for a chat with no pane to say it on. The row says it, and it is
                   announced from outside the tree (`RestartsSaid`). */
                <span className="restart-said ui-text-secondary" aria-hidden="true">
                  {restartSaid}
                </span>
              )}
              {confirming && (
                /* **The second step of ending this task, on its own row** (#1488): nothing ends
                   on one press. Keep has the keyboard, and gives it back to the row. */
                <TaskEndConfirm
                  says={asks}
                  answer={answer}
                  busy={busy}
                  trouble={trouble ?? undefined}
                  onAnswer={() => onConfirm(true)}
                  onKeep={() => {
                    onConfirm(false);
                    self.current?.focus();
                  }}
                />
              )}
            </RowBelow>
          )}
          <RowHeard
            state={state}
            persona={persona}
            folded={open === false}
            askingUnder={askingUnder}
            stopping={stopping}
            unseen={unseen}
            cantReport={cantReport}
            noted={noted}
            said={said}
          />
          {/* The popover family's card, held up by the row and standing beside it, never
              opened by a press: a press on a row opens its chat. Beside the row, over the
              centre, in the default arrangement; it turns to the other side where there is no
              room, as a region moves (ADR 0038). The pointer on the card holds it up (WCAG
              1.4.13): it came across from the row. */}
          <AnchoredPopover
            triggerRef={self}
            isOpen={carded}
            onOpenChange={card}
            side="right"
            align="start"
            offset={6}
            className="row-card chat-card"
            data-testid={`chat-card-${session}`}
            onPointerEnter={() => {
              pointerOn.current = true;
              unrest();
            }}
            onPointerLeave={leave}
          >
            {/* Words about the row, read as its description, and nothing to press: a tooltip,
                which leaves the keyboard on the row as it comes and goes. */}
            <div role="tooltip" id={cardId} className="ui-text-secondary">
              <ChatCard
                state={state}
                name={name}
                persona={persona}
                workspace={workspace}
                branch={branch}
                runsOn={runsOn ?? null}
                cameFrom={from === null ? null : cameFromSaid(from, task, askerWaiting)}
                wentTo={handedToName === null ? null : handedOffSaid(handedToName, handedMore)}
                byYou={byYou}
                said={said}
                clock={clock}
                tokens={task ? used.said : undefined}
              />
            </div>
          </AnchoredPopover>
        </>
      }
    >
      {children}
    </TreeRow>
  );
}

const Row = memo(ChatRowDrawn, sameRow);

/** What a chat's row is drawn from. */
type RowProps = Parameters<typeof ChatRowDrawn>[0];

/**
 * **A chat's row's mark**: its persona's, with its state on it (#1484, #3342), first on the line.
 * Drawn only while its row is: it asks once, as it is drawn, whether its chat's state changed
 * while it was not, so a row a fold had hidden knocks as it comes back for a state that came
 * while it was away.
 */
function RowMark({
  persona,
  state,
  clock,
  grip,
}: {
  persona: string | null;
  state: ShownFacts;
  clock: StateClock;
  grip: ((mark: ReactNode) => ReactNode) | undefined;
}) {
  const [changed] = useState(() => clock.missed(state.session));
  return (
    <ChatPersonaState
      persona={persona}
      {...state}
      changed={changed}
      // **Its grip** (#5290, `ui/grip`): the persona's mark turns into dots under the pointer,
      // and with the keyboard on the row. A drag from it is the row's drag; a press on it picks
      // the chat up for placement mode, the single press for what dragging the row does (WCAG
      // 2.5.7).
      grip={grip}
    />
  );
}

/**
 * **What a chat's row is beside its name, to a reader** (#3198): its persona and its state, as
 * its mark says them (`steward, needs you`), the marks at its end that are words, and what its
 * lines say, as the row's description (`useRowDescription`). It reads its chat's state itself,
 * so a chat moving draws this again and not the row, nor the tree (SC-3).
 */
const RowHeard = memo(function RowHeard({
  state,
  persona,
  folded,
  askingUnder,
  stopping,
  unseen,
  cantReport,
  noted,
  said,
}: {
  state: ShownFacts;
  persona: string | null;
  folded: boolean;
  askingUnder: number;
  stopping: boolean;
  unseen: boolean;
  cantReport: boolean;
  noted: boolean;
  said: readonly Said[];
}) {
  const shown = useShownState(state);
  const total = tasksIn(useTaskCount(state.session));
  const who = persona ?? "terminal";
  const marks = [
    shown === undefined ? who : `${who}, ${shown.word}`,
    folded && total > 0 ? (total === 1 ? "1 task" : `${total} tasks`) : "",
    folded && askingUnder > 0 ? askingSaid(askingUnder) : "",
    stopping ? "Stopping…" : "",
    unseen ? NEW_SAID : "",
    cantReport ? CANT_REPORT_SAID : "",
    noted ? NOTED_SAID : "",
  ]
    .filter((one) => one !== "")
    .join(", ");
  const describes = useRowDescription<HTMLSpanElement>(
    said.map((one) => one.line).reduce(sentences, marks),
  );
  return <span ref={describes} hidden />;
});

/**
 * **A folded row's tasks, as one small number** (#1675): how many tasks are under it, at any
 * depth and however each stands, by the one count (`useTaskCount`). Heard as "3 tasks"; the
 * card says how many are in each state. Reads its own session's tasks, so one arriving redraws
 * this and not the row.
 */
const TasksFolded = memo(function TasksFolded({ session }: { session: number }) {
  const counts = useTaskCount(session);
  const total = tasksIn(counts);
  if (total === 0) return null;
  return (
    <Count
      className="task-count"
      data-testid={`task-count-${session}`}
      said={total === 1 ? "1 task" : `${total} tasks`}
    >
      {total}
    </Count>
  );
});

/**
 * **What a row's card says** (#1675): everything a row said on its second line and in its
 * counts before it was one line, as facts with a word for each. Drawn only while the card is
 * up, so what it reads (the clock, the tasks, what the chat is doing) costs nothing until then.
 */
function ChatCard({
  state,
  name,
  persona,
  workspace,
  branch,
  runsOn,
  cameFrom,
  wentTo,
  byYou,
  said,
  clock,
  tokens,
}: {
  /** What its state is derived from (`ChatShownState`), as its row has it. */
  state: ShownFacts;
  name: string;
  persona: string | null;
  workspace: string;
  branch: string | null;
  /** What it runs on (`ListedChat.runsOn`): its profile and harness, where it has a profile
   *  (#1673). */
  runsOn: string | null;
  /** Where it came from, as its row said it, where it is not drawn under that chat. */
  cameFrom: string | null;
  /** Where its work went, as its row said it. */
  wentTo: string | null;
  byYou: boolean;
  /** What its row says (#3886): each a line of the card, as the row's description has them. */
  said: readonly Said[];
  clock: StateClock;
  /** A task's tokens, once read. */
  tokens: string | undefined;
}) {
  const { session, harness } = state;
  const tasks = useTaskCountSaid(session);
  return (
    <>
      <p className="chat-card-head ui-text-body">
        <span className="chat-card-name ui-text-body ui-text-strong">{name}</span>
        <ChatShownState {...state} />
      </p>
      {/* What a working chat is doing (#1493): read as a line of the card, where the row's own
          line was out of the tree and named by the row's description. */}
      <ChatDoingLine session={session} heard />
      {/* What the row says (#3886): its card describes the row in place of the row's own
          description while it is up, so the card says it too; and here a sighted keyboard
          user reads what the lines under the row carry on their hover. */}
      {said.map((one) => (
        <p key={one.kind}>{one.line}</p>
      ))}
      <dl className="chat-card-facts">
        {persona !== null && <Fact term="Persona">{persona}</Fact>}
        {(runsOn ?? harness) !== null && <Fact term="Harness">{runsOn ?? harness}</Fact>}
        <Fact term="Workspace">{workspace}</Fact>
        <StateSince clock={clock} session={session} />
        {tasks !== undefined && <Fact term="Tasks">{tasks}</Fact>}
        {branch !== null && <Fact term="Branch">{branch}</Fact>}
      </dl>
      {wentTo !== null && <p>{wentTo}</p>}
      {cameFrom !== null && <p>{cameFrom}</p>}
      {byYou && <p>{ASKED_BY_YOU}</p>}
      {/* A chat's helpers, as a count (#1490, V100-4): every chat's, since the explorer draws
          no chat any more (#1673). */}
      <HelpersSaid session={session} />
      {tokens !== undefined && <p>{tokens}</p>}
    </>
  );
}

/** One fact of a card: its word, and what it is. */
function Fact({ term, children }: { term: string; children: ReactNode }) {
  return (
    <div>
      <dt className="ui-text-meta">{term}</dt>
      <dd>{children}</dd>
    </div>
  );
}

/**
 * **When a chat came into its state, at its row's end** (#3484, the prototype's `.rm`): `now`,
 * `2m`, `10:42`, quiet, from the one clock (`useStateSince`), so its tick redraws this and no
 * row. Seen and not heard: the row's card says how long in words (`StateSince`). None where the
 * window did not see the chat come into its state.
 */
/**
 * **What a task's question waits on, and for how long** (#4564), where it has one: `asking
 * steward 2 · 1m` or `needs you · now`, timed from when it was asked or reached the person, on
 * the row's own clock. Otherwise how long the chat has been in its state ({@link RowTime}).
 */
const QuestionOrTime = memo(function QuestionOrTime({
  clock,
  session,
}: {
  clock: StateClock;
  session: number;
}) {
  const question = useQuestionStanding(session);
  if (question === undefined) return <RowTime clock={clock} session={session} />;
  return <QuestionWaits question={question} />;
});

function QuestionWaits({ question }: { question: QuestionStanding }) {
  const now = useNow();
  const { who, since } = questionWords(question, new Date(now));
  // The asker's name gives way, as the row's name does; the time stays whole (#1675).
  return (
    <span className="question-waits chat-time ui-text-meta" aria-hidden="true">
      <span className="question-who">{who}</span>
      {since !== null && <span className="question-since">{` · ${since}`}</span>}
    </span>
  );
}

const RowTime = memo(function RowTime({ clock, session }: { clock: StateClock; session: number }) {
  const seconds = useStateSince(clock, session);
  if (seconds === null) return null;
  return (
    <span className="chat-time ui-text-meta" aria-hidden="true">
      {rowTimeSaid(seconds, new Date())}
    </span>
  );
});

/**
 * How long a chat has been in its state (V100-19), where this window saw it come into it
 * (`stateClock.ts`). Reads its own chat's time, so a chat that moves, and the clock's tick,
 * redraw this and no row.
 */
const StateSince = memo(function StateSince({
  clock,
  session,
}: {
  clock: StateClock;
  session: number;
}) {
  const seconds = useStateSince(clock, session);
  if (seconds === null) return null;
  return <Fact term="In this state">{sinceSaid(seconds)}</Fact>;
});
