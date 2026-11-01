// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (spec #4551, #4561). The window around the question surfaces, drawn from the
 * real families and the Inbox's own classes; the terminal, the tabs and the rows' frames are
 * stand-ins. The dark bar at the bottom is the prototype's, not the design.
 *
 * URL: `?s=<state>` the state, `&place=band|foot` where strip and quiz stand, `&bare` no bar.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDownToLine,
  ChevronLeft,
  ChevronRight,
  Hand,
  List,
  MessageCircleQuestion,
  Settings2,
  SquareTerminal,
  X,
} from "lucide-react";
import { KindMark, type RowKind } from "../InboxRow";
import { PersonaMark } from "../PersonaMark";
import { StateShown } from "../StateShown";
import type { Shown } from "../shownState";
import { Count } from "../ui/badge/Badge";
import { Button, IconButton } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { Radio, RadioGroup, Segment, SegmentedControl } from "../ui/choice/Radio";
import { TextArea } from "../ui/field/Field";
import { StatusItem } from "../ui/status/StatusItem";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { Twist } from "../ui/disclosure/Disclosure";
import { COPY, everyWord } from "./copy";
import {
  SCENES,
  askerOf,
  below,
  chainOf,
  chatOf,
  inProgressOf,
  nameOf,
  needsCount,
  needsInTab,
  needsUnder,
  open as openChat,
  personAnswers,
  questionOf,
  quizInFront,
  reachedOf,
  restBehind,
  rowOf,
  setWhere,
  stripInFront,
  type Chat,
  type Question,
  type Wait,
  type World,
} from "./model";
import "./prototype.css";

const SHOWN: Record<"asking" | "needs" | "working", Shown> = {
  asking: { kind: "asking", word: "asking", shape: "question", token: "state.running" },
  needs: { kind: "needs-you", word: "needs you", shape: "hand", token: "state.waiting" },
  working: { kind: "working", word: "working", shape: "ring", token: "state.running" },
};

const ageSaid = (minutes: number) => (minutes < 1 ? "now" : `${minutes}m`);
const timeSaid = (clock: number) => `14:${String(clock % 60).padStart(2, "0")}`;
/** The pause before the next quiz (story 33): long enough that a second click lands on nothing. */
export const PAUSE_MS = 1000;

type Act = (run: (w: World) => void) => void;

/** Why a question reached the person, in one sentence. */
function whySaid(w: World, q: Question): string {
  const asker = askerOf(w, q.task);
  const chat = asker ? nameOf(w, asker) : "";
  if (q.phase === "with-chat") return COPY.quiz.withChat(chat);
  switch (q.why) {
    case "passed-on":
      return COPY.quiz.passedOn(chat);
    case "no-reply":
      return COPY.quiz.noReply(chat);
    case "gone":
      return COPY.quiz.gone(chat);
    case "crashed":
      return COPY.quiz.crashed(chat);
    case "lost":
      return COPY.quiz.lost(chat);
    case "redirect":
      return COPY.quiz.redirect(chat);
    case "asked-by-you":
      return COPY.quiz.askedByYou;
    default:
      return COPY.quiz.nobody;
  }
}

/** The words the person answers with: one card for the quiz and the Inbox. */
function Answers({ w, q, act, afterSend }: { w: World; q: Question; act: Act; afterSend: () => void }) {
  const done = (run?: (w: World) => void) => {
    afterSend();
    act((d) => {
      run?.(d);
      personAnswers(d, questionOf(d, q.id));
    });
  };
  if (q.kind === "cannot-draw") return null;
  if (q.kind === "text")
    return (
      <div className="tq-answer">
        <TextArea
          className="tq-box"
          aria-label={COPY.quiz.answerBox}
          placeholder={COPY.quiz.answerBox}
          rows={2}
          value={w.typed[q.id] ?? ""}
          onChange={(to) => act((d) => void (d.typed[q.id] = to))}
        />
        <Button size="sm" variant="primary" onPress={() => done()}>
          {COPY.quiz.send}
        </Button>
      </div>
    );
  return (
    <div className="tq-options" role="group" aria-label={`Answers to ${nameOf(w, q.task)}`}>
      {q.options?.map((one) => (
        <Button key={one} size="sm" onPress={() => done()}>
          {one}
        </Button>
      ))}
    </div>
  );
}

