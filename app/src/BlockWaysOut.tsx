// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState, type ReactNode } from "react";
import { Button, ButtonRow } from "./ui/button/Button";
import { NothingDone, useSettledPlace } from "./inboxRowParts";
import { blockSaid, harnessSaid, type BlockRow } from "./chatLog";
import { commands, type FinishedTurn, type GrantLevel, type PlaneId } from "./bindings";
import { finishWaysOf, finishWaysTo } from "./finishRows";
import { SANDBOX_ALLOW_SAID } from "./allowLabels";
import { sandboxCommandReturned } from "./sandboxAsked";
import {
  EVERY_CHATS_LEVELS,
  FOR_EVERY_CHAT,
  GUARD_SWITCH_LATER,
  ONE_CHATS_LEVELS,
  ROW_MOVED_JUST_NOW,
  RULE_WHERE,
  TOLD,
  TYPE_IT,
  WAY_SAID,
  allowsNextTime,
  checkSaid,
  guardGroupSaid,
  nextTimeGroupedSaid,
  nextTimeLevels,
  nextTimeSaid,
  onlyItsCheckDecides,
  ruleSaid,
  tellAsksOf,
  toldIn,
  waysOut,
  type BlockOfWays,
  type NextTimeSaid,
  type Told,
  type Way,
} from "./blockWays";

export type BlockWaysOutProps = {
  /** The block, or a finish row's turn's unresolved blocks. */
  blocks: readonly BlockOfWays[];
  /** The chat, by its name: what each way's name says it is about. */
  who: string;
  /** **Tell the chat…'s second press**: types purlis's own line into the chat where it waits
   *  (`tell_chat_blocked`), and answers what was typed, and why not where nothing was. */
  tell: () => Promise<Told>;
  /** Go to chat: the chat in front. */
  go: () => void;
  /** The chats Tell the chat… types into, by name, where it is more than one (Q272): what it
   *  asks first names each. */
  into?: readonly string[];
  /** **Allow next time's label, pressed** (T1-34, R-291): a grant of the sandbox block's
   *  `target` at `level`, with no ask open, which the core judges again (`allow_next_time`):
   *  for chat `session` alone where a row of several chats names it (Q278), and once for them
   *  all where it names none. Where none is given, no Allow next time is offered. */
  allowNext?: (target: string, level: GrantLevel, session?: number) => Promise<NextTimeSaid>;
  /** **The chats a finish row's Allow next time acts on, each by name** (Q278, Q273): each
   *  target's chats whose own block it is. Given, each chat is named before any press, and its
   *  own labels grant it alone. */
  nextChats?: readonly { session: number; target: string; name: string }[];
};

/** What a detail opened by a way is named, and so found by a screen reader's landmarks. */
const OPENS: Partial<Record<Way, string>> = {
  "allow-next": "Next time",
  tell: "Tell the chat",
  "show-guard": "This guard",
  "show-check": "This check",
  "show-rule": "This rule",
};

/** A block as a sentence names it, for a finish row's blocks that carry no tool. */
const said = (block: BlockOfWays) =>
  blockSaid({ tool: "", qualifier: null, harness: "", ...block } as BlockRow);

/**
 * **A block's ways out** (T1-33; R-189, R-191 to R-194, R-199, R-300), on its Chat log row and
 * its turn's finish row, never on its marker: the ways `blocks` offer, each once, as quiet
 * buttons in a named group. **Nothing here allows anything** (R-191, R-194): no Allow, no
 * command to copy or run by hand.
 *
 * - **Show this guard** and **Show this check** open a read-only detail under the row (phase
 *   1): the guard's group, only here (R-184); the harness's own check, and whose rules it reads.
 * - **Tell the chat…** asks first (docs/ui-copy.md): it says what its press types, and only its
 *   second press, **Type it in the chat**, types it, through the core's one way in (Q267).
 * - **Go to chat**, for a block purlis can't tell who made (R-199).
 *
 * - **Allow next time**, for a sandbox block an Allow would unblock (T1-34, R-291): it opens the
 *   shipped Allow labels, and only a label's press grants, once the row has settled (ADR 0097,
 *   `mayAct`): a press within the settle time of the labels being drawn or moved does nothing,
 *   and says why. **Show this rule** opens the rule that refused it, read-only (R-196).
 *
 * A block by the harness's own check says, beside its ways, that purlis can't allow it (R-193).
 */
