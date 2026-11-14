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
  GUARD_SWITCH_LATER,
  NEXT_TIME_LEVELS,
  ROW_MOVED_JUST_NOW,
  RULE_WHERE,
  TOLD,
  TYPE_IT,
  WAY_SAID,
  allowsNextTime,
  checkSaid,
  guardGroupSaid,
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
   *  `target` at `level`, with no ask open, which the core judges again (`allow_next_time`).
   *  Where none is given, no Allow next time is offered. */
  allowNext?: (target: string, level: GrantLevel) => Promise<NextTimeSaid>;
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
export function BlockWaysOut({ blocks, who, tell, go, into = [], allowNext }: BlockWaysOutProps) {
  const id = useId();
  const ways = waysOut(blocks).filter((way) => way !== "allow-next" || allowNext !== undefined);
  const [open, setOpen] = useState<Way>();
  const [busy, setBusy] = useState(false);
  const [told, setTold] = useState<Told>();
  const [allowed, setAllowed] = useState<NextTimeSaid>();
  // **The settle guard on Allow next time's labels** (ADR 0097): measured from when what the
  // row offers, or which of its details is open, last changed, and from each label's move.
  const { drawnAt, tooSoon } = useSettledPlace<HTMLDivElement>(`${ways.join(" ")}\n${open ?? ""}`);
  if (ways.length === 0) return null;
  const checks = blocks.filter((one) => one.source === "harness_check");
  const guards = blocks.filter((one) => one.source === "guard");
  const sandboxes = blocks.filter((one) => one.source === "sandbox");
  // Each target once: a finish row's turns may hold the same block more than once.
  const allowable = [
    ...new Set(sandboxes.filter(allowsNextTime).flatMap((one) => one.target ?? [])),
  ];
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
  const allowIt = (target: string, level: GrantLevel) => {
    if (busy || allowNext === undefined) return;
    if (tooSoon()) {
      setAllowed({ why: ROW_MOVED_JUST_NOW });
      return;
    }
    setBusy(true);
    setAllowed(undefined);
    void allowNext(target, level)
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
          {allowable.map((target) => (
            <div key={target}>
              <p className="ui-text-body">{nextTimeSaid(target)}</p>
              <ButtonRow size="sm">
                {NEXT_TIME_LEVELS.map((level) => (
                  <Button
                    key={level}
                    variant="grants"
                    isDisabled={busy}
                    onPress={() => allowIt(target, level)}
                  >
                    {SANDBOX_ALLOW_SAID[level]}
                  </Button>
                ))}
              </ButtonRow>
            </div>
          ))}
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
 * **Allows `target` at `level` for each of `sessions`** (T1-34, Q273) through the core's
 * `allow_next_time`, and says what came of it: where it allowed for more than one chat, each by
 * name.
 */
async function allowEach(
  plane: PlaneId,
  sessions: readonly number[],
  [target, level]: [string, GrantLevel],
  nameOf: (session: number) => string,
): Promise<NextTimeSaid> {
  const said = await Promise.all(
    sessions.map((one) =>
      commands.allowNextTime(plane, one, target, level).then((done) => ({
        one,
        said: done.status === "ok" ? done.data.said : undefined,
        why: done.status === "error" ? done.error : undefined,
      })),
    ),
  );
  const named = (one: number, line: string) =>
    sessions.length > 1 ? `${nameOf(one)}: ${line}` : line;
  const allowed = said.flatMap((one) => (one.said === undefined ? [] : [named(one.one, one.said)]));
  const why = said.flatMap((one) => (one.why === undefined ? [] : [named(one.one, one.why)]));
  return {
    said: allowed.length === 0 ? undefined : allowed.join(" "),
    why: why.length === 0 ? undefined : why.join(" "),
  };
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
      allowNext={(target, level) =>
        allowEach(
          plane,
          to.allow.filter((one) => one.target === target).map((one) => one.session),
          [target, level],
          nameOf,
        ).finally(sandboxCommandReturned)
      }
    />
  );
}