/** Why it came, and the asking chat's note, marked as its own. */
function Why({ w, q }: { w: World; q: Question }) {
  const asker = askerOf(w, q.task);
  return (
    <>
      <p className="tq-why ui-text-secondary">{whySaid(w, q)}</p>
      {q.note !== undefined && asker !== null && (
        <p className="tq-note">
          <span className="tq-note-who ui-text-meta">
            <PersonaMark persona={chatOf(w, asker).persona ?? "steward"} />
            {COPY.quiz.noteBy(nameOf(w, asker))}
          </span>
          <span className="ui-text-secondary">{q.note}</span>
        </p>
      )}
    </>
  );
}

function Quiz({
  w,
  q,
  act,
  backToTerminal,
}: {
  w: World;
  q: Question;
  act: Act;
  backToTerminal: () => void;
}) {
  const rest = restBehind(w, q);
  const task = nameOf(w, q.task);
  const chain = chainOf(w, q.task);
  const here = w.front === q.task;
  if (q.kind === "cannot-draw")
    return (
      <div className="tq-line" data-quiz={q.id} data-says={q.says} data-testid="quiz-line">
        <KindMark kind="reply" />
        <p>
          {COPY.quiz.cannotDraw(task)} {here ? COPY.quiz.cannotDrawHere : COPY.quiz.cannotDrawWay}{" "}
          <span className="tq-why">{whySaid(w, q)}</span>
        </p>
        {!here && (
          <Button size="sm" variant="primary" onPress={() => act((d) => openChat(d, q.task))}>
            {COPY.quiz.openTask}
          </Button>
        )}
      </div>
    );
  return (
    <Card
      variant="ask"
      className="inbox-ask tq-quiz"
      data-testid="quiz"
      data-quiz={q.id}
      role="group"
      aria-label={COPY.quiz.asks(task)}
      onKeyDown={(event: React.KeyboardEvent) => {
        if (event.key === "Escape") backToTerminal();
      }}
    >
      <p className="inbox-ask-top">
        <span className="inbox-who ui-text-body ui-text-strong">
          {chain.length > 2 ? chain.join(" › ") : COPY.quiz.asks(task)}
        </span>
        <span className="tq-top-end">
          {rest.length > 0 && (
            <Tooltip tip={COPY.quiz.moreSaid(rest.length)}>
              <Count variant="neutral" data-testid="quiz-more" said={COPY.quiz.moreSaid(rest.length)}>
                {COPY.quiz.more(rest.length)}
              </Count>
            </Tooltip>
          )}
          <span className="inbox-where inbox-secondary ui-text-meta">
            {ageSaid(w.clock - (q.reachedAt ?? q.askedAt))}
          </span>
        </span>
      </p>
      <p className="inbox-says">
        <KindMark kind="reply" />
        <span className="ask-says inbox-primary ui-text-body ui-text-strong" data-says={q.says}>
          {q.says}
        </span>
      </p>
      <Why w={w} q={q} />
      <Answers w={w} q={q} act={act} afterSend={backToTerminal} />
      <div className="tq-foot">
        <span className="ui-text-meta">
          {COPY.quiz.toldYours(task)} {COPY.quiz.keyHint}
        </span>
        {!here && (
          <Button size="sm" variant="ghost" onPress={() => act((d) => openChat(d, q.task))}>
            {COPY.quiz.openTask}
          </Button>
        )}
      </div>
    </Card>
  );
}