export function BlockWaysOut({
  blocks,
  who,
  tell,
  go,
  into = [],
  allowNext,
  nextChats,
}: BlockWaysOutProps) {
  const id = useId();
  const ways = waysOut(blocks).filter((way) => way !== "allow-next" || allowNext !== undefined);
  const [open, setOpen] = useState<Way>();
  const [busy, setBusy] = useState(false);
  const [told, setTold] = useState<Told>();
  const [allowed, setAllowed] = useState<NextTimeSaid>();
  const sandboxes = blocks.filter((one) => one.source === "sandbox");
  // Each target once: a finish row's turns may hold the same block more than once.
  const allowable = [
    ...new Set(sandboxes.filter(allowsNextTime).flatMap((one) => one.target ?? [])),
  ];
  // **The settle guard on Allow next time's labels** (ADR 0097): measured from when what the
  // row offers, which of its details is open, or the targets and chats its labels stand for
  // (Q278) last changed, and from each label's move.
  const { drawnAt, tooSoon } = useSettledPlace<HTMLDivElement>(
    [
      ways.join(" "),
      open ?? "",
      allowable.join(" "),
      (nextChats ?? []).map((one) => `${one.session}:${one.target}`).join(" "),
    ].join("\n"),
  );
  if (ways.length === 0) return null;
  const checks = blocks.filter((one) => one.source === "harness_check");
  const guards = blocks.filter((one) => one.source === "guard");
  const press = (way: Way) => {
    if (way === "go") {
      go();
      return;
    }
    setTold(undefined);
    setAllowed(undefined);
    setOpen((was) => (was === way ? undefined : way));
  };
  /** **A label of Allow next time, pressed**: nothing on a row drawn or moved just now. */
  const allowIt = (target: string, level: GrantLevel, session?: number) => {
    if (busy || allowNext === undefined) return;
    if (tooSoon()) {
      setAllowed({ why: ROW_MOVED_JUST_NOW });
      return;
    }
    setBusy(true);
    setAllowed(undefined);
    // One chat's row names no chat: its grant is that chat's.
    void (session === undefined ? allowNext(target, level) : allowNext(target, level, session))
      .then(setAllowed)
      .catch((err: unknown) =>
        setAllowed({ why: `purlis could not allow ${target}: ${String(err)}` }),
      )
      .finally(() => setBusy(false));
  };
  const typeIt = () => {
    if (busy) return;
    setBusy(true);
    setTold(undefined);
    void tell()
      .then(setTold)
      .catch((err: unknown) =>
        setTold({ why: `purlis could not type in the chat: ${String(err)}` }),
      )
      .finally(() => setBusy(false));
  };
  const detail = `${id}-detail`;
  return (
    <div className="block-ways" ref={drawnAt}>
      {checks.length > 0 && (
        <p className="block-ways-decides ui-text-secondary">
          {onlyItsCheckDecides(checks[0].harness)}
        </p>
      )}
      <ButtonRow size="sm" role="group" aria-label={`Ways out for ${who}`}>
        {ways.map((way) => (
          <Button
            key={way}
            variant="quiet-link"
            aria-expanded={OPENS[way] === undefined ? undefined : open === way}
            aria-controls={open === way ? detail : undefined}
            onPress={() => press(way)}
          >
            {WAY_SAID[way]}
          </Button>
        ))}
      </ButtonRow>
      {open === "show-guard" && (
        <section id={detail} className="block-ways-detail" aria-label={OPENS[open]}>
          <ul>
            {guards.map((block, at) => (
              <li key={`${block.target ?? ""}:${at}`} className="ui-text-body">
                {guardGroupSaid(block)}: {said(block)}.
              </li>
            ))}
          </ul>
          <p className="ui-text-secondary">{GUARD_SWITCH_LATER}</p>
        </section>
      )}
      {open === "show-check" && (
        <section id={detail} className="block-ways-detail" aria-label={OPENS[open]}>
          <p className="ui-text-body">
            Blocked by {harnessSaid(checks[0]?.harness)}&apos;s own permission check.
          </p>
          <p className="ui-text-secondary">{checkSaid(checks[0]?.harness)}</p>
        </section>
      )}
      {open === "allow-next" && (
        <section id={detail} className="block-ways-detail" aria-label={OPENS[open]}>
          {allowable.map((target) => {
            const levels = nextTimeLevels(sandboxes, target);
            const label = (level: GrantLevel, session?: number) => (
              <Button
                key={level}
                variant="grants"
                isDisabled={busy}
                onPress={() => allowIt(target, level, session)}
              >
                {SANDBOX_ALLOW_SAID[level]}
              </Button>
            );
            if (nextChats === undefined)
              return (
                <div key={target}>
                  <p className="ui-text-body">{nextTimeSaid(target)}</p>
                  <ButtonRow size="sm">{levels.map((level) => label(level))}</ButtonRow>
                </div>
              );
            // A row of several chats (Q278, Q273): each named before any press, its own labels
            // granting it alone; the labels for you and the project once, for them all.
            const chats = nextChats.filter((one) => one.target === target);
            const each = levels.filter((level) => ONE_CHATS_LEVELS.includes(level));
            const every = levels.filter((level) => EVERY_CHATS_LEVELS.includes(level));
            return (
              <div key={target}>
                <p className="ui-text-body">
                  {nextTimeGroupedSaid(
                    target,
                    chats.map((one) => one.name),
                  )}
                </p>
                {each.length > 0 &&
                  chats.map((one) => (
                    <div key={one.session}>
                      <p className="ui-text-secondary">{one.name}</p>
                      <ButtonRow size="sm" role="group" aria-label={one.name}>
                        {each.map((level) => label(level, one.session))}
                      </ButtonRow>
                    </div>
                  ))}
                {every.length > 0 && (
                  <>
                    <p className="ui-text-secondary">{FOR_EVERY_CHAT}</p>
                    <ButtonRow size="sm" role="group" aria-label={FOR_EVERY_CHAT}>
                      {every.map((level) => label(level))}
                    </ButtonRow>
                  </>
                )}
              </div>
            );
          })}
          {allowed?.said !== undefined && (
            <p className="ui-text-secondary" role="status">
              {allowed.said}
            </p>
          )}
          <NothingDone said={allowed?.why} />
        </section>
      )}
      {open === "show-rule" && (
        <section id={detail} className="block-ways-detail" aria-label={OPENS[open]}>
          <ul>
            {sandboxes.map((block, at) => (
              <li key={`${block.target ?? ""}:${at}`} className="ui-text-body">
                {ruleSaid(block)}
              </li>
            ))}
          </ul>
          <p className="ui-text-secondary">{RULE_WHERE}</p>
        </section>
      )}
      {open === "tell" && (
        <section id={detail} className="block-ways-detail" aria-label={OPENS[open]}>
          <p className="ui-text-body">{tellAsksOf(into)}</p>
          <ButtonRow size="sm">
            <Button variant="secondary" isDisabled={busy} onPress={typeIt}>
              {TYPE_IT}
            </Button>
          </ButtonRow>
          {/* What was typed is a plain status line; NothingDone is for a press that did
              nothing (#1693), and says only why. */}
          {told?.typed !== undefined && (
            <p className="ui-text-secondary" role="status">
              {told.typed}
            </p>
          )}
          <NothingDone said={told?.why} />
        </section>
      )}
    </div>
  );
}

