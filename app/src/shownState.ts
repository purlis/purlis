// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a chat's row says it is doing: one word and one shape** (#1484, V100-3, V100-72).
 *
 * Derived here and nowhere else, from what already exists: the board's state for the chat,
 * whether it is in the needs-you queue, and, for a task, what its own record says (the report
 * it owes, how it reported, a question it has open with the chat that dispatched it). The Chats
 * list and the explorer both draw this value, so the same chat reads the same in both, and a
 * later surface (a tab's chip, its menu) calls the same function.
 *
 * **Nothing is guessed.** A harness purlis has heard nothing from reads `running (no detail
 * from <harness>)`: the program is running, which the app knows itself, and the rest is said
 * to be unknown (V100-71). A report whose outcome no record gives reads `reported`.
 */
import type { OpenChat, OwnWork } from "./bindings";
import type { State } from "./chatState";
import type { Token } from "./theme/theme";

/** Which state it is: what a surface folds, counts or sorts by. */
export type ShownKind =
  | "working"
  | "needs-you"
  | "task-failed"
  | "waiting-on-tasks"
  | "waiting-on-own-work"
  | "asking"
  | "done"
  | "failed"
  | "cancelled"
  | "stopped-by-you"
  | "closed-by-you"
  | "stopped-at-limit"
  | "stopped-with-above"
  | "unreported"
  | "last-message"
  | "reported"
  | "idle"
  | "stuck"
  | "turn-ended-for-asker"
  | "shell"
  | "unheard";

/** The mark's shape, one per kind, so a state is told without its colour. */
export type ShownShape =
  | "ring"
  | "hand"
  | "hourglass"
  | "question"
  | "tick"
  | "cross"
  | "dash"
  | "square"
  | "octagon"
  | "triangle"
  | "dot"
  | "pause"
  | "stuck"
  | "unseen"
  | "dots";

/**
 * **The Chat states** (ADR 0121 decision 17, #6012), in the order that wins where several are
 * true (#5340): what a chat's mark wears on its row in the list, its tab, its pane's head, a
 * folded pane's rail, All chats and Asks (`ui/chat-mark`). Each is drawn from its current run's
 * state (ADR 0076) by the one function, {@link shownState}.
 *
 * **The keys are the theme's** (`chat-mark.badge.<key>.text`): a published theme format only
 * grows, so the three keys the mark had before #6012 keep their names: `needs` is Waiting on you,
 * `waiting` is Waiting on a chat and `ended` is Done.
 *
 * - `needs`, Waiting on you: in the needs-you queue: the person has the next move.
 * - `stuck`, Stuck?: working, and something says it may not be getting anywhere (#6014 decides what).
 *   A question, never a verdict.
 * - `failed`, Failed: its program or its report says it failed, or a task it asked for did and has
 *   not been seen.
 * - `working`, Working: a turn runs, or its turn ended on work of its own that wakes it (ADR 0076, #3878).
 * - `waiting`, Waiting on a chat: its tasks' reports, an answer from the chat that
 *   asked it, or, for a dispatched task, that chat's next word after its turn ended (#6036).
 * - `idle`, Idle: its turn ended, it asks nothing, and its work is left for the person.
 * - `ended`, Done: its program ended, or it is a task whose report was sent or that the person ended.
 * - `cant-tell`, Can't tell: a shell, or a harness that has said nothing: purlis does not know.
 */
export const CHAT_STATES = [
  "needs",
  "stuck",
  "failed",
  "working",
  "waiting",
  "idle",
  "ended",
  "cant-tell",
] as const;
export type ChatState = (typeof CHAT_STATES)[number];

/** **Each Chat state by its name** (CONTEXT.md, **Chat state**): what a list of every chat says
 *  of it where it names the state rather than the chat's own word (All chats, #6013). */
export const CHAT_STATE_NAMES: Readonly<Record<ChatState, string>> = {
  needs: "Waiting on you",
  stuck: "Stuck?",
  failed: "Failed",
  working: "Working",
  waiting: "Waiting on a chat",
  idle: "Idle",
  ended: "Done",
  "cant-tell": "Can't tell",
};

/**
 * **A split tab's mark is its most urgent pane's** (#5342, spec #5339), so a pane that needs the
 * person can't hide behind the focused one: of `panes`, in reading order, the one whose state
 * comes first in {@link CHAT_STATES}. Of two in one state the first in reading order wins, and a
 * pane with no state (a view, an empty pane) is never chosen: nothing, where no pane has one.
 */
export function urgentOf<T>(
  panes: readonly T[],
  stateOf: (pane: T) => ChatState | null | undefined,
): T | undefined {
  let best: { pane: T; rank: number } | undefined;
  for (const pane of panes) {
    const state = stateOf(pane);
    if (state === null || state === undefined) continue;
    const rank = CHAT_STATES.indexOf(state);
    if (best === undefined || rank < best.rank) best = { pane, rank };
  }
  return best?.pane;
}

/** A state in words and a shape, as a line that says it beside its word draws it. */
export type Said = {
  kind: ShownKind;
  /** The word beside the mark. Always drawn. */
  word: string;
  shape: ShownShape;
  /** The theme token the mark is coloured by. Never the word's colour: the word stays
   *  legible whatever a theme does with these. */
  token: Token;
};

/** A chat's state as a row shows it: its words, and the one Chat state they stand for. */
export type Shown = Said & { mark: ChatState };

/**
 * **Each Chat state's one shape and one colour** (ADR 0121 decision 17): no two share either,
 * so a state is told by its shape where colour is not seen (WCAG 1.4.1). The chat mark's badge
 * draws these (`ui/chat-mark`), and a line that says the state beside its word draws the same
 * (`ChatRows.ChatShownState`), so a chat wears one glyph in one colour wherever it is.
 */
export const CHAT_STATE_SAID: Readonly<Record<ChatState, { shape: ShownShape; token: Token }>> = {
  needs: { shape: "hand", token: "needs-you.base" },
  stuck: { shape: "stuck", token: "state.unreadable" },
  failed: { shape: "cross", token: "state.failed" },
  working: { shape: "ring", token: "state.running" },
  waiting: { shape: "hourglass", token: "state.waiting" },
  idle: { shape: "pause", token: "accent.base" },
  ended: { shape: "tick", token: "state.success" },
  "cant-tell": { shape: "unseen", token: "text.muted" },
};

/** Which Chat state each word stands for. The word says which way a finished chat ended. */
const STATE_OF_KIND: Readonly<Record<ShownKind, ChatState>> = {
  "needs-you": "needs",
  stuck: "stuck",
  "task-failed": "failed",
  failed: "failed",
  unreported: "failed",
  working: "working",
  "waiting-on-own-work": "working",
  "waiting-on-tasks": "waiting",
  asking: "waiting",
  "turn-ended-for-asker": "waiting",
  idle: "idle",
  done: "ended",
  cancelled: "ended",
  "stopped-by-you": "ended",
  "closed-by-you": "ended",
  "stopped-at-limit": "ended",
  "stopped-with-above": "ended",
  "last-message": "ended",
  reported: "ended",
  shell: "cant-tell",
  unheard: "cant-tell",
};

/** What a task's own record says of it, apart from what its program is doing. */
export type TaskFacts = {
  /** The one report it owes the chat that dispatched it: still owed, sent, or failed, which
   *  is purlis's word for a task that ended without one. */
  report: "owed" | "sent" | "failed";
  /** How it ended, in its dispatch record's word, where a record says: its own report's, or
   *  the app's own fact that the person ended it and which way (`stopped_by_person`,
   *  `closed_by_person`), which is never a word a task can report. */
  outcome: string | null;
  /** The chat it has a question open with, by name, while it has one. */
  asking: string | null;
};

/** Everything a chat's shown state is derived from. */
export type Facts = {
  /** The board's state for it, or nothing for a shell tab nothing has reported for
   *  (`markOf`). `unknown` is a harness chat the board has not heard from. */
  board: State | undefined;
  /** Whether it is in the needs-you queue. A chat whose turn has ended and which is not in it
   *  is one the person told purlis to ignore until it asks again, or a task whose report
   *  already reached the chat that asked. */
  needsYou: boolean;
  /** Its record as a task, or nothing for a chat that is not one. */
  task: TaskFacts | null;
  /** Its harness, as the person calls it: named in what is not known. */
  harness: string | null;
  /** How many tasks below it, at any depth, are working (#1491): the working bucket of its
   *  count (`taskBuckets.ts`), which a task waiting on the person is not in. A chat whose turn
   *  has ended with any is waiting on them, and says how many. None where a surface does not
   *  say. */
  tasksAtWork?: number;
  /** What it waits on of its own, where its turn ended while that is in flight (#3878,
   *  `chatState.ownWorkOf`). None where it waits on none, or a surface does not say. */
  ownWork?: OwnWork;
  /** How many tasks it asked for came to nothing and have not been seen (#5340,
   *  `chatState.failuresUnseenOf`). None where a surface does not say. */
  failuresUnseen?: number;
  /** It is such a task itself, and has not been seen (#5340, `chatState.failedUnseen`). */
  failedUnseen?: boolean;
  /** Its turn runs and may not be getting anywhere (ADR 0121 decision 19): the core's word
   *  (`purlis_core::stuck`, #6014), which "It's fine" clears until the chat changes state. None
   *  where a surface does not say. */
  stuck?: boolean;
  /** Whether its folder holds uncommitted work (#6014, `purlis_core::leftwork`), asked as it
   *  changed state. A finished chat with work left is Idle; with none it is Done. None where it
   *  is not known (not a repository, or a surface that does not say), which reads Idle. */
  leftWork?: boolean;
};

const WORKING: Said = { kind: "working", word: "working", shape: "ring", token: "state.running" };
const NEEDS_YOU: Said = {
  kind: "needs-you",
  word: "needs you",
  shape: "hand",
  token: "needs-you.base",
};
const DONE: Said = { kind: "done", word: "done", shape: "tick", token: "text.muted" };
const FAILED: Said = { kind: "failed", word: "failed", shape: "cross", token: "state.failed" };
const CANCELLED: Said = {
  kind: "cancelled",
  word: "cancelled",
  shape: "dash",
  token: "text.muted",
};
/** **The person stopped it and got its report** (#1488): the square of a stop button. Its own
 *  word and shape, as the person's doing: never "cancelled", which is the asking chat's. */
const STOPPED_BY_YOU: Said = {
  kind: "stopped-by-you",
  word: "stopped by you",
  shape: "square",
  token: "text.muted",
};
/** **The person closed it**, with no report from it (#1488): the stop sign's eight sides. */
const CLOSED_BY_YOU: Said = {
  kind: "closed-by-you",
  word: "closed by you",
  shape: "octagon",
  token: "text.muted",
};
/** **purlis stopped it at its time limit** (#1512): a stop, so the stop's square, in words
 *  that say it was the limit and not the person there and then. */
const STOPPED_AT_LIMIT: Said = {
  kind: "stopped-at-limit",
  word: "stopped at its time limit",
  shape: "square",
  token: "text.muted",
};
/** **purlis stopped it with the task above it**, at that task's time limit (#1512). */
const STOPPED_WITH_ABOVE: Said = {
  kind: "stopped-with-above",
  word: "stopped with the task above it",
  shape: "square",
  token: "text.muted",
};
/** **purlis stopped it because its session reached its token limit** (#1512, #1457). */
const STOPPED_AT_TOKEN_LIMIT: Said = {
  kind: "stopped-at-limit",
  word: "stopped at its session's token limit",
  shape: "square",
  token: "text.muted",
};
const UNREPORTED: Said = {
  kind: "unreported",
  word: "ended without a report",
  shape: "triangle",
  token: "state.unreadable",
};
/** **A report purlis made from its last message** (#1981): the task's turns ended without a
 *  report, and purlis sent what it last said. Its words, never the task's own report. */
const LAST_MESSAGE: Said = {
  kind: "last-message",
  word: "a report purlis made from its last message",
  shape: "triangle",
  token: "state.unreadable",
};
const IDLE: Said = { kind: "idle", word: "idle", shape: "pause", token: "accent.base" };
const REPORTED: Said = { kind: "reported", word: "reported", shape: "dot", token: "text.muted" };
/** **Stuck? is a question** (ADR 0121 decision 19): working, with nothing to show for a while. */
const STUCK: Said = { kind: "stuck", word: "stuck?", shape: "stuck", token: "state.unreadable" };
/** **A dispatched task whose turn ended owing its report** (ADR 0107, #6036): it reports to the
 *  chat that dispatched it, so it waits on that chat's next word, not the person's. */
const TURN_ENDED_FOR_ASKER: Said = {
  kind: "turn-ended-for-asker",
  word: "waiting on the chat that asked",
  shape: "hourglass",
  token: "state.waiting",
};
/** **A shell reports no state**: purlis cannot tell what it is doing, and says so. */
const SHELL: Said = {
  kind: "shell",
  word: "a shell (no detail)",
  shape: "unseen",
  token: "text.muted",
};

/**
 * How a report a task sent reads, by the dispatch record's word for its outcome. `blocked` is
 * a task that could not do the work, which the row says as `failed`; its report says why. Any
 * other word is not one this window knows, and is not guessed at.
 */
const BY_OUTCOME: ReadonlyMap<string, Said> = new Map([
  ["done", DONE],
  ["failed", FAILED],
  ["blocked", FAILED],
  ["cancelled", CANCELLED],
  ["last_message", LAST_MESSAGE],
]);

/**
 * **A chat whose turn has ended while tasks below it still work** (#1491, V100-16): it waits
 * on them and not on the person, so it wears the working colour and a shape of its own, and
 * says how many. The core keeps such a chat out of the needs-you queue, so this never stands
 * where `needs you` would.
 */
export function waitingOnTasks(tasks: number): Said {
  return {
    kind: "waiting-on-tasks",
    word: tasks === 1 ? "waiting on 1 task" : `waiting on ${tasks} tasks`,
    shape: "hourglass",
    token: "state.running",
  };
}

/** `1 monitor`, `2 monitors`: one kind of a chat's own work, counted. */
export function counted(n: number, one: string): string | undefined {
  if (n === 0) return undefined;
  return `${n} ${one}${n === 1 ? "" : "s"}`;
}

/**
 * **What a chat waiting on its own work says it waits on** (#3878): each kind of work in
 * flight, counted, and the time of its next wake-up of its own, as `waiting on 2 monitors` or
 * `waiting on 1 helper and a wake-up at 14:05`. The clock is the person's own.
 */
export function ownWorkSaid(work: OwnWork): string {
  const parts = [
    counted(work.helpers, "helper"),
    counted(work.workflows, "workflow"),
    counted(work.monitors, "monitor"),
    counted(work.shells, "background shell"),
    work.wakes_at == null
      ? undefined
      : `a wake-up at ${new Date(work.wakes_at * 1000).toLocaleTimeString([], {
          hour: "2-digit",
          minute: "2-digit",
        })}`,
  ].filter((part): part is string => part !== undefined);
  const said =
    parts.length < 2 ? parts.join("") : `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)}`;
  return `waiting on ${said === "" ? "its own work" : said}`;
}

/**
 * **A chat whose turn has ended while work of its own is in flight** (#3878): waiting on it,
 * and not on the person, so it wears the working colour and the hourglass a chat waiting on its
 * tasks wears, and says what it waits on. The core keeps such a chat out of the needs-you queue.
 */
export function waitingOnOwnWork(work: OwnWork): Said {
  return {
    kind: "waiting-on-own-work",
    word: ownWorkSaid(work),
    shape: "hourglass",
    token: "state.running",
  };
}

/**
 * **How a task the person ended reads**, by the core's own fact of who ended it and which way
 * (`dispatchrecord::Finished::key`): never read from a report's words, so nothing a task says
 * of itself reads as either. `stopped` is the outcome of a record written before the way was
 * kept, and is read as closed.
 */
const BY_THE_PERSON: ReadonlyMap<string, Said> = new Map([
  ["stopped_by_person", STOPPED_BY_YOU],
  ["closed_by_person", CLOSED_BY_YOU],
  ["stopped", CLOSED_BY_YOU],
  // The app's fact that it stopped the task at a limit the person set (#1512).
  ["stopped_at_limit", STOPPED_AT_LIMIT],
  ["stopped_with_above", STOPPED_WITH_ABOVE],
  ["stopped_at_token_limit", STOPPED_AT_TOKEN_LIMIT],
]);

/**
 * **The one function that derives a chat's state** (ADR 0121 decision 17, #6012): its words, and
 * the Chat state they stand for, read off what the window knows of it.
 *
 * **The needs-you queue is read first.** It is the core's word for "the person has the next
 * move", and the title bar's list and the hands on the rows above are drawn from it: a row
 * that said anything else would disagree with them. The core keeps a reported task's ordinary
 * end of turn out of the queue, so a finished task is not in it for having finished.
 *
 * **Then a task's record, before its program's state.** A task that reported is done, failed
 * or cancelled however its turn then ends: that end is what made a finished task look like a
 * chat waiting on the person. One that is running again, on the person's word, is working.
 *
 * **The order, whole** (#1491, #5340, #6012): needs you; stuck?; a failed task not yet seen; a sent report (working while a turn runs, else how
 * it ended); a report written in its place; a program that ended; asking its asker; waiting on its
 * tasks; waiting on its own work; a task's turn ended owing its report; idle; working; not known. **A
 * record of how it ended is a program that has ended** (a task ends at its report), so nothing
 * live is true of it: its tasks at work leave it `done`.
 */
export function shownState(facts: Facts): Shown {
  const said = saidOf(facts);
  // Its own failure, not yet seen: its own word, on the mark that says so.
  if (facts.failedUnseen === true && said.kind !== "needs-you" && said.kind !== "stuck")
    return { ...said, mark: "failed" };
  return { ...said, mark: STATE_OF_KIND[said.kind] };
}

/** `a task failed`, `3 tasks failed`: what a chat's tasks came to, until it has been seen. */
function tasksFailed(tasks: number): Said {
  return {
    kind: "task-failed",
    word: tasks === 1 ? "a task failed" : `${tasks} tasks failed`,
    shape: "cross",
    token: "state.failed",
  };
}

/** The words and shape of a chat's state, in the order {@link shownState} says. */
function saidOf({
  board,
  needsYou,
  task,
  harness,
  tasksAtWork = 0,
  ownWork,
  failuresUnseen = 0,
  stuck = false,
  leftWork,
}: Facts): Said {
  if (needsYou) return NEEDS_YOU;
  // Stuck? is asked of a turn that runs, before anything else the person could look at.
  if (stuck && board === "running" && task?.report !== "failed") return STUCK;
  if (failuresUnseen > 0) return tasksFailed(failuresUnseen);
  const byThePerson = task?.outcome == null ? undefined : BY_THE_PERSON.get(task.outcome);
  if (task?.report === "sent") {
    if (board === "running") return WORKING;
    // The person stopped it, whatever it then said of itself in its one short report.
    if (byThePerson !== undefined) return byThePerson;
    return (task.outcome === null ? undefined : BY_OUTCOME.get(task.outcome)) ?? REPORTED;
  }
  // The person closed it: not a task that died, though neither sent a report.
  if (task?.report === "failed") return byThePerson ?? UNREPORTED;
  // Its program ended owing its report: purlis tells the chat that asked, and the row says
  // the same without waiting to be told.
  if (task !== null && (board === "done" || board === "failed")) return UNREPORTED;
  if (board === "done") return DONE;
  if (board === "failed") return FAILED;
  if (task !== null && task.asking !== null) {
    return {
      kind: "asking",
      word: `asking ${task.asking}`,
      shape: "question",
      token: "state.waiting",
    };
  }
  // **Below needs you and above idle** (#1491): its turn has ended and tasks below it have
  // not, so it is waiting on them. A task that is asking its own asker says that first: the
  // answer is what it is paused on.
  if (board === "waiting" && tasksAtWork > 0) return waitingOnTasks(tasksAtWork);
  // Its turn has ended on work of its own, which wakes it (#3878): waiting, not idle.
  if (board === "waiting" && ownWork !== undefined) return waitingOnOwnWork(ownWork);
  // A task's turn ended owing its report: it waits on the chat that asked (#6036).
  if (board === "waiting" && task !== null && task.report === "owed") return TURN_ENDED_FOR_ASKER;
  // Its turn has ended and it is not asking for the person: the hand is not raised for it.
  // Idle is finished with work left behind (#6014): with none left in its folder, it is done.
  if (board === "waiting") return leftWork === false ? DONE : IDLE;
  if (board === "running") return WORKING;
  if (board === undefined) return SHELL;
  return {
    kind: "unheard",
    word: harness === null ? "running (no detail)" : `running (no detail from ${harness})`,
    shape: "unseen",
    token: "text.muted",
  };
}

/**
 * What `chat`'s record says of it as a task, or nothing for a chat that is not one. Its open
 * question is not its record's to say: the asks registry says where it stands (#4564), and the
 * row reads it there (`QuestionsHere`).
 */
export function taskFactsOf(chat: Pick<OpenChat, "from">): TaskFacts | null {
  const from = chat.from;
  if (!from?.task) return null;
  return {
    report: from.unreported ? "failed" : from.reported ? "sent" : "owed",
    outcome: from.outcome ?? null,
    asking: null,
  };
}

/** A chat's harness as the person calls it (`Codex`), or the project's word for it. */
export function harnessCalled(chat: Pick<OpenChat, "harness" | "card">): string | null {
  return chat.card?.title ?? chat.harness;
}

/** What a row hands the one function about its chat beside the board's word: plain values, so
 *  a row held on them is drawn again only when one changes. Its question is the asks
 *  registry's, filled in where the list is made (`listedChat`, #4564). */
export type RowFacts = {
  report: TaskFacts["report"] | null;
  outcome: string | null;
  /** The chat its question is with, by name, as the asks registry says (#4564). */
  asking: string | null;
  harness: string | null;
};

/** `chat` as a row hands it on: its record as a task, flat, and its harness's name. */
export function rowFactsOf(chat: Pick<OpenChat, "from" | "harness" | "card">): RowFacts {
  const task = taskFactsOf(chat);
  return {
    report: task?.report ?? null,
    outcome: task?.outcome ?? null,
    asking: null,
    harness: harnessCalled(chat),
  };
}
