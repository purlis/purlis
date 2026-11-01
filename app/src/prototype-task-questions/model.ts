// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (spec #4551, #4561): one world in memory, and everything every surface draws
 * is computed from it. That is the spec's "one computed state per question": no surface holds a
 * copy, so one answer clearing every surface is true here by construction, and the page's checks
 * can read the rest. No persistence, nothing calls the core.
 */
import { COPY } from "./copy";

export type Where = "top" | "each";
export type Phase = "with-chat" | "reached" | "closed";
export type Why = "passed-on" | "no-reply" | "gone" | "crashed" | "lost" | "nobody" | "asked-by-you" | "redirect";
export type ClosedBy = "chat" | "person" | "nobody";
export type Kind = "choices" | "text" | "cannot-draw";

export type Chat = {
  id: string;
  name: string;
  persona: string | null;
  parent: string | null;
  harness: string;
  /** Whether it has a tab of its own to be "the top" (a session). */
  tab: boolean;
  folded?: boolean;
  /** A harness with no adapter: its question stays in its pane (story 58, 65). */
  noAdapter?: boolean;
};

export type Step = { at: number; said: string };

export type Question = {
  id: string;
  task: string;
  kind: Kind;
  says: string;
  options?: string[];
  phase: Phase;
  why?: Why;
  /** The asking chat's one line, marked as its own (story 11). */
  note?: string;
  closedBy?: ClosedBy;
  chatSaid?: string;
  askedAt: number;
  reachedAt?: number;
  trail: Step[];
  overridden?: boolean;
};

/** A chat waiting on the person that is not a task question: stays on the chat and the Inbox. */
export type Wait = {
  id: string;
  chat: string;
  kind: "reply" | "prose" | "permission" | "browser";
  says: string;
  /** The few turn-end messages before the last, oldest first (story 44). */
  earlier?: string[];
  since: number;
};

export type Update = { id: string; says: string; question?: string; trouble?: boolean };

export type Panel = "inbox" | "activity" | null;

export type World = {
  chats: Chat[];
  questions: Question[];
  waits: Wait[];
  updates: Update[];
  where: Where;
  /** The first-time line has not been answered yet. */
  firstLine: boolean;
  /** The line that says the setting moved, once. */
  movedLine: boolean;
  tabs: string[];
  /** A chat's id, or "settings". */
  front: string;
  panel: Panel;
  /** The task whose questions Activity lists. */
  activityOf: string | null;
  /** The minutes since the scene began, as the world's clock. */
  clock: number;
  /** The short pause before the next quiz (story 33). */
  pause: { task: string } | null;
  /** A quiz an agent closed: its line, until dismissed (story 36). */
  closing: string[];
  stripOpen: boolean;
  /** Which earlier message the strip shows, counted from the oldest; null is the last. */
  stripAt: number | null;
  /** What the person typed into a quiz's box, by question: kept if it closes (AnswerQuestion). */
  typed: Record<string, string>;
  /** Lines the terminal has printed since the scene began, per chat. */
  printed: Record<string, string[]>;
};

export const ASK = {
  limit:
    "The pool limit is per runner in the docs but per project in the code. Which one should win?",
  empty: "Should the empty Inbox say when the last question closed, or stay blank?",
  migration:
    "Two tickets both claim the audit migration. Should I split it, or hand all of it to the core ticket?",
  clock: "The 100-second timer needs a fake clock in the core to test. May I add one?",
  long: "The sign-in token for the staging pool is sk-••••••••3f2a, and it expires tonight. The runner image pins it at build time, so rotating it means rebuilding the image for every pool, which takes about forty minutes each and blocks the release branch while it runs. I can rotate it now and hold the release until the images are done, or leave it until after the release and run on the old token for the two hours left. Which do you want?",
  fixture: "The recorded fixture disagrees with the new row word. Should I re-record it?",
};

const CHOICES = ["Per runner", "Per project", "Keep both and warn"];

