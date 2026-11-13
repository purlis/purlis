// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  Fragment,
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Send, X } from "lucide-react";
import { RestartEndsLine } from "./RestartEnds";
import { commands, type Offered, type PlaneId, type ReachedWhy, type Shown } from "./bindings";
import { answerTaken, asksMoved, secretAskEntries } from "./asks";
import { counted as countedSaid } from "./dispatches";
import { DispatchGrantNotice } from "./DispatchGrantNotice";
import { KindIcon, kindOfAsk, RowAge } from "./InboxRow";
import { toneOfKind } from "./messageTone";
import { asksScopeOf, keptAsksScope, pickAsksScope, type AsksScope } from "./asksScope";
import { NoQuestion, ReadingTheQuestion, useAnswerQuestion } from "./AnswerQuestion";
import { askKey, heldKey, noteAnswered, replyBytes, ageOf, type Answered } from "./inboxRules";
import { type AllowedLately } from "./allowedLately";
import { NoticeWayGuard } from "./noticeSettle";
import { ChatText } from "./ChatText";
import { capMarks } from "./combiningMarks";
import { blankOf, marksIn, markOf } from "./invisible";
import { AskWrites } from "./AskWrites";
import { LoginSite, useTypedSite } from "./LoginSite";
import { RunFacts, SecretAskNotice, type SecretAskGiven } from "./SecretAskNotice";
import { AskDecision, REASON_TYPED_MOST, waysOf } from "./AskDecision";
import { RuleOffer, ruleMadeOf } from "./AskRules";
import { SecretFillSite } from "./SecretFillNotice";
import { Button, ButtonRow, IconButton } from "./ui/button/Button";
import {
  MessageBody,
  MessageForm,
  MessageLinks,
  MessageMeta,
  MessageRow,
  MessageState,
  MessageTop,
} from "./ui/message/Message";
import { Form, TextArea, TextField } from "./ui/field/Field";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { ChatMark } from "./ui/chat-mark/ChatMark";
import { ChatActions } from "./ChatActions";
import { shownState } from "./shownState";
import { highRisk } from "./asksSame";
import { Badge } from "./ui/badge/Badge";

import { chainSaid, timeSaid, useSettledPlace } from "./inboxRowParts";
import { askSeenKey, useSeenRow, type SeenRows } from "./seenRows";
import { useChatsHere, useChatsSelect } from "./chatState";
import { AskStanding } from "./AskStanding";
import type { Standing } from "./asksOrder";

// **The Asks view's rows** (#1692, #5644; moved from `Inbox.tsx`): what one ask offers, and how it
// is drawn and answered in place.

/**
 * **The Chat state of a chat an ask names** (ADR 0121 decisions 2 and 17, #6012): an ask is
 * listed because it stops its chat until the person answers, so its chat is in the needs-you
 * queue, whatever else is true of it. Read through the one function, as its row and its tab
 * read theirs, so Asks never says something they do not.
 */
const ASKING = shownState({ board: "unknown", needsYou: true, task: null, harness: null }).mark;

/** What the card of a chat waiting on a reply says where the app heard no message from it: its
 *  harness gave none, or the app was started since (#4550). */
export const NO_MESSAGE = "No message to show. Go to chat to see what it said.";

/** What the list of asks is called: the list family's grid of them (#3123). */
export const ASKS = "Asks";

/** What an empty Inbox says (I-12). */
export const NOTHING_WAITS = "Nothing waits on you";

/** How the empty Asks says the chats at work (ADR 0121 decision 23). */
export const workingSaid = (count: number) =>
  count === 1 ? "1 chat working" : `${count} chats working`;

/**
 * **Which chats a part of the empty Asks opens** (ADR 0121 decision 23): every chat, or those
 * working. All chats, a mode of the Chats sidebar, is #6013's; until it is built each opens the
 * Chats sidebar as it is, and the filter is the seam it fills. Stuck? (#6014) joins as `stuck`
 * once a chat can be marked so: until then the empty Asks says no "N stuck?".
 */
export type AllChatsFilter = "all" | "working";

/** How many of this project's chats are working now: read off the chats' store, so only the
 *  empty sentence is drawn again as a chat starts or stops. */
function useWorkingChats(): number {
  return useChatsSelect(
    useChatsHere(),
    (states) => Object.values(states.bySession).filter((state) => state === "running").length,
  );
}

/** The empty Asks' sentence: each part a way to the chats where `onAllChats` is given. */
function NothingWaits({ onAllChats }: { onAllChats?: (filter: AllChatsFilter) => void }) {
  const working = useWorkingChats();
  const parts: { said: string; filter: AllChatsFilter }[] = [
    { said: NOTHING_WAITS, filter: "all" },
    ...(working > 0 ? [{ said: workingSaid(working), filter: "working" as const }] : []),
  ];
  if (onAllChats === undefined)
    return <p className="inbox-none">{parts.map((part) => part.said).join(" · ")}</p>;
  return (
    <p className="inbox-none inbox-none-parts">
      {parts.map((part, at) => (
        <Fragment key={part.filter}>
          {at > 0 && <span aria-hidden="true">{" · "}</span>}
          <Button variant="link" size="sm" onPress={() => onAllChats(part.filter)}>
            {part.said}
          </Button>
        </Fragment>
      ))}
    </p>
  );
}

/**
 * **The asks whose answers are Allow and Keep blocked** (ADR 0097 §6): a permission prompt, a
 * sandbox's host, folder or local service, a browser's sites, and a refused vault. On these, each answer that
 * allows is drawn as the `grants` variant and every other in the plain style: equal in weight,
 * and told apart at a glance in every theme by the grant's edge and its open lock. A question's choices, a removal and a browser's reach are
 * answers of another kind and keep their own looks.
 */
export const ALLOW_OR_KEEP: ReadonlySet<Shown["source"]> = new Set<Shown["source"]>([
  "permission",
  "sandbox-host",
  "sandbox-write",
  "sandbox-local",
  "browser-site",
  "pool-login",
  "pool-tool",
  "browser-login",
  "run-grant",
  "secret-fill",
  "vault",
  "sandbox-expects",
]);

/** What one ask's row offers, decided once for its stops and its drawing. */
export type Shape = {
  /** Why the chat waits, said in place of what it asked. */
  why?: string;
  /** The line drawn: the reason, or what it asked, each run of stacked marks cut (#1146). */
  line: string;
  /** The answers drawn: none that allows, where the line is not what the ask said whole. */
  offers: readonly Offered[];
  /** The first character the line marks as one a reader does not see (`⟦U+XXXX⟧`), where that
   *  is why the core offers no Allow on a permission (SK-51, #3202). */
  hidden?: string;
  /** The lines a permission's action runs over, where its line puts several on one (#3502):
   *  the count, those drawn as lines (at most `MOST_LINES`, each run of stacked marks cut), and
   *  how many more it runs over than are drawn. */
  lines?: { count: number; drawn: string[]; more: number };
  /** Its answers are buttons here. */
  answers: boolean;
  /** It gets a reply box. */
  reply: boolean;
  /** Its line is what its chat said, or that there is no message (#4550), so it is drawn whole
   *  over the lines it has, and the status line above it names the card. */
  message: boolean;
  /** What its chat said, or that there is no message, where its line is the reason it waits
   *  instead. */
  said: string | undefined;
  /** It is a task's question, which the Answer form takes. */
  answerable: boolean;
  /** **It is answered here, behind its Answer link** (#5651, ADR 0119 decision 7): its answers,
   *  its reply box, its dispatch's Notice or a task's Answer form. An ask only its own place can
   *  answer (a prompt in its terminal, its browser waiting for a sign-in) has Go to chat alone. */
  form: boolean;
  /** It gets a box for why it is kept blocked, which its chat is told with the answer
   *  (#1987). */
  reason: boolean;
  /** It's a login's ask, whose site the person may change (#4595). */
  site: boolean;
  /**
   * **It is a decision, answered on its row** (#5998, ADR 0121 amended 2026-11-09): its options
   * stand on the row, with no Answer, and its title is {@link Shape.question}. Only an ask that
   * needs typing (a reply, a secret's value) keeps its form behind Answer.
   */
  decision: boolean;
  /** What a decision asks, as its row's title. */
  question?: string;
  /** What an ask behind Answer needs typed, said on its row: a run's or a secret's value. */
  needs?: string;
  /** It has the queue's Ignore. */
  ignore: boolean;
  /** It has Go to chat: every ask but a profile's, which is about no chat (#2035). */
  go: boolean;
};