/** The line a quiz leaves when an agent answered first: who said what, and a way to disagree. */
function Closing({ w, id, act }: { w: World; id: string; act: Act }) {
  const q = questionOf(w, id);
  const [saying, setSaying] = useState(false);
  const [text, setText] = useState("");
  const asker = nameOf(w, askerOf(w, q.task) as string);
  const task = nameOf(w, q.task);
  const typed = w.typed[id];
  const send = () =>
    act((d) => {
      personAnswers(d, questionOf(d, id));
      delete d.typed[id];
    });
  const drop = () =>
    act((d) => {
      d.closing = d.closing.filter((one) => one !== id);
      delete d.typed[id];
    });
  if (q.overridden)
    return (
      <div className="tq-line" data-testid="closing">
        <p>{COPY.closing.overridden(task, asker)}</p>
        <IconButton aria-label={COPY.closing.dismiss} tooltip={COPY.closing.dismiss} size="sm" onPress={drop}
 variant="ghost" icon={<X aria-hidden="true" />} />
      </div>
    );
  if (typed !== undefined)
    return (
      <div className="tq-closing-box" data-testid="closing-typed">
        <p>{COPY.quiz.typedKept(asker, q.chatSaid ?? "")}</p>
        <div className="tq-answer">
          <TextArea
            className="tq-box"
            aria-label={COPY.quiz.answerBox}
            rows={2}
            value={typed}
            onChange={(to) => act((d) => void (d.typed[id] = to))}
          />
          <Button size="sm" onPress={send}>
            {COPY.quiz.sendAnyway}
          </Button>
          <Button size="sm" variant="ghost" onPress={drop}>
            {COPY.closing.dismiss}
          </Button>
        </div>
      </div>
    );
  return (
    <div className={saying ? "tq-closing-box" : "tq-line"} data-testid="closing">
      <p>{COPY.closing.answered(asker, task, q.chatSaid ?? "")}</p>
      {saying ? (
        <>
          <div className="tq-answer">
            <TextArea
              className="tq-box"
              aria-label={COPY.closing.otherwiseBox}
              placeholder={COPY.closing.otherwiseBox}
              rows={2}
              value={text}
              onChange={setText}
            />
            <Button size="sm" onPress={send}>
              {COPY.closing.sendOtherwise}
            </Button>
            <Button size="sm" variant="ghost" onPress={() => setSaying(false)}>
              {COPY.closing.cancel}
            </Button>
          </div>
          <p className="ui-text-meta">{COPY.closing.otherwiseHow(task, asker)}</p>
        </>
      ) : (
        <>
          <Button size="sm" variant="ghost" onPress={() => act((d) => openChat(d, q.task))}>
            {COPY.closing.goToTask}
          </Button>
          <Button size="sm" variant="ghost" onPress={() => setSaying(true)}>
            {COPY.closing.sayOtherwise}
          </Button>
          <IconButton
            aria-label={COPY.closing.dismiss}
            tooltip={COPY.closing.dismiss}
            size="sm"
            onPress={drop}
 variant="ghost" icon={<X aria-hidden="true" />} />
        </>
      )}
    </div>
  );
}

/** The waiting strip. */
function Strip({ w, wait, act }: { w: World; wait: Wait; act: Act }) {
  const earlier = wait.earlier ?? [];
  const at = w.stripAt;
  const says = at === null ? wait.says : earlier[at];
  const open = w.stripOpen;
  const go = (to: number | null) => act((d) => void (d.stripAt = to));
  return (
    <div className="tq-strip" data-testid="strip" data-open={open ? "" : undefined} data-kind={wait.kind}>
      <span className="tq-strip-mark" aria-hidden="true">
        <Hand />
      </span>
      <div className="tq-strip-words">
        <span className="tq-strip-label ui-text-meta">
          {wait.kind === "prose"
            ? at === null
              ? COPY.strip.lastMessage
              : COPY.strip.earlierAt(earlier.length - at, earlier.length)
            : COPY.strip.waitingOn}
          {wait.kind === "permission" ? ` · ${COPY.strip.permissionAside}` : ""}
        </span>
        <p className="tq-strip-says" data-says={at === null ? says : undefined}>
          {says}
        </p>
      </div>
      {wait.kind === "prose" && (
        <span className="tq-strip-ways">
          {at === null ? (
            <Button size="sm" variant="ghost" onPress={() => go(earlier.length - 1)}>
              {COPY.strip.earlier(earlier.length)}
            </Button>
          ) : (
            <>
              <IconButton
                size="sm"
                aria-label={COPY.strip.previous}
                tooltip={COPY.strip.previous}
                isDisabled={at === 0}
                onPress={() => go(at - 1)}
 variant="ghost" icon={<ChevronLeft aria-hidden="true" />} />
              <IconButton
                size="sm"
                aria-label={COPY.strip.next}
                tooltip={COPY.strip.next}
                onPress={() => go(at === earlier.length - 1 ? null : at + 1)}
 variant="ghost" icon={<ChevronRight aria-hidden="true" />} />
              <IconButton
                size="sm"
                aria-label={COPY.strip.backToLast}
                tooltip={COPY.strip.backToLast}
                onPress={() => go(null)}
 variant="ghost" icon={<ArrowDownToLine aria-hidden="true" />} />
            </>
          )}
        </span>
      )}
      {wait.kind !== "permission" && says.length > 90 && (
        <Button
          size="sm"
          variant="ghost"
          aria-expanded={open}
          onPress={() => act((d) => void (d.stripOpen = !d.stripOpen))}
        >
          <Twist open={open} />
          {open ? COPY.strip.showLess : COPY.strip.showAll}
        </Button>
      )}
    </div>
  );
}

/** The permission Notice, as it stands today: its buttons are where they were. */
function PermissionNotice({ wait, act }: { wait: Wait; act: Act }) {
  return (
    <div className="tq-permission" data-scope="notice" data-part="root" data-variant="attention" data-placement="pane">
      <p className="ui-text-body">
        <strong>steward 4 wants to run</strong> <code>{wait.says.replace("Run ", "")}</code>
      </p>
      <div className="tq-options" data-testid="permission-answers">
        <Button size="sm" variant="grants" onPress={() => act((d) => void (d.waits = []))}>
          Allow once
        </Button>
        <Button size="sm" onPress={() => act((d) => void (d.waits = []))}>
          Keep blocked
        </Button>
      </div>
    </div>
  );
}