export const CHATS: Chat[] = [
  { id: "steward", name: "steward 4", persona: "steward", parent: null, harness: "Claude Code", tab: true },
  { id: "dev7", name: "developer 7", persona: "developer", parent: "steward", harness: "Claude Code", tab: false },
  { id: "design3", name: "designer 3", persona: "designer", parent: "steward", harness: "Claude Code", tab: false },
  { id: "arch2", name: "architect 2", persona: "architect", parent: "steward", harness: "Claude Code", tab: false },
  { id: "dev9", name: "developer 9", persona: "developer", parent: "arch2", harness: "Claude Code", tab: false },
  { id: "qa5", name: "qa 5", persona: "qa", parent: "dev9", harness: "Claude Code", tab: false },
  { id: "rev1", name: "reviewer 1", persona: "reviewer", parent: "steward", harness: "Codex", tab: false, noAdapter: true },
  { id: "talk", name: "talk 2", persona: null, parent: null, harness: "Claude Code", tab: true },
  { id: "ops3", name: "devops 3", persona: "devops", parent: "talk", harness: "Claude Code", tab: false },
];

export function world(more: Partial<World> = {}): World {
  return {
    chats: structuredClone(CHATS),
    questions: [],
    waits: [],
    updates: [],
    where: "top",
    firstLine: false,
    movedLine: false,
    tabs: ["steward", "talk"],
    front: "steward",
    panel: null,
    activityOf: null,
    clock: 30,
    pause: null,
    closing: [],
    stripOpen: false,
    stripAt: null,
    typed: {},
    printed: {},
    ...more,
  };
}

export const chatOf = (w: World, id: string): Chat => w.chats.find((c) => c.id === id) as Chat;
export const nameOf = (w: World, id: string): string => chatOf(w, id).name;
export const askerOf = (w: World, task: string): string | null => chatOf(w, task).parent;
export const questionOf = (w: World, id: string): Question =>
  w.questions.find((q) => q.id === id) as Question;

/** The nearest chat above with a tab, as a task's session tab is defined today. */
export function topOf(w: World, id: string): string {
  let at = chatOf(w, id);
  while (!at.tab && at.parent !== null) at = chatOf(w, at.parent);
  return at.id;
}

/** The chain from the session down to the task: `steward 4 › architect 2 › qa 5`. */
export function chainOf(w: World, id: string): string[] {
  const names: string[] = [];
  for (let at: Chat | null = chatOf(w, id); at; at = at.parent ? chatOf(w, at.parent) : null)
    names.unshift(at.name);
  return names;
}

export function below(w: World, id: string): Chat[] {
  const under = w.chats.filter((c) => c.parent === id);
  return under.flatMap((c) => [c, ...below(w, c.id)]);
}

/* ---------- the three levels of the visibility rule ---------- */

export const reachedOf = (w: World) =>
  w.questions.filter((q) => q.phase === "reached").sort((a, b) => order(a) - order(b));
export const inProgressOf = (w: World) => w.questions.filter((q) => q.phase === "with-chat");
const order = (q: Question) => q.reachedAt ?? q.askedAt;

/** ✋: what waits on the person, and nothing else. */
export const needsCount = (w: World) => reachedOf(w).length + w.waits.length;

/** What waits on the person under a chat, for the rows above it: a total and nothing more. */
export function needsUnder(w: World, id: string): number {
  const ids = new Set(below(w, id).map((c) => c.id));
  return (
    reachedOf(w).filter((q) => ids.has(q.task)).length +
    w.waits.filter((wait) => ids.has(wait.chat)).length
  );
}

/** What a tab's count says: everything that waits in that tab, itself included. */
export function needsInTab(w: World, tab: string): number {
  const own = w.waits.filter((one) => one.chat === tab).length;
  const mine = (q: Question) => q.task === tab;
  return needsUnder(w, tab) + own + reachedOf(w).filter(mine).length;
}

/* ---------- what a tab draws ---------- */