export function shapeOf(
  ask: Shown,
  whyOf: ((session: number) => string | undefined) | undefined,
  onIgnore: ((session: number) => void) | undefined,
  asked: ((session: number) => boolean) | undefined,
): Shape {
  const waits = ask.source === "question" || ask.source === "terminal";
  const why = ask.source === "question" ? whyOf?.(ask.session) : undefined;
  // A chat that both asked and has a reason (a report with nowhere to go, say) still asked:
  // while it is itself waiting, it gets the box beside the reason (#1700).
  const reply = ask.source === "question" && (why === undefined || asked?.(ask.session) === true);
  const message = reply && why === undefined;
  // **A line cut is not the ask, so nothing that lets it run is offered on it** (#1146): the
  // core offers Allow only where the line says the action word for word
  // (`hooked::shown_in_full`), and a run of combining marks cut here would undo that.
  const { text: line, capped } = capMarks(
    why ?? (message ? (ask.last_message ?? NO_MESSAGE) : ask.says),
  );
  // An edit whose change is not shown is never allowed from here (#1146), whatever it offers.
  const unshown = ask.writes?.kind === "not-shown";
  const offers =
    (capped && why === undefined) || unshown
      ? ask.options.filter((one) => !one.allows)
      : ask.options;
  const withheld =
    ask.source === "permission" && why === undefined && !offers.some((one) => one.allows);
  // A browser waiting for a sign-in is answered in the browser itself, which its chat shows
  // (#5651): its Take over is that place's own, not an answer drawn here.
  const answers =
    ask.source !== "dispatch" &&
    ask.options.length > 0 &&
    ask.answer.via !== "in-its-pane" &&
    ask.answer.via !== "take-over";
  const answerable = ask.source === "task-question";
  const needs = needsOf(ask);
  const decision =
    needs === undefined &&
    ask.login == null &&
    (ask.source === "dispatch" || (answers && DECISIONS.has(ask.source)));
  return {
    why,
    line,
    offers,
    hidden: withheld ? withheldFor(line) : undefined,
    lines: withheld ? linesOf(ask) : undefined,
    answers,
    reply,
    message,
    said: reply && !message ? capMarks(ask.last_message ?? NO_MESSAGE).text : undefined,
    answerable,
    form: !decision && (ask.source === "dispatch" || reply || answers || answerable),
    site: ask.login != null,
    decision,
    question: decision ? questionOf(ask) : undefined,
    needs,
    reason:
      (ask.answer.via === "sandbox-block" ||
        ask.answer.via === "browser" ||
        ask.answer.via === "browser-sites" ||
        ask.answer.via === "browser-login" ||
        ask.answer.via === "secret-fill" ||
        ask.answer.via === "vault") &&
      ask.options.some((one) => one.id === "keep"),
    // A chat that only waits for the person's next word asks again at its next stop, so it
    // may be put away, as the queue always let it be. A decision never is.
    ignore: waits && onIgnore !== undefined,
    // An ask of no chat's (session 0: a sign-in, a sandbox-only profile, purlis's own) has no
    // chat to go to.
    go: ask.session !== 0 && ask.source !== "sign-in" && ask.source !== "sandbox-only",
  };
}

/**
 * **The kinds of ask that are decisions** (#5998, ADR 0121 amended 2026-11-09): a permission, a
 * run grant, a sandbox's host, folder or service, and a browser's site. A dispatch's grant, its
 * budget and its loop come as a dispatch's ask, which is one too.
 */
export const DECISIONS: ReadonlySet<Shown["source"]> = new Set<Shown["source"]>([
  "permission",
  "run-grant",
  "sandbox-host",
  "sandbox-write",
  "sandbox-local",
  "browser",
  "browser-site",
  // What the project expects of this machine, and a command's confirmation (#6000).
  "sandbox-expects",
  "confirmation",
  // A chat paused at its budget or as looping (#6015).
  "budget",
  "looping",
]);

/** What a decision of each kind asks, as its row's title: a question. */
const QUESTION: Partial<Record<Shown["source"], string>> = {
  permission: "Allow this action?",
  dispatch: "Allow this dispatch?",
  "sandbox-host": "Allow reaching this host?",
  "sandbox-write": "Allow writing in this folder?",
  "sandbox-local": "Allow reaching this service?",
  browser: "Let its browser reach this?",
  "browser-site": "Let its browser reach this site?",
  "sandbox-expects": "Allow what this project expects?",
  confirmation: "Confirm what this command asks?",
  budget: "Let it go on past its budget?",
  looping: "Let it go on?",
};

/** **The question a decision asks** (#5998), named as the person reads it: a run grant names
 *  its program and its vault ("Allow curl to use vault team?"). */
export function questionOf(ask: Shown): string {
  const run = ask.run ?? undefined;
  if (run !== undefined) {
    const program = run.program.split("/").pop() ?? run.program;
    return run.offer === "vault"
      ? `Allow ${run.persona} to use vault ${run.vault}?`
      : `Allow ${program} to use vault ${run.vault}?`;
  }
  return QUESTION[ask.source] ?? "Allow this?";
}

/** The keys said whole: `A`, `A and B`, `A, B and C`. */
const keysSaid = (keys: readonly string[]) =>
  keys.length < 2
    ? (keys[0] ?? "")
    : `${keys.slice(0, -1).join(", ")} and ${keys[keys.length - 1]}`;

/** **What an ask needs typed before it is answered** (#5998): a secret's value, or a run's values
 *  its vault lacks. Said on its row, whose Answer opens the boxes. */
function needsOf(ask: Shown): string | undefined {
  if (ask.secret != null) return `Needs a value for ${ask.secret.key}`;
  const run = ask.run ?? undefined;
  if (run === undefined) return undefined;
  if (run.fills.length > 0) return `Needs a value for ${keysSaid(run.fills.map((one) => one.key))}`;
  return run.offer === "fill" ? "Needs a value" : undefined;
}

/**
 * **The mark that keeps a permission's Allow away** (SK-51, #3202): the core offers Allow only
 * on a line that says its action word for word and hides nothing by purlis's one rule
 * (`hooked::shown_in_full`), and the registry writes each character it hides as its mark
 * (`asking::said_inertly`), a `⟦` the chat wrote itself as `⟦U+27E6⟧`. So any mark but that one
 * on the line is a character the action holds that reads otherwise here: a no-break or an en
 * space that looks like a space, a control, a bidi override. Where it holds several, the row
 * names the one that misleads most (#3470): a line break, which can hide a second command, then
 * any other hidden character, then a blank that looks like a space.
 */