const TERMINAL: Record<string, string[]> = {
  steward: [
    "⏺ Dispatched developer 7, designer 3 and architect 2.",
    "⏺ Reading the pool limits in the spec and the code.",
    "  Read crates/purlis-core/src/pool/limits.rs (214 lines)",
    "  Read docs/spec.md (2,481 lines)",
    "⏺ Three tasks are at work. I'll report when they have.",
    "",
    "✻ Waited for 3 tasks",
  ],
  talk: ["⏺ Started devops 3 for you.", "", "✻ Idle"],
};
const TASK_TERMINAL = [
  "⏺ Reading the brief.",
  "  Read app/src/settings/PoolLimits.tsx (188 lines)",
  "⏺ dispatch_ask: a question for the chat that asked.",
  "  The task goes on with what it can do while it waits.",
];

function Terminal({ w, inputRef, act }: { w: World; inputRef: React.Ref<HTMLTextAreaElement>; act: Act }) {
  const lines = [...(TERMINAL[w.front] ?? TASK_TERMINAL), ...(w.printed[w.front] ?? [])];
  const [text, setText] = useState("");
  return (
    <div className="tq-terminal" data-testid="terminal">
      <div className="tq-terminal-lines">
        {lines.map((line, at) => (
          <div key={at}>{line === "" ? " " : line}</div>
        ))}
      </div>
      <label className="tq-prompt">
        <span aria-hidden="true">❯</span>
        <textarea
          ref={inputRef}
          data-testid="terminal-input"
          aria-label="Type to this chat"
          rows={1}
          value={text}
          spellCheck={false}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== "Enter") return;
            event.preventDefault();
            setText("");
            act((d) => {
              d.waits = d.waits.filter((one) => one.chat !== d.front);
            });
          }}
        />
      </label>
    </div>
  );
}

/** A chat's row: state mark, name, and who it waits on and how long. */
function Row({ w, chat, level, act }: { w: World; chat: Chat; level: number; act: Act }) {
  const state = rowOf(w, chat.id);
  const total = needsUnder(w, chat.id);
  const under = w.chats.some((c) => c.parent === chat.id);
  const foldedAsking = chat.folded
    ? below(w, chat.id).filter((c) => rowOf(w, c.id)?.kind === "asking").length
    : 0;
  const shown =
    state?.kind === "needs-you" ? SHOWN.needs : state?.kind === "asking" ? SHOWN.asking : SHOWN.working;
  const worded =
    state?.kind === "needs-you"
      ? COPY.row.needsYou
      : state?.kind === "asking"
        ? COPY.row.asking(state.who)
        : state?.kind === "own-pane"
          ? COPY.row.ownPane
          : null;
  return (
    <li role="none" data-level={level}>
      <button
        type="button"
        className="chat ui-row ui-tree-row"
        role="treeitem"
        aria-level={level}
        aria-current={w.front === chat.id || undefined}
        aria-expanded={under ? !chat.folded : undefined}
        data-row={chat.id}
        data-row-state={state?.kind ?? "none"}
        onClick={() => act((d) => openChat(d, chat.id))}
      >
        {under && (
          <span
            className="ui-tree-twist"
            aria-hidden="true"
            onClick={(event) => {
              event.stopPropagation();
              act((d) => void (chatOf(d, chat.id).folded = !chatOf(d, chat.id).folded));
            }}
          >
            <ChevronRight aria-hidden="true" />
          </span>
        )}
        <span className="ui-row-label session ui-text-body">{chat.name}</span>{" "}
        <span className="ui-row-lead">
          <span className="chat-persona" data-state={shown.kind}>
            {chat.persona ? (
              <PersonaMark persona={chat.persona} />
            ) : (
              <SquareTerminal className="node-icon" aria-hidden="true" />
            )}{" "}
            <StateShown shown={shown} markOnly />
          </span>
        </span>
        <span className="ui-row-trail tq-row-trail">
          {worded !== null && (
            <Tooltip tip={state?.kind === "own-pane" ? COPY.row.ownPaneTip(chat.harness) : worded}>
              <span className="tq-row-word ui-text-meta" data-testid={`word-${chat.id}`}>
                {worded}
                {state && state.kind !== "own-pane" && (
                  <>
                    <span aria-hidden="true"> · </span>
                    <time>{ageSaid(w.clock - state.since)}</time>
                  </>
                )}
              </span>
            </Tooltip>
          )}
          {foldedAsking > 0 && (
            <span className="tq-row-word ui-text-meta">{COPY.row.foldedAsking(foldedAsking)}</span>
          )}
          {total > 0 && (
            <Tooltip tip={COPY.row.total(total)}>
              <span data-testid={`total-${chat.id}`}>
                <Count variant="waiting" said={COPY.row.total(total)}>
                  {total}
                </Count>
              </span>
            </Tooltip>
          )}
        </span>
      </button>
    </li>
  );
}