/** The quiz in front: the task's own in its tab (either mode), or the oldest under a top tab. */
export function quizInFront(w: World): Question | null {
  const front = w.front;
  if (front === "settings" || w.pause) return null;
  // In a task's own tab its open question can be answered at once, at any phase (story 16, 30).
  const own = w.questions.find((q) => q.task === front && q.phase !== "closed");
  if (own) return own;
  if (w.where !== "top" || !chatOf(w, front).tab) return null;
  // A failed redirect falls back to the task's pane and the Inbox, never the top (spec, Route).
  return (
    reachedOf(w).find((q) => topOf(w, q.task) === front && q.why !== "redirect") ?? null
  );
}

/** The questions behind the one drawn, in the same tab (story 32). */
export function restBehind(w: World, shown: Question): Question[] {
  if (shown.task === w.front) return [];
  return reachedOf(w).filter(
    (q) => q.id !== shown.id && topOf(w, q.task) === w.front && q.why !== "redirect",
  );
}

/** The strip: a wait of the chat in front. A task's quiz is the strip with its answers on it. */
export function stripInFront(w: World): Wait | null {
  if (w.front === "settings") return null;
  return w.waits.find((one) => one.chat === w.front) ?? null;
}

/** Where a chat's row says it stands, or nothing while nothing waits on anyone. */
export function rowOf(w: World, id: string) {
  const open = w.questions.find((q) => q.task === id && q.phase !== "closed");
  const wait = w.waits.find((one) => one.chat === id);
  // Said once, and only while it has something to ask (story 58).
  if (chatOf(w, id).noAdapter) return wait ? { kind: "own-pane" as const, since: wait.since } : null;
  if (open?.phase === "reached") return { kind: "needs-you" as const, since: order(open) };
  if (wait) return { kind: "needs-you" as const, since: wait.since };
  if (open) {
    const asker = askerOf(w, id);
    return { kind: "asking" as const, since: open.askedAt, who: asker ? nameOf(w, asker) : "" };
  }
  return null;
}

/* ---------- what happens ---------- */

export const at = (w: World, q: Question, said: string) =>
  q.trail.push({ at: w.clock, said });

export function ask(
  w: World,
  id: string,
  task: string,
  says: string,
  kind: Kind = "choices",
  options: string[] | undefined = kind === "choices" ? CHOICES : undefined,
  askedAt = w.clock,
): Question {
  const asker = askerOf(w, task);
  const q: Question = { id, task, kind, says, options, phase: "with-chat", askedAt, trail: [] };
  q.trail.push({ at: askedAt, said: COPY.trail.asked(nameOf(w, task)) });
  if (asker) q.trail.push({ at: askedAt, said: COPY.trail.handed(nameOf(w, asker)) });
  w.questions.push(q);
  return q;
}

function reach(w: World, q: Question, why: Why) {
  q.phase = "reached";
  q.why = why;
  q.reachedAt = w.clock;
  at(w, q, COPY.trail.reached);
}

export function passOn(w: World, q: Question, note?: string) {
  const chat = nameOf(w, askerOf(w, q.task) as string);
  q.note = note;
  at(w, q, note ? COPY.trail.passedNote(chat) : COPY.trail.passed(chat));
  reach(w, q, "passed-on");
}

export function noReply(w: World, q: Question) {
  at(w, q, COPY.trail.noReply(nameOf(w, askerOf(w, q.task) as string)));
  reach(w, q, "no-reply");
}

export function chatGone(w: World, q: Question, why: "gone" | "crashed" | "lost") {
  const chat = nameOf(w, askerOf(w, q.task) as string);
  at(w, q, why === "lost" ? COPY.trail.lost(chat) : COPY.trail.gone(chat));
  reach(w, q, why);
}

export function direct(w: World, q: Question, why: "nobody" | "asked-by-you") {
  reach(w, q, why);
}