function withheldFor(line: string): string | undefined {
  const marks = marksIn(line)
    .map((one) => one.mark)
    .filter((one) => one !== markOf("\u27E6"));
  const rank = (mark: string) => {
    const kind = blankOf(mark);
    return kind === "break" ? 0 : kind === "blank" ? 2 : 1;
  };
  return marks.reduce<string | undefined>(
    (best, one) => (best === undefined || rank(one) < rank(best) ? one : best),
    undefined,
  );
}

/** The most lines of a command drawn on its row (#3502); the rest are counted. */
export const MOST_LINES = 5;

/** **The lines a permission's action runs over** (#3502): the core says how many where the
 *  ask's line puts several on one, and gives each as the line has it (`asking::lines_of`). They
 *  are the line, split where the command breaks one, so drawing them shows nothing it does not. */
function linesOf(ask: Shown): Shape["lines"] {
  if (ask.lines === undefined || ask.lines === null || ask.lines.count < 2) return undefined;
  const drawn = ask.lines.shown.slice(0, MOST_LINES).map((one) => capMarks(one).text);
  return { count: ask.lines.count, drawn, more: Math.max(0, ask.lines.count - drawn.length) };
}

/** Why a permission over several lines offers no Allow (#3502): its line is not what runs. */
export const linesSaid = (lines: NonNullable<Shape["lines"]>) =>
  lines.drawn.length > 0
    ? `Allow is not offered. It runs over ${lines.count} lines.`
    : `Allow is not offered. It runs over ${lines.count} lines, put on one here.`;

/** What a row says of the lines it does not draw. */
const moreLines = (more: number) => `and ${countedSaid(more, "more line", "more lines")}`;

/** An ask's line, each mark on it drawn as one (`invisible-char`), apart from the words. Its
 *  parts, not a component of its own, so the tooltip still reads the words it repeats. */
function markedLine(line: string): ReactNode[] {
  const parts: ReactNode[] = [];
  let at = 0;
  for (const { mark, at: from } of marksIn(line)) {
    parts.push(line.slice(at, from));
    parts.push(
      <mark key={from} className="invisible-char">
        {mark}
      </mark>,
    );
    at = from + mark.length;
  }
  parts.push(line.slice(at));
  return parts;
}

/** **What one ask's row draws, in its order** (#1693, #3123): the row, its links, its form's
 *  answers and boxes only while `open`, its ×. A row below it stands under these, and moves when
 *  one comes or goes (`useSettledPlace`). A fold's button is not listed: whether it is drawn is
 *  `ChatText`'s measure, and its growing is heard where each Allow is drawn (#3167). */
export function placeIds(
  key: string,
  shape: Shape,
  options: readonly Offered[] = [],
  open = false,
): string[] {
  // The row's order (#5649): its links, what answers it in the row (#5651: while its Answer
  // link is open), then its ×.
  return [
    key,
    ...(shape.form ? [`${key}:answer`] : []),
    // A decision's answers stand on its row (#5998): the narrowest grant, Other scopes… and
    // the refusals, before Go to chat.
    ...(shape.decision ? decisionPlaces(key, options) : []),
    // A profile's Ask is about no chat (#2035): there is no chat to go to.
    ...(shape.go ? [`${key}:go`] : []),
    // What a decision opens under its line: its wider grants, or why it is kept blocked.
    ...(open && shape.decision ? [`${key}:under`] : []),
    ...(open && !shape.decision
      ? [
          ...(shape.site ? [`${key}:site`] : []),
          ...(shape.reason ? [`${key}:reason`] : []),
          ...options.map((option) => `${key}:${option.id}`),
          ...(shape.reply ? [`${key}:reply`, `${key}:send`] : []),
          ...(shape.answerable ? [`${key}:task-answer`, `${key}:task-send`] : []),
        ]
      : []),
    ...(shape.ignore ? [`${key}:ignore`] : []),
  ];
}

/** The stops of a decision's answers on its row, in the order {@link AskDecision} draws them. */
function decisionPlaces(key: string, options: readonly Offered[]): string[] {
  const { main, wider, rest } = waysOf(options);
  return [
    ...(main === undefined ? [] : [`${key}:${main.id}`]),
    ...(wider.length > 0 ? [`${key}:scopes`] : []),
    ...rest.map((one) => `${key}:${one.id}`),
  ];
}

/** An edit's change, drawn above its answers, which wait for it (#1146); any other ask's
 *  answers as they are. */
function WithWrites({
  ask,
  onDrawn,
  children,
}: {
  ask: Shown;
  onDrawn: () => void;
  children: ReactNode;
}) {
  if (ask.writes === undefined || ask.writes === null) return <>{children}</>;
  return (
    <AskWrites writes={ask.writes} onDrawn={onDrawn}>
      {children}
    </AskWrites>
  );
}

/** What the row says a hidden character does, by the kind of blank it is (#3470). */
const DOES = {
  break: "that breaks the line here",
  blank: "that looks like another one here",
  hidden: "a reader does not see here",
} as const;

/** What a row says of a hidden character a permission holds. */
const holdsSaid = (mark: string, also = false) =>
  `It ${also ? "also holds" : "holds"} ${mark}, a character ${DOES[blankOf(mark) ?? "hidden"]}.`;

/** Why a permission holding `mark` offers no Allow (SK-51, #3202; #3470). */
export const hiddenSaid = (mark: string) => `Allow is not offered. ${holdsSaid(mark)}`;

/** Why a permission over several lines offers no Allow, and the hidden character it holds too,
 *  where it holds one (#3502). */
const withheldSaid = (lines: NonNullable<Shape["lines"]>, mark: string | undefined) =>
  mark === undefined ? linesSaid(lines) : `${linesSaid(lines)} ${holdsSaid(mark, true)}`;

/** What a row says of where it waits. */
export const SOURCE_SAID: Record<Shown["source"], string> = {
  permission: "Asks your permission",
  dispatch: "Asks to dispatch",
  "sandbox-host": "Asks to reach a host",
  "sandbox-write": "Asks to write in a folder",
  "sandbox-local": "Asks to reach a service on this machine",
  "workspace-removal": "Asks you to remove a workspace",
  "proposed-change": "Proposes a change for you to apply",
  "runner-write": "Asks, from a pool's runner, to change this project",
  "runner-dispatch": "Asks, from a pool's runner, to start a chat here",
  "sign-in": "Asks you to sign a profile in",
  "sandbox-only": "Asks you to make a sandbox-only profile",
  "old-harness-plugins": "Old harness plugin settings, ignored now",
  signing: "Lanes wait on signed commits",
  browser: "Asks what its browser may reach",
  "browser-needs-you": "Needs you in its browser",
  "browser-site": "Asks to let its browser reach a new site",
  "pool-login": "Asks to log its browser in on a pool's runner",
  "pool-tool": "Asks to send a tool its vault values for a run on a pool's runner",
  secret: "Asks you for a secret its vault doesn't hold",
  vault: "Refused a vault its persona isn't tagged for",
  "browser-login": "Asks to log its browser in to a site with vault credentials",
  "secret-fill": "Asks to put a vault secret into a field of its browser",
  "run-grant": "Asks you to allow a program to use a vault secret",
  terminal: "In its terminal",
  question: "Waiting on your reply",
  "task-question": "Asks you, through its asking chat",
  "sandbox-expects": "Asks you to allow what this project expects",
  confirmation: "A command waits on your confirmation",
  budget: "Paused at its budget",
  looping: "Paused, as it may be looping",
  "merge-hold": "Asks you to merge a request",
  "trust-clone": "Asks you to trust a clone",
  "trust-clones": "Asks you to trust clones purlis did not clone",
};