function ChatList({ w, act }: { w: World; act: Act }) {
  const walk = (parent: string | null, level: number): React.ReactNode[] =>
    w.chats
      .filter((c) => c.parent === parent)
      .flatMap((c) => [
        <Row key={c.id} w={w} chat={c} level={level} act={act} />,
        ...(c.folded ? [] : walk(c.id, level + 1)),
      ]);
  return (
    <aside className="tq-side" aria-label="Navigation">
      <div className="chats-section ui-text-body">
        <h2 className="tq-side-head ui-text-secondary">Chats</h2>
        <ul className="ui-tree" role="tree" aria-label="Chats">
          {walk(null, 1)}
        </ul>
      </div>
    </aside>
  );
}

function TabBar({ w, act }: { w: World; act: Act }) {
  return (
    <div className="tq-tabs" role="tablist" aria-label="Tabs">
      {w.tabs.map((tab) => {
        const settings = tab === "settings";
        const count = settings ? 0 : needsInTab(w, tab);
        const inFront = w.front === tab;
        return (
          <button
            key={tab}
            type="button"
            role="tab"
            className="tq-tab"
            aria-selected={inFront}
            data-tab={tab}
            onClick={() => act((d) => void (d.front = tab))}
          >
            {settings ? <Settings2 aria-hidden="true" /> : null}
            {settings ? "Settings" : nameOf(w, tab)}
            {count > 0 && (
              <Count variant="hot" data-testid={`tab-count-${tab}`}>
                {count}
              </Count>
            )}
          </button>
        );
      })}
    </div>
  );
}

function SettingsView({ w, act }: { w: World; act: Act }) {
  return (
    <div className="tq-settings">
      <h2 className="ui-text-body ui-text-strong">{COPY.settings.group}</h2>
      <div className="tq-setting">
        <p className="ui-text-body ui-text-strong" id="tq-where">
          {COPY.settings.label}
        </p>
        <p className="ui-text-secondary">{COPY.settings.help}</p>
        <RadioGroup
          aria-labelledby="tq-where"
          value={w.where}
          onChange={(to) => act((d) => setWhere(d, to as World["where"]))}
        >
          <Radio value="top" description={COPY.settings.topHelp}>
            {COPY.settings.top}
          </Radio>
          <Radio value="each" description={COPY.settings.eachHelp}>
            {COPY.settings.each}
          </Radio>
        </RadioGroup>
      </div>
    </div>
  );
}