/** An agent's answer: first answer wins; nothing in ✋ or Updates (story 36, 53). */
export function chatAnswers(w: World, q: Question, said: string) {
  const onScreen = q.phase === "reached";
  q.phase = "closed";
  q.closedBy = "chat";
  q.chatSaid = said;
  at(w, q, COPY.trail.chatAnswered(nameOf(w, askerOf(w, q.task) as string)));
  // A quiz with text in its box stays, keeping it (the rule of `AnswerQuestion`); else its line.
  if (onScreen) w.closing.push(q.id);
}

export function personAnswers(w: World, q: Question) {
  const late = q.phase === "closed" && q.closedBy === "chat";
  if (late) {
    q.overridden = true;
    at(w, q, COPY.trail.youOverrode(nameOf(w, askerOf(w, q.task) as string)));
    return;
  }
  const wasQuiz = quizInFront(w)?.id === q.id;
  q.phase = "closed";
  q.closedBy = "person";
  at(w, q, COPY.trail.youAnswered);
  delete w.typed[q.id];
  if (wasQuiz && reachedOf(w).some((one) => topOf(w, one.task) === w.front))
    w.pause = { task: q.task };
  w.firstLine = false;
}

export function taskEnds(w: World, q: Question) {
  q.phase = "closed";
  q.closedBy = "nobody";
  at(w, q, COPY.trail.taskEnded);
  w.updates.unshift({
    id: `end-${q.id}`,
    question: q.id,
    says: COPY.inbox.closedUnanswered(nameOf(w, q.task)),
  });
}

export function setWhere(w: World, where: Where) {
  if (w.where === where) return;
  w.where = where;
  for (const q of w.questions) if (q.phase !== "closed") at(w, q, COPY.trail.moved);
}

export function open(w: World, id: string) {
  if (!w.tabs.includes(id)) w.tabs.push(id);
  w.front = id;
  w.closing = [];
}

export function backgroundTurn(w: World, chat: string) {
  (w.printed[chat] ??= []).push(
    "⏺ Background task finished: the nightly report is written.",
    "⏺ Read 14 files and updated the notes.",
  );
}

export function restart(w: World) {
  for (const q of w.questions) if (q.phase !== "closed") at(w, q, COPY.trail.restart);
  w.pause = null;
  w.closing = [];
}

/* ---------- the states ---------- */

export type Pretend = { label: string; run: (w: World) => void };

export type Scene = {
  id: string;
  group: string;
  name: string;
  /** The spec's user stories this state draws. */
  stories: string;
  make: () => World;
  pretend?: Pretend[];
  /** The page is not the window: the words, or the checks. */
  page?: "words";
};

const q0 = (w: World, id: string) => questionOf(w, id);

const NOTE = "I can't tell which the docs meant, and it changes what people are billed.";
const CHAT_SAID = "Per project. The code is what customers are billed by.";

/** A question the person now has: from developer 7, passed on with a note. */
function oneQuiz(more: Partial<World> = {}): World {
  const w = world({ clock: 28, ...more });
  passOn(w, ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 27), NOTE);
  w.clock = 30;
  return w;
}

/** Three of them, oldest first. */
function threeQuizzes(more: Partial<World> = {}): World {
  const w = oneQuiz({ ...more, clock: 28 });
  w.clock = 29;
  noReply(w, ask(w, "q2", "design3", ASK.empty, "text", undefined, 26));
  w.clock = 30;
  passOn(w, ask(w, "q3", "arch2", ASK.migration, "choices", ["Split it", "Hand it all over"], 29));
  return w;
}

function prose(w: World, chat: string) {
  w.waits.push({
    id: "w1",
    chat,
    kind: "prose",
    says: "The branch is ready and every check passes. Should I open the pull request now, or wait for the other two tasks?",
    earlier: [
      "I read the spec and the three tickets it names.",
      "The route lives in the dispatch messages module, so I started there.",
      "Two tests failed on the old wording. I changed the tests, not the code.",
    ],
    since: w.clock - 3,
  });
}