/** Why a task's question reached the person, as its row says it. */
const REACHED_SAID: Record<ReachedWhy, (from: string) => string> = {
  "passed-on": (from) => `${from} passed this on`,
  "no-reply": (from) => `${from} didn't reply`,
  "asker-gone": (from) => `${from} has closed`,
};

export function Ask({
  plane,
  ask,
  dispatches,
  shape,
  open,
  onOpen,
  onGo,
  onAnswered,
  onIgnore,
  onSnooze,
  onStop,
  onTouched,
  above,
  where,
  now,
  standing,
  theirs = false,
  seen,
}: {
  plane: PlaneId;
  ask: Shown;
  /** **Why it stands where it does in Asks** (#6006), said on its reason line. */
  standing?: Standing;
  /** Where its chat is: project · workspace · persona, as much as the window says. */
  where: string;
  /** The time its age is said against. */
  now: number;
  /** Its chat's dispatch asks, which its dispatch Notice draws its answers from (#1700). */
  dispatches: readonly Shown[];
  shape: Shape;
  /** Its Answer is open: its form is drawn in the row (#5651). */
  open: boolean;
  /** Its Answer, or Cancel, was pressed. */
  onOpen: (open: boolean) => void;
  /** The stops drawn above it: one changing moves it (`useSettledPlace`). */
  above: string;
  onGo: () => void;
  onAnswered?: (ask: Shown, option: Offered, live: boolean) => void;
  onIgnore?: (session: number) => void;
  /** Snooze its chat, with every ask it holds (#6013). Nothing where Asks snoozes nothing. */
  onSnooze?: () => void;
  /** Stop its chat (#6013). Nothing where its chat cannot be stopped from here. */
  onStop?: () => void;
  /** A way out of the Notice drawn in the row was pressed, with its words. */
  onTouched: (answer: string) => void;
  /**
   * **It is another open project's ask** (#1921), listed here and answered in `plane`, its own
   * project: each answer first checks it still waits there ({@link SETTLED_ELSEWHERE}).
   */
  theirs?: boolean;
  /** Its project's seen record where it is another's (#1921), else the one drawn under. */
  seen?: SeenRows | null;
}) {
  const id = useId();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  /** Why the person keeps it blocked, if they say: told to the chat with the answer. */
  const [reason, setReason] = useState("");
  /** The site as the person changed it, on a login's ask (#4595). */
  const [typed, setTyped] = useTypedSite(ask);
  /** **The grant pressed on a high-risk ask answered in its form, asking once more** (#6178),
   *  as a decision's does (`AskDecision`, #6011): with what a secret's notice gave, read only
   *  once its Allow there is pressed, and the words its pressed button showed, which the
   *  question's own Allow repeats. It holds only for the ask, the question and the answer it
   *  was asked of (Q242): where the row moves under it (a held merge at a new head, say), the
   *  question is taken back, and nothing the person pressed carries over. */
  const [askedOf, setAsking] = useState<{
    option: Offered;
    given?: () => SecretAskGiven;
    said: string;
    of: string;
  }>();
  const question = [ask.ask, ask.risk?.confirm ?? "", JSON.stringify(ask.answer)].join("\n");
  const asking = askedOf?.of === question ? askedOf : undefined;
  const who = chainSaid(ask.chain);
  /** A secret's one notice draws its own answers (#5080). */
  const secretNotice = ask.secret != null || ask.run != null;
  // Another project's ask is known by its project too (#1921): two projects' asks may share
  // a chat's number and an ask's id.
  const key = theirs ? heldKey(ask, plane) : askKey(ask);
  /** The task a task's question is from, as its Answer form names it. */
  const task = ask.chain[ask.chain.length - 1] ?? who;
  /** Its words are drawn whole: it is a dispatch's own Notice, or an answer here allows. */
  const whole =
    ask.source === "dispatch" || (shape.answers && shape.offers.some((one) => one.allows));
  /** Its words are a chat's own, not a command's or purlis's: a message it ended its turn on,
   *  or a task's question (#5214). Never an ask an answer here allows, which is read as it is. */
  const chatWords =
    shape.message || (ask.source === "task-question" && !whole && shape.lines === undefined);

  // An edit's change arrives after its row (#1146): its Allow waits to settle from then.
  const [changeDrawn, setChangeDrawn] = useState(false);
  const onChangeDrawn = useCallback(() => setChangeDrawn(true), []);
  const here = [
    above,
    shape.line,
    ...shape.offers.map((one) => one.label),
    // A rule offered under its answers (#6010) is a way drawn anew too.
    ask.rule?.offer?.options.join(",") ?? "",
    changeDrawn ? "drawn" : "",
    // **An Answer just opened is a form just drawn** (#5651): the keyboard lands on its first
    // answer, so an Allow there waits to settle as one drawn anew does, and a second Enter on
    // the link's heels allows nothing.
    open ? "open" : "",
    // A login's notice drawn anew has to be read anew (#4595).
    ask.answer.via === "browser-login" || ask.answer.via === "secret-fill" ? ask.answer.shown : "",
    // The question asked once more is drawn anew, and its Allow waits to settle (#6178).
    asking === undefined ? "" : `confirming ${asking.option.id}`,
  ].join("\n");
  /** Its watch to be seen, once its row is attached (`useSettledPlace`). */
  const seenAttached = useRef<() => void>(undefined);
  const { drawn, drawnAt, tooSoon, readPlaces } = useSettledPlace<HTMLElement>(here, () =>
    seenAttached.current?.(),
  );
  // **New until seen** (#6002): its accent bar, until it has been in sight for about a second.
  const [isNew, seenDrawnNow] = useSeenRow(askSeenKey(ask), drawn, seen);
  useLayoutEffect(() => {
    seenAttached.current = seenDrawnNow;
  });
  /** **The keyboard goes into the form its Answer opens** (#5651): onto its box, or its first
   *  answer. A task's box takes it itself once its question is read (`useAnswerQuestion`). */
  const focusForm = useRef(false);
  /** After each drawing of its row (the list family draws it a commit after this, #3123): where
   *  its answers stand is read, and a form its Answer just opened takes the keyboard. */
  const drawnNow = () => {
    readPlaces();
    seenDrawnNow();
    if (!open || !focusForm.current) return;
    const form = document.getElementById(`${id}-form`);
    if (form === null) return;
    focusForm.current = false;
    form.querySelector<HTMLElement>("textarea, input, button")?.focus();
  };
  /** **The settle guard on each of a decision's ways** (#5998, ADR 0097): a press just after
   *  its row was drawn or moved does nothing, and says why. */
  const mayAct = () => {
    if (!tooSoon()) return true;
    setSaid(MOVED_JUST_NOW);
    return false;
  };
  /** **A press in its form** (#6178): a grant on a high-risk ask asks once more under its
   *  answers, its Allow saying what the button pressed said (`said`, the option's label unless
   *  the form drew other words), a second press of it takes the question back, and every other
   *  answer acts. */
  const answerOrAsk = (option: Offered, given?: () => SecretAskGiven, said = option.label) => {
    if (ask.risk != null && option.allows) {
      setAsking(
        asking?.option.id === option.id ? undefined : { option, given, said, of: question },
      );
      return;
    }
    answer(option, given);
  };
  /** Whether `option` is an Allow on an ask of Allow and Keep blocked (ADR 0097 §6). */
  const grants = (option: Offered) => option.allows && ALLOW_OR_KEEP.has(ask.source);
  /** A press of `option`; on a secret's one notice (#5080), with what it gave, read once. */
  const answer = (
    option: Offered,
    given?: () => SecretAskGiven,
    kept = reason,
    /** Pressed on the rule its row offers (#6010): kept in Rules I made once it went through. */
    asRule = false,
  ) => {
    if (busy) return;
    // **An Allow waits for the row to settle** (#1695), as an update's does: with the hand's
    // list retired, the Inbox is where a permission or a host is allowed from outside its
    // pane, and an ask joining above must not put another's Allow under the pointer.
    if (option.allows && tooSoon()) {
      setSaid(MOVED_JUST_NOW);
      return;
    }
    setBusy(true);
    setSaid(undefined);
    // A secret's values are read from their boxes as the answer is pressed, and the boxes
    // emptied: only now, once the row has settled (#5080).
    const got = given?.();
    // **Another project's ask is answered only while it still waits there** (#1921): this list
    // is what its window last told, so its project's asks are read again first, and an ask
    // answered meanwhile, in its own window or anywhere, is sent nothing.
    void (theirs ? stillWaits(plane, ask) : Promise.resolve(true))
      .then((waits) =>
        waits === true
          ? answerTaken(
              plane,
              ask,
              option.id,
              option.id === "keep" ? kept : undefined,
              got === undefined ? typed : got.typed,
              got?.picked,
              got?.remember,
              got?.run,
              got?.named,
            )
          : {
              refused: waits === false ? SETTLED_ELSEWHERE : NOT_CHECKED_THERE,
              live: false,
            },
      )
      .then(({ refused, live, opened, said: answeredSaid, allowedList }) => {
        if (refused !== undefined) {
          setSaid(refused);
          return;
        }
        // A dialog of the window's was opened, where the person decides (qw356): the ask
        // still waits, and nothing was answered here.
        if (opened === true) return;
        noteAnswered(plane, {
          at: Date.now(),
          session: ask.session,
          chain: ask.chain,
          says: ask.says,
          answer: option.label,
          ...(answeredSaid === undefined ? {} : { said: answeredSaid }),
          ...(allowedList === true ? { allowedList } : {}),
        });
        if (asRule) void commands.ruleMade(plane, ruleMadeOf(ask, option)).catch(() => undefined);
        onAnswered?.(ask, option, live);
      })
      .finally(() => {
        setBusy(false);
        // Whatever it answered, the list is read again: a refusal may mean it went elsewhere.
        asksMoved(plane);
      });
  };

  const body =
    ask.source === "dispatch" ? (
      // The dispatch's own Notice, whole: what an Allow is bound to is read before it. Only a
      // press of one of its ways out counts as answering it here, never a tick or a scroll.
      <div className="inbox-notice">
        <NoticeWayGuard
          value={(way) => {
            // **Its answers wait for the row to settle** (#1695), as an ask's Allow does: the
            // Notice grants, and what is under the pointer or the keyboard must be what the
            // person read.
            if (tooSoon()) {
              setSaid(MOVED_JUST_NOW);
              return false;
            }
            setSaid(undefined);
            if (way.textContent) onTouched(way.textContent);
            return true;
          }}
        >
          <DispatchGrantNotice plane={plane} session={ask.session} asks={dispatches} />
        </NoticeWayGuard>
      </div>
    ) : shape.reply ? (
      <Reply plane={plane} ask={ask} />
    ) : shape.answerable ? (
      <TaskAnswer plane={plane} ask={ask} task={task} />
    ) : shape.answers ? (
      <WithWrites ask={ask} onDrawn={onChangeDrawn}>
        {ask.login != null && <LoginSite login={ask.login} typed={typed} onTyped={setTyped} />}
        {/* A secret's one notice (#5080): its facts, the chat's words, its fields, its Allow
            and its own answers, from the secret-ask family. */}
        {secretNotice && (
          <SecretAskNotice
            ask={ask}
            chat={who === "" ? "The chat" : who}
            entries={() => secretAskEntries(plane, ask)}
            busy={busy}
            onAnswer={answerOrAsk}
            onSaid={setSaid}
          />
        )}
        {/* What a secret fill puts where (#5115): words to read, nothing to reach. */}
        {ask.fill != null && <SecretFillSite fill={ask.fill} />}
        {shape.reason && (
          <TextField
            className="inbox-reason"
            aria-label={`Why you keep it blocked, told to ${who}`}
            value={reason}
            placeholder="Why keep it blocked? (optional, the chat is told)"
            maxLength={REASON_TYPED_MOST}
            spellCheck
            onChange={setReason}
          />
        )}
        {!secretNotice && (
          <ButtonRow className="inbox-answers" role="group" aria-label={`Answers to ${who}`}>
            {shape.offers.map((option) => (
              // Never `disabled` while busy: a disabled button drops the keyboard to the page
              // (#1700). It says it is busy, and `answer` sends nothing a second time.
              <Button
                key={option.id}
                // The answer the source suggests is the primary one, in its place (#2241): a
                // persona's declared browser. Still only the person's press answers. On an ask
                // of Allow and Keep blocked, each Allow is the grants variant, as plain as Keep
                // blocked and told apart by its edge and its lock, and never the primary look
                // (ADR 0097 §6).
                variant={
                  grants(option) ? "grants" : option.preselected === true ? "primary" : "secondary"
                }
                className={option.allows ? "inbox-answer allows" : "inbox-answer"}
                aria-disabled={busy || undefined}
                aria-busy={busy || undefined}
                aria-expanded={
                  ask.risk != null && option.allows ? asking?.option.id === option.id : undefined
                }
                onPress={() => answerOrAsk(option)}
              >
                {option.label}
              </Button>
            ))}
          </ButtonRow>
        )}
        {/* **Asked once more** (ADR 0121 decision 8, #6178): only the Allow here answers, held
              by the settle guard as every grant is; Cancel takes the question back. */}
        {ask.risk != null && asking !== undefined && (
          <ButtonRow className="inbox-answers" role="group" aria-label={ask.risk.confirm}>
            <span className="ask-confirm ui-text-strong">{ask.risk.confirm}</span>
            <Button
              variant={grants(asking.option) ? "grants" : "primary"}
              className="inbox-answer allows"
              aria-disabled={busy || undefined}
              aria-busy={busy || undefined}
              onPress={() => answer(asking.option, asking.given)}
            >
              {asking.said}
            </Button>
            <Button variant="secondary" onPress={() => setAsking(undefined)}>
              Cancel
            </Button>
          </ButtonRow>
        )}
      </WithWrites>
    ) : null;

  // **A row of Asks** (#5649, ADR 0119 decision 6), a row of the list family's (#3123): its
  // kind's glyph in the waiting tint; the chat's name with how long it has waited at the right,
  // the × in its place under the pointer; what it waits on (in the waiting colour) and where; its
  // words; its links (Answer, Go to chat); and what answers it, in the row.
  return (
    <MessageRow
      as="row"
      id={key}
      textValue={`${who}: ${shape.line}`}
      tone={toneOfKind(kindOfAsk(ask.source))}
      icon={<KindIcon kind={kindOfAsk(ask.source)} />}
      ref={drawnAt}
      onDrawn={drawnNow}
      isNew={isNew}
      className="inbox-ask"
      data-source={ask.source}
      // **Keyboard triage never lands here** (ADR 0121 decision 8, #6011, `asksTriage`).
      data-risk={highRisk(ask) ? "" : undefined}
      aria-labelledby={shape.message ? `${id}-whose` : `${id}-says`}
      aria-describedby={`${
        shape.message ? `${id}-says` : `${id}-whose${shape.said === undefined ? "" : ` ${id}-said`}`
      }${standing === undefined ? "" : ` ${id}-standing`}${ask.risk == null ? "" : ` ${id}-risk`}`}
      dismiss={
        shape.ignore && onIgnore !== undefined ? (
          <IconButton
            icon={<X />}
            size="sm"
            variant="quiet"
            aria-label={`Ignore ${who} until it asks again`}
            onPress={() => onIgnore(ask.session)}
          />
        ) : undefined
      }
    >
      <MessageTop
        title={
          // **A decision's title is its question** (#5998), and its chat's name leads its meta.
          <>
            {ask.session !== 0 && <ChatMark state={ASKING} className="inbox-chat-state" />}
            {shape.question === undefined ? (
              <span className="inbox-who">{who}</span>
            ) : (
              <span className="inbox-question">{shape.question}</span>
            )}
          </>
        }
        time={<RowAge when={ageOf(plane, ask)[0]} now={now} since="Waiting since" />}
      />
      <MessageMeta>
        {shape.question !== undefined && (
          <>
            <span className="inbox-who">{who}</span>
            {" · "}
          </>
        )}
        <MessageState id={`${id}-whose`}>{SOURCE_SAID[ask.source]}</MessageState>
        {where !== "" && (
          <>
            {" · "}
            <span className="inbox-where">{where}</span>
          </>
        )}
      </MessageMeta>
      {standing !== undefined && <AskStanding id={`${id}-standing`} why={standing} now={now} />}
      <MessageBody>
        {/* **Its risk, said first and spoken with the row** (ADR 0121 decision 8, #6011, #6020):
              in the waiting tone with its words, never a colour alone. */}
        {ask.risk != null && (
          <p id={`${id}-risk`} className="inbox-risk ui-text-secondary">
            {HIGH_RISK} {ask.risk.why}
          </p>
        )}
        {/* The ask's words are the source's, drawn as text and never as a control. What an
              Allow here would let run is never cut (#1146): it is read whole first. **A chat's
              own words** (its message, a task's question) are markdown under the safe subset
              (`ChatText`, #5214), folded past three lines with Show all (#5649): the person
              answers them here, and a question is often a message's last line (#4550). */}
        {chatWords ? (
          <ChatText
            id={`${id}-says`}
            className="ask-says inbox-primary inbox-chat-text"
            text={shape.line}
            foldAt={ASK_FOLD_LINES}
          />
        ) : (
          <Tooltip tip={shape.line}>
            {shape.lines !== undefined && shape.lines.drawn.length > 0 ? (
              // **A command over several lines is drawn on them** (#3502): each as the line has
              // it, the first few, and how many more. Never clamped: the cap is its own.
              <span
                id={`${id}-says`}
                className="ask-says ask-lines inbox-primary ui-text-body ui-text-strong"
              >
                {shape.lines.drawn.map((one, at) => (
                  // A space between lines, which a column does not draw, so the row's name
                  // reads them apart.
                  <Fragment key={at}>
                    {at > 0 && " "}
                    <span className="ask-line">{markedLine(one)}</span>
                  </Fragment>
                ))}
                {shape.lines.more > 0 && (
                  <>
                    {" "}
                    <span className="ask-line ask-more-lines ui-text-secondary">
                      {moreLines(shape.lines.more)}
                    </span>
                  </>
                )}
              </span>
            ) : (
              <span
                id={`${id}-says`}
                className={`ask-says inbox-primary ui-text-body ui-text-strong${whole ? "" : " inbox-clamped"}`}
              >
                {markedLine(shape.line)}
              </span>
            )}
          </Tooltip>
        )}
        {shape.said !== undefined && (
          <ChatText
            id={`${id}-said`}
            className="inbox-said"
            text={shape.said}
            foldAt={ASK_FOLD_LINES}
          />
        )}
        {/* What an ask behind Answer needs typed (#5998): a secret's value, a run's. */}
        {shape.needs !== undefined && (
          <p className="inbox-needs ui-text-secondary">{shape.needs}</p>
        )}
        {/* What a run answered on its row lets run, read before its answers (#5998). */}
        {shape.decision && ask.run != null && <RunFacts run={ask.run} />}
        {/* Why its Allow is missing: the lines it runs over, which its line put on one
              (#3502), and a character it holds that reads otherwise (SK-51). */}
        {shape.answers && (shape.lines !== undefined || shape.hidden !== undefined) && (
          <p className="inbox-hidden ui-text-secondary">
            {shape.lines !== undefined
              ? withheldSaid(shape.lines, shape.hidden)
              : hiddenSaid(shape.hidden ?? "")}
          </p>
        )}
        {/* A folder or something on this machine reaches the chat only by its restart (#1986):
              what that ends is said before an Allow here, naming the chat. */}
        {(ask.source === "sandbox-write" || ask.source === "sandbox-local") &&
          shape.answers &&
          shape.offers.some((one) => one.allows) && (
            <p className="inbox-restart ui-text-secondary">
              <RestartEndsLine
                plane={plane}
                session={ask.session}
                chat={ask.chain[ask.chain.length - 1]}
              />
            </p>
          )}
        {ask.answer.via === "task-question" && (
          <>
            <p className="inbox-secondary ui-text-secondary">
              {REACHED_SAID[ask.answer.why](ask.answer.from)}
              {ask.answer.note === null ? "" : ". Its note:"}
            </p>
            {/* The asking chat's note is its own words, drawn as a chat's text is (#5214). */}
            {ask.answer.note !== null && (
              <ChatText className="inbox-note" text={ask.answer.note} foldAt={ASK_FOLD_LINES} />
            )}
          </>
        )}
      </MessageBody>
      {/* **Its links** (#5649): the chat's actions in the one order (ADR 0121 decision 21,
            #6013): Answer, where it is answered here, which opens its form in the row and reads
            Cancel while it is open (#5651, ADR 0119 decision 7); then, on every ask of a chat,
            Go to chat, Snooze and Stop. A decision's answers are its Answer, on its row. */}
      {shape.decision && ask.source !== "dispatch" ? (
        // **A decision's answers, on its row** (#5998): as a chat pane's band draws them, with
        // Go to chat after them. An edit's change is drawn above them, and they wait for it
        // (#1146).
        <>
          {ask.writes != null && (
            <MessageForm>
              <AskWrites writes={ask.writes} onDrawn={onChangeDrawn}>
                {null}
              </AskWrites>
            </MessageForm>
          )}
          <AskDecision
            id={`${id}-form`}
            who={who}
            offers={shape.offers}
            grants={ALLOW_OR_KEEP.has(ask.source)}
            ready={ask.writes == null || ask.writes.kind === "not-shown" || changeDrawn}
            busy={busy}
            reason={shape.reason}
            mayAct={mayAct}
            confirm={ask.risk?.confirm}
            onOpen={onOpen}
            onAnswer={(option, why) => answer(option, undefined, why ?? "")}
          >
            {shape.go && <ChatActions who={who} onGo={onGo} onSnooze={onSnooze} onStop={onStop} />}
          </AskDecision>
          {/* **The rule it offers** (#6010), after the same Allow twice: its wider grants. */}
          <RuleOffer
            ask={ask}
            who={who}
            busy={busy}
            mayAct={mayAct}
            onMake={(option) => answer(option, undefined, "", true)}
          />
        </>
      ) : (
        (shape.form || shape.go) && (
          <MessageLinks>
            <ChatActions
              who={who}
              answer={
                shape.form
                  ? {
                      open,
                      controls: `${id}-form`,
                      onPress: () => {
                        focusForm.current = !open;
                        onOpen(!open);
                      },
                    }
                  : undefined
              }
              onGo={shape.go ? onGo : undefined}
              onSnooze={onSnooze}
              onStop={onStop}
            />
          </MessageLinks>
        )
      )}
      {/* **What answers it, in the row, while its Answer is open** (#5651): its answers, its
            reply box, a task's Answer form; and a dispatch's own Notice, which is a decision and
            always drawn (#5998). */}
      {(shape.decision ? ask.source === "dispatch" : open) && body !== null && (
        <MessageForm id={`${id}-form`}>{body}</MessageForm>
      )}
      {said !== undefined && (
        <p className="inbox-refused" role="status">
          {said}
        </p>
      )}
    </MessageRow>
  );
}