function InboxPanel({ w, act }: { w: World; act: Act }) {
  const kindOf: Record<Wait["kind"], RowKind> = {
    reply: "reply",
    prose: "reply",
    permission: "decision",
    browser: "elsewhere",
  };
  const sourceOf: Record<Wait["kind"], string> = {
    reply: COPY.inbox.reply,
    prose: COPY.inbox.reply,
    permission: COPY.inbox.permission,
    browser: COPY.inbox.browser,
  };
  const reached = reachedOf(w);
  const empty = reached.length === 0 && w.waits.length === 0;
  return (
    <section className="inbox tq-inbox" data-view="inbox" aria-label="Inbox">
      <div className="inbox-list">
        {empty && <p className="tq-empty ui-text-secondary">{COPY.inbox.nothing}</p>}
        {reached.map((q) => (
          <section key={q.id} className="inbox-chat" data-question={q.id}>
            <h3 className="inbox-chain">{chainOf(w, q.task).join(" › ")}</h3>
            <Card as="li" variant="ask" className="inbox-ask" data-testid="inbox-card">
              <p className="inbox-says">
                <KindMark kind="reply" />
                <span className="ask-says inbox-primary ui-text-body ui-text-strong" data-says={q.says}>
                  {q.says}
                </span>
                <span className="inbox-meta ui-text-meta">
                  <span className="inbox-source">{COPY.inbox.question}</span>
                  <span className="inbox-meta-dot"> · </span>
                  <time className="inbox-age">{ageSaid(w.clock - (q.reachedAt ?? q.askedAt))}</time>
                </span>
              </p>
              <Why w={w} q={q} />
              <Answers w={w} q={q} act={act} afterSend={() => {}} />
              <div className="tq-foot">
                <Button size="sm" variant="ghost" onPress={() => act((d) => openChat(d, q.task))}>
                  {COPY.inbox.goToChat}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onPress={() =>
                    act((d) => {
                      d.panel = "activity";
                      d.activityOf = q.task;
                    })
                  }
                >
                  {COPY.trail.open}
                </Button>
              </div>
            </Card>
          </section>
        ))}
        {w.waits.map((wait) => (
          <section key={wait.id} className="inbox-chat">
            <h3 className="inbox-chain">{chainOf(w, wait.chat).join(" › ")}</h3>
            <Card as="li" variant="ask" className="inbox-ask" data-testid="inbox-card">
              <p className="inbox-says">
                <KindMark kind={kindOf[wait.kind]} />
                <span className="ask-says inbox-primary ui-text-body ui-text-strong" data-says={wait.says}>
                  {wait.says}
                </span>
                <span className="inbox-meta ui-text-meta">
                  <span className="inbox-source">{sourceOf[wait.kind]}</span>
                  <span className="inbox-meta-dot"> · </span>
                  <time className="inbox-age">{ageSaid(w.clock - wait.since)}</time>
                </span>
              </p>
              <div className="tq-foot">
                <Button size="sm" variant="primary" onPress={() => act((d) => openChat(d, wait.chat))}>
                  {COPY.inbox.goToChat}
                </Button>
              </div>
            </Card>
          </section>
        ))}
        {w.updates.length > 0 && (
          <section className="inbox-chat" aria-label={COPY.inbox.updates}>
            <h3 className="inbox-chain">{COPY.inbox.updates}</h3>
            {w.updates.map((one) => (
              <div key={one.id} className="tq-update" data-testid="update">
                <KindMark kind={one.trouble ? "trouble" : "update"} />
                <p className="ui-text-body">{one.says}</p>
                {one.question !== undefined && (
                  <Button
                    size="sm"
                    variant="ghost"
                    onPress={() =>
                      act((d) => {
                        d.panel = "activity";
                        d.activityOf = questionOf(d, one.question as string).task;
                      })
                    }
                  >
                    {COPY.trail.open}
                  </Button>
                )}
              </div>
            ))}
          </section>
        )}
      </div>
    </section>
  );
}

function ActivityPanel({ w }: { w: World }) {
  const task = w.activityOf ?? w.front;
  const mine = w.chats.some((c) => c.id === task)
    ? w.questions.filter((q) => q.task === task)
    : [];
  return (
    <section className="tq-activity" aria-label="Activity">
      <p className="ui-text-body ui-text-strong">
        {w.chats.some((c) => c.id === task) ? COPY.trail.activity(nameOf(w, task)) : ""}
      </p>
      {mine.length === 0 && <p className="ui-text-secondary">{COPY.trail.none}</p>}
      {mine.map((q) => (
        <div key={q.id} className="tq-trail" data-testid="trail">
          <p className="ui-text-secondary">{COPY.trail.title}</p>
          <ol>
            {q.trail.map((step, n) => (
              <li key={n}>
                <time className="ui-text-meta">{timeSaid(step.at)}</time>
                <span>{step.said}</span>
              </li>
            ))}
          </ol>
          <p className="ui-text-meta">{COPY.trail.words}</p>
        </div>
      ))}
    </section>
  );
}

function FirstLine({ act }: { act: Act }) {
  return (
    <div className="tq-first" data-testid="first-line">
      <p>{COPY.first.line}</p>
      <Button
        size="sm"
        variant="ghost"
        onPress={() =>
          act((d) => {
            setWhere(d, "each");
            d.firstLine = false;
            d.movedLine = true;
          })
        }
      >
        {COPY.first.each}
      </Button>
      <Button size="sm" variant="ghost" onPress={() => act((d) => void (d.firstLine = false))}>
        {COPY.first.keep}
      </Button>
    </div>
  );
}

function MovedLine({ act }: { act: Act }) {
  return (
    <div className="tq-first" data-testid="moved-line">
      <p>{COPY.first.moved}</p>
      <IconButton
        size="sm"
        aria-label={COPY.first.close}
        tooltip={COPY.first.close}
        onPress={() => act((d) => void (d.movedLine = false))}
 variant="ghost" icon={<X aria-hidden="true" />} />
    </div>
  );
}