/**
 * **Tells each of `sessions`** (Q272) through the core's one way in (`tell_chat_blocked`), and
 * says what came of it: where it typed into more than one chat, each by name.
 */
async function tellEach(
  plane: PlaneId,
  sessions: readonly number[],
  nameOf: (session: number) => string,
): Promise<Told> {
  const said = await Promise.all(
    sessions.map((one) =>
      commands
        .tellChatBlocked(plane, one)
        .then((done) => ({ one, why: done.status === "error" ? done.error : undefined })),
    ),
  );
  const typed = said.filter((one) => one.why === undefined).map((one) => nameOf(one.one));
  const why = said.flatMap((one) => (one.why === undefined ? [] : [one.why]));
  return {
    typed: typed.length === 0 ? undefined : sessions.length > 1 ? toldIn(typed) : TOLD,
    why: why.length === 0 ? undefined : why.join(" "),
  };
}

/**
 * **Allows `target` at `level` for chat `session`** (T1-34, Q278) through the core's
 * `allow_next_time`, and says what came of it, naming the chat where the row names several.
 */
async function allowFor(
  plane: PlaneId,
  session: number,
  [target, level]: [string, GrantLevel],
  named: string | undefined,
): Promise<NextTimeSaid> {
  const said = (line: string) => (named === undefined ? line : `${named}: ${line}`);
  const done = await commands.allowNextTime(plane, session, target, level);
  return done.status === "ok" ? { said: said(done.data.said) } : { why: said(done.error) };
}