const passPretend = (id: string): Pretend => ({
  label: "steward 4 passes it on",
  run: (w) => passOn(w, q0(w, id), NOTE),
});
const answerPretend = (id: string): Pretend => ({
  label: "steward 4 answers",
  run: (w) => chatAnswers(w, q0(w, id), CHAT_SAID),
});

export const SCENES: Scene[] = [
  /* ---- strip ---- */
  {
    id: "strip-ask",
    group: "Waiting strip",
    name: "A real ask, in your own chat",
    stories: "40, 43, 46, 48",
    make: () => {
      const w = world();
      w.waits.push({ id: "w1", chat: "steward", kind: "reply", says: ASK.fixture, since: 26 });
      return w;
    },
    pretend: [
      { label: "A later turn prints more", run: (w) => backgroundTurn(w, "steward") },
      { label: "You answer", run: (w) => void (w.waits = []) },
    ],
  },
  {
    id: "strip-long",
    group: "Waiting strip",
    name: "A long question, shown in full",
    stories: "41",
    make: () => {
      const w = world({ stripOpen: true });
      w.waits.push({ id: "w1", chat: "steward", kind: "reply", says: ASK.long, since: 24 });
      return w;
    },
  },
  {
    id: "strip-prose",
    group: "Waiting strip",
    name: "A prose question: its last message",
    stories: "44",
    make: () => {
      const w = world();
      prose(w, "steward");
      return w;
    },
  },
  {
    id: "strip-prose-earlier",
    group: "Waiting strip",
    name: "A prose question: the messages before it",
    stories: "44",
    make: () => {
      const w = world({ stripAt: 2 });
      prose(w, "steward");
      return w;
    },
  },
  {
    id: "strip-background",
    group: "Waiting strip",
    name: "A background turn runs while it waits",
    stories: "45, 43",
    make: () => {
      const w = world();
      w.waits.push({ id: "w1", chat: "steward", kind: "reply", says: ASK.fixture, since: 26 });
      backgroundTurn(w, "steward");
      return w;
    },
    pretend: [{ label: "Another background turn", run: (w) => backgroundTurn(w, "steward") }],
  },
  {
    id: "strip-permission",
    group: "Waiting strip",
    name: "A permission ask: its buttons stay put",
    stories: "46, 47, 15",
    make: () => {
      const w = world();
      w.waits.push({
        id: "w1",
        chat: "steward",
        kind: "permission",
        says: "Run npm publish --tag next",
        since: 29,
      });
      return w;
    },
    pretend: [{ label: "You answer", run: (w) => void (w.waits = []) }],
  },
  {
    id: "strip-gone",
    group: "Waiting strip",
    name: "Answered: the strip has gone",
    stories: "42",
    make: () => world(),
  },

  /* ---- quiz ---- */
  {
    id: "quiz-choices",
    group: "Top-tab quiz",
    name: "One quiz: choices, passed on with a note",
    stories: "9, 10, 11, 25, 39, 48, 52",
    make: () => oneQuiz(),
    pretend: [answerPretend("q1")],
  },
  {
    id: "quiz-text",
    group: "Top-tab quiz",
    name: "One quiz: a question to type an answer to",
    stories: "25, 52",
    make: () => {
      const w = world();
      passOn(w, ask(w, "q1", "design3", ASK.empty, "text", undefined, 28));
      return w;
    },
  },
  {
    id: "quiz-count",
    group: "Top-tab quiz",
    name: "One quiz with a count of the rest",
    stories: "32, 34",
    make: () => threeQuizzes(),
  },
  {
    id: "quiz-pause",
    group: "Top-tab quiz",
    name: "The short pause before the next quiz",
    stories: "33",
    make: () => {
      const w = threeQuizzes();
      personAnswers(w, q0(w, "q1"));
      return w;
    },
  },
  {
    id: "quiz-agent-closed",
    group: "Top-tab quiz",
    name: "A quiz closes after an agent answers",
    stories: "36, 12, 13, 53",
    make: () => {
      const w = oneQuiz();
      chatAnswers(w, q0(w, "q1"), CHAT_SAID);
      return w;
    },
  },
  {
    id: "quiz-agent-closed-typing",
    group: "Top-tab quiz",
    name: "An agent answers while you are typing",
    stories: "36, 37",
    make: () => {
      const w = world();
      passOn(w, ask(w, "q1", "design3", ASK.empty, "text", undefined, 28));
      w.typed.q1 = "Say when it closed, and keep it to one line";
      chatAnswers(w, q0(w, "q1"), "Blank. The Inbox should stay quiet.");
      return w;
    },
  },
  {
    id: "quiz-override",
    group: "Top-tab quiz",
    name: "Your later word replaces an agent's answer",
    stories: "37, 13",
    make: () => {
      const w = oneQuiz();
      chatAnswers(w, q0(w, "q1"), CHAT_SAID);
      personAnswers(w, q0(w, "q1"));
      return w;
    },
  },
  {
    id: "quiz-no-reply",
    group: "Top-tab quiz",
    name: "Why: no reply",
    stories: "18, 52",
    make: () => {
      const w = world();
      noReply(w, ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 28));
      return w;
    },
  },
  {
    id: "quiz-gone",
    group: "Top-tab quiz",
    name: "Why: the asking chat has closed or crashed",
    stories: "19, 52",
    make: () => {
      const w = oneQuizGone("gone");
      return w;
    },
    pretend: [
      {
        label: "It crashed instead",
        run: (w) => void (q0(w, "q1").why = "crashed"),
      },
    ],
  },
  {
    id: "quiz-lost",
    group: "Top-tab quiz",
    name: "Why: the connection was lost (remote runner)",
    stories: "63, 64",
    make: () => oneQuizGone("lost"),
  },
  {
    id: "quiz-nobody",
    group: "Top-tab quiz",
    name: "Why: nobody to ask",
    stories: "7, 52",
    make: () => {
      const w = world({ front: "talk" });
      direct(w, ask(w, "q1", "ops3", ASK.fixture, "choices", ["Re-record it", "Leave it"], 29), "asked-by-you");
      return w;
    },
  },
  {
    id: "quiz-nested",
    group: "Top-tab quiz",
    name: "A nested question, labelled with its chain",
    stories: "20, 21",
    make: () => {
      const w = world();
      passOn(w, ask(w, "q1", "qa5", ASK.clock, "choices", ["Add the fake clock", "Find another way"], 28), "This needs the person's say, it changes the core.");
      return w;
    },
  },
  {
    id: "quiz-focus",
    group: "Top-tab quiz",
    name: "You are typing as a quiz arrives",
    stories: "39, 16",
    make: () => {
      const w = world();
      ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 29);
      return w;
    },
    pretend: [passPretend("q1")],
  },
  {
    id: "quiz-elsewhere",
    group: "Top-tab quiz",
    name: "The top tab shows something else",
    stories: "38",
    make: () => {
      const w = threeQuizzes({ front: "settings", tabs: ["steward", "talk", "settings"] });
      return w;
    },
  },
  {
    id: "quiz-top-closed",
    group: "Top-tab quiz",
    name: "The top tab is closed",
    stories: "38",
    make: () => threeQuizzes({ front: "talk", tabs: ["talk"] }),
  },
  {
    id: "quiz-cannot-draw",
    group: "Top-tab quiz",
    name: "A quiz this window can't draw",
    stories: "62",
    make: () => {
      const w = world();
      passOn(w, ask(w, "q1", "dev7", ASK.limit, "cannot-draw", undefined, 28));
      return w;
    },
  },
  {
    id: "quiz-own-and-task",
    group: "Top-tab quiz",
    name: "Your chat waits too: strip and quiz together",
    stories: "46, 25",
    make: () => {
      const w = oneQuiz();
      prose(w, "steward");
      return w;
    },
  },
  {
    id: "quiz-restart",
    group: "Top-tab quiz",
    name: "After a restart, the question carries on",
    stories: "59, 60",
    make: () => {
      const w = oneQuiz({ panel: "activity", activityOf: "dev7" });
      restart(w);
      return w;
    },
  },
  {
    id: "quiz-browser",
    group: "Top-tab quiz",
    name: "A browser takeover comes straight to you, no quiz",
    stories: "6, 15",
    make: () => {
      const w = world({ panel: "inbox" });
      w.waits.push({
        id: "w1",
        chat: "dev7",
        kind: "browser",
        says: "The sign-in page shows a CAPTCHA. Solve it in the browser, then developer 7 carries on.",
        since: 29,
      });
      return w;
    },
  },

  /* ---- where you answer ---- */
  {
    id: "each-top-tab",
    group: "From each session",
    name: "The top tab stays calm",
    stories: "26, 17",
    make: () => threeQuizzes({ where: "each" }),
  },
  {
    id: "each-task-tab",
    group: "From each session",
    name: "The quiz waits in the task's own tab",
    stories: "26, 30",
    make: () => {
      const w = threeQuizzes({ where: "each" });
      open(w, "dev7");
      return w;
    },
  },
  {
    id: "top-answer-in-task",
    group: "From each session",
    name: "From the top, answering in the task's tab",
    stories: "30, 35",
    make: () => {
      const w = oneQuiz();
      open(w, "dev7");
      return w;
    },
  },
  {
    id: "each-change",
    group: "From each session",
    name: "Changing the setting moves open questions at once",
    stories: "29",
    make: () => threeQuizzes({ panel: null }),
    pretend: [
      { label: "Set from each session", run: (w) => setWhere(w, "each") },
      { label: "Set from the top", run: (w) => setWhere(w, "top") },
    ],
  },
  {
    id: "settings",
    group: "From each session",
    name: "Where you answer, in Settings",
    stories: "28",
    make: () => oneQuiz({ front: "settings", tabs: ["steward", "talk", "settings"] }),
  },

  /* ---- first-time line ---- */
  {
    id: "first-line",
    group: "First-time line",
    name: "One quiet line above your first question",
    stories: "27",
    make: () => oneQuiz({ firstLine: true }),
  },
  {
    id: "first-line-moved",
    group: "First-time line",
    name: "You chose from each session",
    stories: "27, 28, 29",
    make: () => {
      const w = oneQuiz({ movedLine: true, where: "each" });
      return w;
    },
  },
  {
    id: "first-line-next",
    group: "First-time line",
    name: "The next question has no line",
    stories: "27, 28",
    make: () => oneQuiz(),
  },

  /* ---- rows ---- */
  {
    id: "row-asking",
    group: "Task row",
    name: "In progress: with the asking chat",
    stories: "17, 49, 50, 51",
    make: () => {
      const w = world();
      ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 28);
      ask(w, "q2", "dev9", ASK.clock, "choices", undefined, 29);
      return w;
    },
    pretend: [
      passPretend("q1"),
      answerPretend("q1"),
      {
        label: "A minute passes",
        run: (w) => void (w.clock += 1),
      },
    ],
  },
  {
    id: "row-asking-open",
    group: "Task row",
    name: "In progress: open the task and answer first",
    stories: "16, 30, 13",
    make: () => {
      const w = world();
      ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 28);
      open(w, "dev7");
      return w;
    },
  },
  {
    id: "row-needs-you",
    group: "Task row",
    name: "Needs you: on the row, a total above it",
    stories: "49, 50, 51",
    make: () => {
      const w = threeQuizzes();
      ask(w, "q4", "dev9", ASK.clock, "choices", undefined, 29);
      return w;
    },
    pretend: [{ label: "A minute passes", run: (w) => void (w.clock += 1) }],
  },
  {
    id: "row-folded",
    group: "Task row",
    name: "A collapsed task still shows its state",
    stories: "31",
    make: () => {
      const w = threeQuizzes();
      ask(w, "q4", "dev9", ASK.clock, "choices", undefined, 29);
      chatOf(w, "arch2").folded = true;
      return w;
    },
  },
  {
    id: "row-no-adapter",
    group: "Task row",
    name: "A harness without an adapter says so once",
    stories: "58, 65",
    make: () => {
      const w = world({ panel: "inbox" });
      w.waits.push({ id: "w1", chat: "rev1", kind: "reply", says: ASK.fixture, since: 27 });
      return w;
    },
  },

  /* ---- inbox ---- */
  {
    id: "inbox-list",
    group: "Inbox",
    name: "Every question, to pick out of order",
    stories: "34, 35, 48",
    make: () => threeQuizzes({ panel: "inbox" }),
  },
  {
    id: "inbox-closed-unanswered",
    group: "Inbox",
    name: "A question closed unanswered: one Update",
    stories: "54, 61",
    make: () => {
      const w = oneQuiz({ panel: "inbox" });
      taskEnds(w, q0(w, "q1"));
      return w;
    },
  },
  {
    id: "inbox-agent-quiet",
    group: "Inbox",
    name: "An agent handled it: no Update",
    stories: "53",
    make: () => {
      const w = world({ panel: "inbox" });
      chatAnswers(w, ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 28), CHAT_SAID);
      return w;
    },
  },
  {
    id: "inbox-failed",
    group: "Inbox",
    name: "A redirect failed: said once",
    stories: "57",
    make: () => {
      const w = world({ panel: "inbox" });
      const q = ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 28);
      at(w, q, COPY.trail.redirectFailed);
      noReplyQuiet(w, q);
      w.updates.push({
        id: "fail",
        trouble: true,
        question: "q1",
        says: COPY.inbox.failed("developer 7", "steward 4"),
      });
      return w;
    },
  },

  /* ---- trail ---- */
  {
    id: "trail-agent",
    group: "Question trail",
    name: "An agent answered",
    stories: "55",
    make: () => {
      const w = oneQuiz({ panel: "activity", activityOf: "dev7" });
      chatAnswers(w, q0(w, "q1"), CHAT_SAID);
      w.closing = [];
      return w;
    },
  },
  {
    id: "trail-person",
    group: "Question trail",
    name: "No reply, then you answered",
    stories: "55",
    make: () => {
      const w = world({ panel: "activity", activityOf: "dev7" });
      const q = ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 28);
      w.clock = 29;
      noReply(w, q);
      w.clock = 34;
      personAnswers(w, q);
      return w;
    },
  },
  {
    id: "trail-override",
    group: "Question trail",
    name: "You replaced an agent's answer",
    stories: "55, 37",
    make: () => {
      const w = oneQuiz({ panel: "activity", activityOf: "dev7" });
      chatAnswers(w, q0(w, "q1"), CHAT_SAID);
      w.clock = 33;
      personAnswers(w, q0(w, "q1"));
      w.closing = [];
      return w;
    },
  },
  {
    id: "trail-ended",
    group: "Question trail",
    name: "The task ended, unanswered",
    stories: "55, 61",
    make: () => {
      const w = oneQuiz({ panel: "activity", activityOf: "dev7" });
      taskEnds(w, q0(w, "q1"));
      return w;
    },
  },

  {
    id: "words",
    group: "The words",
    name: "Every word, to judge",
    stories: "copy",
    make: () => world(),
    page: "words",
  },
];

function oneQuizGone(why: "gone" | "lost"): World {
  const w = world();
  chatGone(w, ask(w, "q1", "dev7", ASK.limit, "choices", undefined, 29), why);
  return w;
}

function noReplyQuiet(w: World, q: Question) {
  q.phase = "reached";
  q.why = "redirect";
  q.reachedAt = w.clock;
  at(w, q, COPY.trail.reached);
}