function Pause({ w }: { w: World }) {
  return (
    <div className="inbox-ask tq-pause" data-testid="pause">
      <p className="ui-text-body">{COPY.quiz.sent(nameOf(w, w.pause?.task as string))}</p>
      <p className="ui-text-secondary">{COPY.quiz.nextSoon}</p>
    </div>
  );
}

/** The pane: band, head, terminal; strip and quiz stand in the band or at the foot. */
function Pane({
  w,
  act,
  place,
  inputRef,
  afterAnswer,
}: {
  w: World;
  act: Act;
  place: "band" | "foot";
  inputRef: React.RefObject<HTMLTextAreaElement | null>;
  afterAnswer: () => void;
}) {
  const quiz = quizInFront(w);
  const strip = stripInFront(w);
  const asks = (
    <div className="tq-asks" data-testid="asks" data-place={place}>
      {w.movedLine && w.front === "steward" && <MovedLine act={act} />}
      {strip && <Strip w={w} wait={strip} act={act} />}
      {strip?.kind === "permission" && <PermissionNotice wait={strip} act={act} />}
      {w.closing.map((id) => (
        <Closing key={id} w={w} id={id} act={act} />
      ))}
      {w.pause && <Pause w={w} />}
      {quiz && w.firstLine && <FirstLine act={act} />}
      {quiz && <Quiz w={w} q={quiz} act={act} backToTerminal={afterAnswer} />}
    </div>
  );
  const drawn =
    w.movedLine || strip !== null || w.closing.length > 0 || w.pause !== null || quiz !== null;
  return (
    <div className="tq-pane" data-testid="pane">
      {place === "band" && drawn && asks}
      <div className="tq-pane-head">
        <span className="ui-text-meta">{chatOf(w, w.front).harness} · smart-ide</span>
      </div>
      <Terminal w={w} inputRef={inputRef} act={act} />
      {place === "foot" && drawn && asks}
    </div>
  );
}