/** **The mark of a high-risk ask** (#6011): in the waiting tone, beside the words of its risk. */
const HIGH_RISK = <Badge variant="waiting">High risk</Badge>;

/** **The most lines an ask's own words show before "Show all · N lines"** (#5649, ADR 0119
 *  decision 7): a long one never pushes the others away. */
export const ASK_FOLD_LINES = 3;

/**
 * **A reply box for a chat waiting on the person's reply** (I-3): what is typed is sent to the
 * chat as the person's message, one paste and Enter (`replyBytes`), through the pane's own
 * input path. Short replies only; anything longer is better written in the chat itself.
 */
function Reply({ plane, ask }: { plane: PlaneId; ask: Shown }) {
  const [text, setText] = useState("");
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const box = useRef<HTMLTextAreaElement>(null);
  const who = chainSaid(ask.chain);
  const send = () => {
    const bytes = replyBytes(text);
    if (bytes === undefined || busy) return;
    // The send is inert while it sends and once the box is empty: the keyboard goes back to the
    // box rather than stay on a button that does nothing (#1700).
    box.current?.focus();
    setBusy(true);
    setSaid(undefined);
    // **Checked before it is sent** (#1700): the reply is a paste and Enter, and if the chat
    // moved on since the list was read (a delivered report started a turn that stopped on a
    // prompt in its terminal), that Enter would land on whatever is in front. So the asks are
    // read again, and nothing is sent unless the chat still waits on a reply.
    void stillWaits(plane, ask)
      .then((waits) => {
        if (waits !== true) {
          asksMoved(plane);
          return { status: "error" as const, error: waits === false ? MOVED_ON : NOT_CHECKED };
        }
        return commands.sendInput(plane, ask.session, bytes);
      })
      .then((done) => {
        if (done.status === "error") {
          setSaid(done.error);
          return;
        }
        noteAnswered(plane, {
          at: Date.now(),
          chain: ask.chain,
          says: ask.says,
          answer: text.trim(),
        });
        setText("");
        asksMoved(plane);
      })
      .catch((err: unknown) => setSaid(`purlis could not send it: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  const empty = busy || replyBytes(text) === undefined;
  return (
    <Form
      className="inbox-reply"
      onSubmit={(event) => {
        event.preventDefault();
        send();
      }}
    >
      {/* **The reply strip** (#5214): always at the card's foot, edge to edge, one line at rest
          and two once it holds the caret, growing to four with its lines. Enter sends,
          Shift+Enter starts a new line, Esc leaves it for its row. → reaches it from its row;
          inside its lines ↑ and ↓ move the caret, and from its first or last they move on to
          the row above or below. */}
      <TextArea
        strip
        inputRef={box}
        aria-label={`Reply to ${who}`}
        value={text}
        placeholder={`Reply to ${who}…`}
        hint={REPLY_HINT}
        spellCheck
        onChange={setText}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            send();
          }
          // Esc leaves it for its row, and its caret keeps its arrows, Home and End: the list
          // family's (`ui/list/rowKeys.ts`).
        }}
        end={
          // Dim and inert while there is nothing to send, the accent's once there is. Never
          // `disabled`: a disabled button drops the keyboard to the page (#1700).
          <IconButton
            icon={<Send />}
            type="submit"
            size="sm"
            variant={empty ? "quiet" : "primary"}
            aria-label={`Send reply to ${who}`}
            aria-disabled={empty || undefined}
          />
        }
      />
      {said !== undefined && (
        <p className="inbox-refused" role="status">
          {said}
        </p>
      )}
    </Form>
  );
}

/** Notes `answer` given to `ask` just now, for the list's recently answered. */
function answeredNow(plane: PlaneId, ask: Shown, answer: string) {
  noteAnswered(plane, { at: Date.now(), chain: ask.chain, says: ask.says, answer });
}

/**
 * **A task's question, answered in its row** (#5651, ADR 0107, ADR 0119 decision 7): the Answer
 * form's rules (`useAnswerQuestion`: it reads the question the task is paused on and answers that
 * one by its number, refuses an empty answer, and keeps what was typed through a refusal), drawn
 * as the reply strip is, since the row already shows the question. Enter sends.
 */
function TaskAnswer({ plane, ask, task }: { plane: PlaneId; ask: Shown; task: string }) {
  const answering = useAnswerQuestion({
    plane,
    session: ask.session,
    task,
    onDone: () => {
      answeredNow(plane, ask, text.trim());
      asksMoved(plane);
    },
  });
  const { asked, text, setText, sending, refused, setRefused, send, box } = answering;
  if (asked === undefined) return <ReadingTheQuestion />;
  if (asked === null) return <NoQuestion task={task} refused={refused} />;
  const empty = sending || text.trim() === "";
  return (
    <Form
      className="inbox-reply"
      onSubmit={(event) => {
        event.preventDefault();
        void send();
      }}
    >
      <TextArea
        strip
        inputRef={box}
        aria-label={`Your answer to ${task}`}
        value={text}
        placeholder={`Answer ${task}…`}
        hint={REPLY_HINT}
        spellCheck
        onChange={(to) => {
          setText(to);
          setRefused(undefined);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            void send();
          }
        }}
        end={
          <IconButton
            icon={<Send />}
            type="submit"
            size="sm"
            variant={empty ? "quiet" : "primary"}
            aria-label={`Send answer to ${task}`}
            aria-disabled={empty || undefined}
          />
        }
      />
      {/* A refusal, as the core says it, with what was typed still in the box: said as the
          reply strip says one. */}
      {refused !== undefined && (
        <p className="inbox-refused" role="status">
          {refused}
        </p>
      )}
    </Form>
  );
}

/** What the reply strip says at its foot while it holds the caret (#5214). */
export const REPLY_HINT = "Enter sends · Shift+Enter new line";

/** What an Allow pressed on an ask drawn or moved just now says: nothing was allowed. */
export const MOVED_JUST_NOW =
  "This ask was drawn or moved just now, so nothing was allowed. Read it and press again.";

/** What a reply refused because its chat moved on says. */
export const MOVED_ON =
  "This chat no longer waits on a reply, so nothing was sent. Go to the chat to see what it does now.";

/** What a reply refused because the check could not be made says. */
export const NOT_CHECKED =
  "purlis could not check that this chat still waits on a reply, so nothing was sent. Try again, or reply in the chat itself.";

/** What an answer to another project's ask says where that ask was answered meanwhile (#1921). */
export const SETTLED_ELSEWHERE =
  "This ask was already answered, in its own project or another window, so nothing was sent.";

/** What an answer to another project's ask says where its project's asks could not be read. */
export const NOT_CHECKED_THERE =
  "purlis could not check that this ask still waits in its project, so nothing was sent. Try again, or go to the project.";

/** Whether `ask` still waits in `plane`, the project's asks read again now: `undefined` where
 *  they could not be read, which sends nothing either. A reply checks it (#1700), and so does
 *  every answer to another project's ask (#1921). */
export async function stillWaits(plane: PlaneId, ask: Shown): Promise<boolean | undefined> {
  const answer = await commands.asksWaiting(plane).catch(() => undefined);
  const asks =
    answer?.status === "ok" ? (answer.data as { asks?: readonly Shown[] } | null)?.asks : undefined;
  if (asks === undefined) return undefined;
  return asks.some((one) => one.session === ask.session && one.ask === ask.ask);
}

/** An empty Inbox: the sentence, and what was answered here lately, read-only (I-12). Where a
 *  chat cannot say it waits, the sentence says that instead. */
export function Empty({
  recent,
  allowed,
  quiet,
  onAllChats,
}: {
  recent: readonly Answered[];
  /** What was allowed on this machine in the last day, from the grants (D-1700-7). */
  allowed: readonly AllowedLately[];
  quiet?: string;
  /** Where each part of "Nothing waits on you · N chats working" leads (ADR 0121 decision 23). */
  onAllChats?: (filter: AllChatsFilter) => void;
}) {
  return (
    <>
      {/* Never "nothing" over a chat that cannot say it waits (charter-app#52): the faint
          hand's sentence, and the chats are the Notices below. */}
      {quiet === undefined ? (
        <NothingWaits onAllChats={onAllChats} />
      ) : (
        <p className="inbox-none">{quiet}</p>
      )}
      {recent.length > 0 && (
        <section className="inbox-recent" aria-label="Recently answered">
          <PanelSectionTitle level={3} className="inbox-chain">
            Recently answered
          </PanelSectionTitle>
          <ul>
            {recent.map((one) => (
              <li key={`${one.at}:${chainSaid(one.chain)}:${one.says}`}>
                <time dateTime={new Date(one.at).toISOString()}>{timeSaid(one.at)}</time>{" "}
                <span className="inbox-recent-chat">{chainSaid(one.chain)}</span>
                {": "}
                <span className="ask-says">{capMarks(one.says).text}</span>
                {" · "}
                <span className="inbox-recent-answer">{one.answer}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {allowed.length > 0 && (
        <section className="inbox-recent" aria-label={ALLOWED_LATELY}>
          <PanelSectionTitle level={3} className="inbox-chain">
            {ALLOWED_LATELY}
          </PanelSectionTitle>
          <ul>
            {allowed.map((one) => (
              <li key={one.key}>
                <time dateTime={new Date(one.at).toISOString()}>{timeSaid(one.at)}</time>{" "}
                {one.chat !== null && (
                  <>
                    <span className="inbox-recent-chat">{one.chat}</span>
                    {": "}
                  </>
                )}
                {/* A host, a folder, a persona: the grant's own words, as data. */}
                <span className="ask-says">{capMarks(one.says).text}</span>
                {" · "}
                <span className="inbox-recent-answer">{one.level}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
}

/** What the scope chips are called. */
export const SCOPE_LABEL = "Show what needs you in";

/** Why Chat is greyed (ADR 0121, amended 2026-11-12, decision 2). */
export const NO_CHAT_FOCUSED = "No chat has focus. Pick one to list its asks here.";

/** What Chat's tooltip says of the focused chat: "steward 8 and its 2 tasks". */
export function tasksSaid(name: string, tasks: number): string {
  if (tasks <= 0) return name;
  return tasks === 1 ? `${name} and its task` : `${name} and its ${tasks} tasks`;
}

/**
 * **A scope with nothing in it, while a wider one has something** (qw359; ADR 0121, amended
 * 2026-11-12, decision 4): it says so, and its way out says how many wait in the nearest wider
 * scope, one press to widen, so a narrow pick never reads as "nothing needs you". It never
 * widens by itself.
 */
export function EmptyScope({
  said,
  wider,
  onWiden,
}: {
  /** What the scope says of itself: that nothing waits in it, or that it is not read yet. */
  said: string;
  wider: { scope: AsksScope; count: number } | undefined;
  onWiden: () => void;
}) {
  if (wider === undefined) return <p className="inbox-none">{said}</p>;
  return (
    <div className="inbox-scope-empty">
      <p className="inbox-none">{said}</p>
      <Button size="sm" onPress={onWiden}>
        {`${wider.count} waiting in ${asksScopeOf(wider.scope)}`}
      </Button>
    </div>
  );
}

/** What the empty Inbox's look back at the grants of the last day is called (D-1700-7). */
export const ALLOWED_LATELY = "Allowed in the last day";

/**
 * **Where a click on an ask's notification lands** (#1694, I-7): chat `session`'s group in the
 * Inbox drawn now, brought on screen, its first ask given the keyboard, from which the keys go
 * on as from any ask. Answers whether the chat has a group to land on: its asks may have been
 * answered elsewhere since.
 */
export function landOnGroup(session: number, source?: Shown["source"]): boolean {
  // Its own group, or the identical requests it joins (#6009), whose row answers it too.
  const group = document.querySelector<HTMLElement>(
    `section.inbox .inbox-chat[data-session="${session}"], section.inbox .inbox-chat[data-sessions~="${session}"]`,
  );
  // At the ask it came for, where it names one (#5215), else the group's first: the row itself,
  // a row of the list family's (#3123).
  const first =
    (source === undefined
      ? null
      : group?.querySelector<HTMLElement>(`[role="row"][data-source="${source}"]`)) ??
    group?.querySelector<HTMLElement>('[role="row"][data-key]');
  if (!group || !first) return false;
  group.scrollIntoView?.({ block: "nearest" });
  first.focus();
  return true;
}

/**
 * **The same landing, through a scope that leaves the chat out** (qw359): a notification is
 * about one ask, and Chat or Workspace may not list its chat. So where the group is not drawn and
 * the scope is narrower than the project's, it widens to Project, as revealing a row a filter
 * hides clears the filter, and the landing is tried again once that is drawn. The person's press
 * on the notification is the widening; nothing widens by itself.
 */
export function landOnGroupIn(plane: PlaneId, session: number, source?: Shown["source"]): void {
  if (landOnGroup(session, source) || keptAsksScope(plane) === "project") return;
  pickAsksScope(plane, "project");
  requestAnimationFrame(() => landOnGroup(session, source));
}