/**
 * **A finish row's ways out** (T1-33, R-300; Q272): its turns' unresolved blocks', as their
 * Chat log rows offer them, or none where it has none. **Each acts on the chat it is about**:
 * Tell the chat… types only into the chats whose own turn holds a block that offers it, and
 * names them where there is more than one; Go to chat goes to the chat that holds the
 * Can't-tell block; Allow next time allows only for the chats whose own sandbox block it is.
 *
 * A component, not a helper the view calls while it draws: what `go` presses reads refs, and
 * a function handed one during render is what react-hooks/refs refuses. Draw it only where
 * {@link finishWaysOf} finds a block, so a row with none has no empty form.
 */
export function FinishRowWays({
  plane,
  turns,
  keys,
  who,
  nameOf,
  go,
}: {
  plane: PlaneId;
  turns: readonly FinishedTurn[];
  /** The finish rows the row stands for: a grouped row's every chat's. */
  keys: readonly string[];
  /** The row's chat, or chats, by name. */
  who: string;
  nameOf: (session: number) => string;
  /** Go to chat: the chat `session` in front. */
  go: (session: number) => void;
}): ReactNode {
  const blocks = finishWaysOf(turns, keys);
  if (blocks.length === 0) return null;
  const to = finishWaysTo(turns, keys);
  return (
    <BlockWaysOut
      blocks={blocks}
      who={who}
      into={to.tell.map(nameOf)}
      tell={() => tellEach(plane, to.tell, nameOf)}
      go={() => {
        if (to.go !== undefined) go(to.go);
      }}
      nextChats={to.allow.map((one) => ({ ...one, name: nameOf(one.session) }))}
      allowNext={(target, level, session) => {
        // Only a chat whose own block it is (Q273); the labels for you and the project are one
        // grant, judged on the first of them (Q278).
        const theirs = to.allow.filter((one) => one.target === target).map((one) => one.session);
        const one = session ?? theirs[0];
        if (one === undefined || !theirs.includes(one))
          return Promise.resolve({
            why: `Nothing was allowed: no chat here was blocked on ${target}.`,
          });
        return allowFor(
          plane,
          one,
          [target, level],
          session === undefined ? undefined : nameOf(one),
        ).finally(sandboxCommandReturned);
      }}
    />
  );
}