function Words() {
  return (
    <div className="tq-words">
      {everyWord().map((one) => (
        <section key={one.surface}>
          <h2 className="ui-text-body ui-text-strong">{one.surface}</h2>
          <ul>
            {one.lines.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

function params() {
  return new URLSearchParams(window.location.search);
}

export type Look = { appearance: "light" | "dark"; theme: string };

export function PrototypeQuestions({
  look,
  onLook,
}: {
  look: Look;
  onLook: (look: Look) => void;
}) {
  const [at, setAt] = useState(() => {
    const found = SCENES.findIndex((one) => one.id === params().get("s"));
    return found < 0 ? 0 : found;
  });
  const [place, setPlace] = useState<"band" | "foot">(() =>
    params().get("place") === "foot" ? "foot" : "band",
  );
  const bare = params().has("bare");
  const scene = SCENES[at];
  const [w, setW] = useState<World>(() => scene.make());
  const [listing, setListing] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const go = useCallback((to: number) => {
    const n = (to + SCENES.length) % SCENES.length;
    setAt(n);
    setW(SCENES[n].make());
    const url = new URL(window.location.href);
    url.searchParams.set("s", SCENES[n].id);
    window.history.replaceState(null, "", url);
  }, []);

  const act = useCallback<Act>((run) => {
    setW((was) => {
      const next = structuredClone(was);
      run(next);
      return next;
    });
  }, []);

  // The pause ends by itself, unless the state is the pause, held still to be looked at.
  useEffect(() => {
    if (!w.pause || scene.id === "quiz-pause") return;
    const done = setTimeout(() => act((d) => void (d.pause = null)), PAUSE_MS);
    return () => clearTimeout(done);
  }, [w.pause, act, scene.id]);

  // For capture.mjs: plays a state's Pretend with nothing pressed, so focus stays where it was.
  useEffect(() => {
    (window as unknown as { tq: unknown }).tq = {
      pretend: (at_: number) => act((scene.pretend ?? [])[at_].run),
    };
  }, [act, scene]);

  const backToTerminal = useCallback(() => {
    // Only where the keyboard was in the quiz: a quiz leaving must not leave it on the page.
    const inside = document.activeElement?.closest("[data-quiz]");
    if (inside) inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      if (event.key === "F6") {
        event.preventDefault();
        const inQuiz = document.activeElement?.closest("[data-quiz]");
        if (inQuiz) inputRef.current?.focus();
        else
          document
            .querySelector<HTMLElement>(
              "[data-quiz] button:not([disabled]), [data-quiz] textarea",
            )
            ?.focus();
        return;
      }
      const typing = (event.target as HTMLElement).closest("input, textarea");
      if (typing) return;
      if (event.key === "]") go(at + 1);
      if (event.key === "[") go(at - 1);
      if (event.key === "\\") setListing((was) => !was);
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  }, [at, go]);

  const count = needsCount(w);
  const groups = useMemo(() => [...new Set(SCENES.map((one) => one.group))], []);

  return (
    <div
      className="tq-root"
      data-bare={bare ? "" : undefined}
      data-needs={count}
      data-in-progress={inProgressOf(w).length}
      data-where={w.where}
      data-scene={scene.id}
    >
      <header className="tq-title">
        <span className="ui-text-body ui-text-strong">charter · smart-ide</span>
        <span className="tq-title-end">
          {count > 0 && (
            <StatusItem
              variant="waiting"
              data-testid="needs-you-button"
              aria-label={COPY.needs.count(count)}
              tooltip={COPY.needs.count(count)}
              onPress={() =>
                act((d) => void (d.panel = d.panel === "inbox" ? null : "inbox"))
              }
              icon={<Hand aria-hidden="true" />}
              count={count}
            />
          )}
        </span>
      </header>
      {scene.page === "words" ? (
        <Words />
      ) : (
        <div className="tq-body">
          <ChatList w={w} act={act} />
          <main className="tq-main">
            <TabBar w={w} act={act} />
            {w.front === "settings" ? (
              <SettingsView w={w} act={act} />
            ) : (
              <Pane
                w={w}
                act={act}
                place={place}
                inputRef={inputRef}
                afterAnswer={backToTerminal}
              />
            )}
          </main>
          {w.panel !== null && (
            <aside className="tq-panel" aria-label={w.panel === "inbox" ? "Inbox" : "Activity"}>
              <div className="tq-panel-head">
                <span className="ui-text-body ui-text-strong">
                  {w.panel === "inbox" ? COPY.inbox.title : "Activity"}
                </span>
                <IconButton
                  size="sm"
                  aria-label="Close"
                  tooltip="Close"
                  onPress={() => act((d) => void (d.panel = null))}
 variant="ghost" icon={<X aria-hidden="true" />} />
              </div>
              {w.panel === "inbox" ? <InboxPanel w={w} act={act} /> : <ActivityPanel w={w} />}
            </aside>
          )}
        </div>
      )}

      {!bare && (
        <div className="tq-bar" data-theme-keep="">
          <IconButton size="sm" aria-label="Previous state" onPress={() => go(at - 1)}
 variant="ghost" icon={<ChevronLeft aria-hidden="true" />} />
          <button type="button" className="tq-bar-name" onClick={() => setListing((was) => !was)}>
            <List aria-hidden="true" />
            <span>
              {at + 1}/{SCENES.length} · {scene.group}: {scene.name}
              <small> · stories {scene.stories}</small>
            </span>
          </button>
          <IconButton size="sm" aria-label="Next state" onPress={() => go(at + 1)}
 variant="ghost" icon={<ChevronRight aria-hidden="true" />} />
          {(scene.pretend ?? []).map((one) => (
            <Button key={one.label} size="sm" onPress={() => act(one.run)}>
              <MessageCircleQuestion aria-hidden="true" /> {one.label}
            </Button>
          ))}
          <span className="tq-bar-end">
            <SegmentedControl
              aria-label="Where strip and quiz stand"
              value={place}
              onChange={(to) => setPlace(to as "band" | "foot")}
            >
              <Segment value="band">Band</Segment>
              <Segment value="foot">Foot</Segment>
            </SegmentedControl>
            <SegmentedControl
              aria-label="Appearance"
              value={look.appearance}
              onChange={(to) => onLook({ ...look, appearance: to as Look["appearance"] })}
            >
              <Segment value="light">Light</Segment>
              <Segment value="dark">Dark</Segment>
            </SegmentedControl>
          </span>
        </div>
      )}
      {listing && !bare && (
        <div className="tq-list" role="dialog" aria-label="Every state">
          <div className="tq-list-head">
            <strong>Every state</strong>
            <span>[ and ] move, \ closes this. Band or Foot on the bar moves strip and quiz.</span>
            <IconButton size="sm" aria-label="Close" onPress={() => setListing(false)}
 variant="ghost" icon={<X aria-hidden="true" />} />
          </div>
          <div className="tq-list-groups">
            {groups.map((group) => (
              <section key={group}>
                <h3>{group}</h3>
                {SCENES.map((one, n) =>
                  one.group === group ? (
                    <button
                      key={one.id}
                      type="button"
                      data-here={n === at ? "" : undefined}
                      onClick={() => {
                        go(n);
                        setListing(false);
                      }}
                    >
                      {one.name}
                    </button>
                  ) : null,
                )}
              </section>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
