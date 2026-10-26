// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import { useTabStop } from "./roving";
import { RestartEndsLine } from "./RestartEnds";
import { commands, type InboxUpdate, type Offered, type PlaneId, type Shown } from "./bindings";
import { answerTaken, asksMoved } from "./asks";
import { DispatchGrantNotice } from "./DispatchGrantNotice";
import {
  ElsewhereGroup,
  elsewhereKey,
  elsewhereStops,
  hereKey,
  hereStops,
  OtherProjects,
  othersWaiting,
  QuietNotices,
  type Elsewhere,
} from "./InboxElsewhere";
import { KindMark, kindOfAsk, kindOfUpdate, RowAge, useNow } from "./InboxRow";
import {
  INBOX_SCOPES,
  keepInboxScope,
  keptInboxScope,
  scopeDrawn,
  scopeOf,
  useInboxScope,
  widerWaiting,
  type InboxScope,
  type ScopeCounts,
} from "./inboxScope";
import { quietSaid } from "./NeedsYou";
import { NoticeList } from "./Notice";
import { PersonaMark } from "./PersonaMark";
import {
  askKey,
  byChat,
  noteAnswered,
  noteSeen,
  replyBytes,
  ageOf,
  useAnswered,
  useAnsweredWhenGone,
  type Answered,
} from "./inboxRules";
import { KIND_SAID } from "./inboxUpdates";
import { useAllowedLately, type AllowedLately } from "./allowedLately";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { NoticeWayGuard, useNoticeSettle } from "./noticeSettle";
import { useWayPlaces } from "./wayPlaces";
import { useProjectShown } from "./projectShown";
import { capMarks } from "./combiningMarks";
import { AskWrites } from "./AskWrites";
import { ScopeSwitch } from "./ScopeSwitch";
import { Button, ButtonRow } from "./ui/button/Button";
import { Form, TextField } from "./ui/field/Field";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** What an empty Inbox says (I-12). */
export const NOTHING_WAITS = "Nothing is waiting on you";

/** What an Inbox says before its project's asks were read. */
export const NOT_READ_YET = "Reading what waits on you…";

/** What the Notices' section is called (#1695). */
export const NOTICES = "Notices";

/** What a chain is drawn as: the session first, the chat that asked last (I-9). */
export const chainSaid = (chain: readonly string[]) => chain.join(" › ");

/** The time an answer was given, or an update happened, as the list says it. */
const timeSaid = (at: number) =>
  new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

/**
 * **The Inbox** (#1692, spec #1688): everything in this project that waits on the person, at the
 * top of the right side's activity bar, the "for you" side (ADR 0038 as amended 2026-10-10).
 *
 * **One list, from the asks registry** (#1690): grouped by the chat that asks, the chat that has
 * waited longest first, each group under its chain (`steward 12 › #3046 drill › log watch`, I-9).
 * An ask is gone from here the moment its source stops waiting, wherever it was answered (I-8).
 *
 * **Answered in place, through the ask's own path** (I-3): the registry names the command its
 * source's Notice answers with, and a button here sends exactly that (`asks.answerTaken`).
 * Nothing new is opened, and an answer here clears its Notice on the chat's pane, because both
 * are drawn from the same source:
 *
 * - **a permission** prompt and a choice of several: each option the harness offered is a button;
 * - **a sandbox host**: Allow at each level policy leaves open, and Keep blocked; an Allow owes
 *   the chat its restart, as the block Notice's does, unless its proxy took it live (`onAnswered`);
 * - **a dispatch grant** is its own Notice, drawn here whole (`DispatchGrantNotice`): the brief,
 *   what the target works with and the boxes its digest is bound to are read before any Allow,
 *   and it reads the very list the pane's copy reads, so answering one clears the other;
 * - **a question** a chat's turn ended on gets a reply box, sent as the person's message;
 * - **a prompt in a harness's own terminal** says so, and is answered there.
 *
 * Every ask has **Go to chat**, which for a task opens it in its session's tab (`onGo`).
 *
 * **The keyboard** (I-11): ↑ and ↓ move between the asks and their buttons, Enter presses the
 * focused button, as it does anywhere, and Escape leaves for the chat in front. **No key of a
 * single letter does anything**, so a stray keystroke never answers an ask. The keyboard comes
 * in on the first ask itself, never on a button.
 *
 * **What an ask says is data** (I-1, #1690): a chain a chat named, a command line, a host are
 * drawn as text, and the only controls are the fixed words of the answers it offers.
 *
 * **A switch at the top picks whose asks are listed** (qw359, `inboxScope.ts`): every project's
 * this window knows of, this project's (the default) or those of the workspace in front on the
 * strip, each with its count, the pick kept for the project as the Chats view's is. Only the
 * scopes the window can list are offered. A scope with nothing in it says how many wait in the
 * nearest wider one, with one press to widen. Notices and updates are the project's and are
 * listed whatever the scope.
 *
 * **A row reads in three ranks** (qw359, `InboxRow.tsx`): what needs you, strongest; where it
 * is (project · workspace · persona), quieter; its kind and how long it has waited, smallest
 * and last. Its kind is a mark and words together, never a colour alone.
 *
 * **Empty, it says so** (I-12) — "Nothing is waiting on you" — and lists what was answered here
 * lately, read-only: when, which chat, what it asked and the answer.
 *
 * **The project's Notices follow the asks** (#1695, I-4): what stood under the tab strip and what
 * the Alerts drawer listed, each a Notice with its ways out (`notices`), the most important
 * first (`NoticeList`). They wait on nothing the person must decide, so they come after the
 * asks; they stand until they are dealt with, so they come before what merely happened.
 *
 * **What the asks registry cannot see follows the asks** (#1695, `InboxElsewhere`): a chat
 * waiting in another window's project is a group of its own, and a chat that cannot say it is
 * waiting is a Notice. The ✋'s list named both until it was retired.
 *
 * **Updates follow, newest first** (#1693, I-6): a task that finished or failed, a
 * doctor finding, a chat that came back, a sandbox change, a dispatch refused while nobody was
 * there, a Smart close that stopped. Each has Dismiss, and Go to chat where it is about one; a
 * refused dispatch has its own three answers. **Mark all read and Dismiss all act on updates
 * alone** (I-10): an ask is never answered in bulk, so neither button is drawn among them.
 */
export function Inbox({
  plane,
  asks,
  onGo,
  onLeave,
  onAnswered,
  onIgnore,
  whyOf,
  asked,
  personaOf,
  updates,
  notices,
  windowLines,
  onNotices,
  onNoticeAnswer,
  elsewhere,
  here,
  workspaceOf,
  workspaceNamed,
}: {
  plane: PlaneId;
  /** The project's asks, as the registry derived them last; nothing before the first read. */
  asks: readonly Shown[] | undefined;
  /** The chat that asked, in front: a task in its session's tab. */
  onGo: (session: number) => void;
  /** Escape: the keyboard goes back to the chat in front. */
  onLeave: () => void;
  /** An answer from here applied: what the source's Notice does after it is done here too.
   *  `live`: the chat took it at once, so nothing restarts for it (#1666). */
  onAnswered?: (ask: Shown, option: Offered, live: boolean) => void;
  /**
   * The queue's own Ignore for a chat waiting on a reply or in its terminal
   * (`needs.ignore:<session>`): put away until it asks again. Never offered on a decision.
   */
  onIgnore?: (session: number) => void;
  /**
   * Why a chat in the queue waits, where it is not that it asked anything (#1448): a report
   * with nowhere to go. Said in place of "Waiting on your reply", and such a chat gets no reply
   * box, since nothing it asked is answered by typing (#1700). A task that failed and a Smart
   * close that stopped are updates (#1693).
   */
  whyOf?: (session: number) => string | undefined;
  /**
   * Whether chat `session` itself is waiting on the person's reply (its state is `waiting`):
   * such a chat gets the reply box even where it also has a reason to be in the queue (#1700).
   */
  asked?: (session: number) => boolean;
  /** The persona chat `session` runs as, where it runs as one: its mark leads the group (#1449),
   *  as it led the chat's row in the hand's list. */
  personaOf?: (session: number) => string | null | undefined;
  /** The project's updates, drawn after the asks (#1693); none before they were read. */
  updates?: Updates;
  /** The project's Notices (#1695): every `Notice` drawn in it is listed here, in its order. */
  notices?: ReactNode;
  /** How many Notices the list holds that the status line counts (`countsInStatusBar`). */
  onNotices?: (count: number) => void;
  /** A Notice answering the person's own press arrived (`ANSWERS`, D-1695-3). */
  onNoticeAnswer?: () => void;
  /** What waits where this project's registry cannot see (#1695): chats in other windows'
   *  projects, and chats that cannot say they wait. */
  elsewhere?: Elsewhere;
  /** **The window's own lines** (D-LB-1, #1695): what the window says about itself, listed at
   *  the top of the Notices while this project is in front. The window draws its one list
   *  into the element `at` is handed, and says how many it lists. */
  windowLines?: WindowLines;
  /**
   * **The workspace in front on the strip** (qw359), by the key {@link workspaceOf} answers
   * with: This workspace lists the asks of the chats filed there, and follows the strip. The
   * plane root has a key of its own, so there it lists the asks of no workspace. Without it
   * and `workspaceOf`, no workspace scope is offered.
   */
  here?: string;
  /** Where chat `session` is filed, by the strip's own key. */
  workspaceOf?: (session: number) => string | undefined;
  /** What the strip calls the workspace with that key: its name, or the plane root's words. */
  workspaceNamed?: (key: string) => string;
}) {
  if (asks !== undefined) noteSeen(plane, asks);
  /** How many lines the window lists in this Inbox, and where it draws them. */
  const windowLinesCount = windowLines?.count ?? 0;
  // **Only while this project is in front** (`projectShown.tsx`). The window has one list and
  // draws it in the place it was last handed; a project kept behind another keeps what it drew,
  // so it gives its place up as it leaves the front and hands it over again as it comes back.
  const shown = useProjectShown();
  const placeWindowLines = shown ? windowLines?.at : undefined;
  const asking = elsewhere?.asking ?? [];
  const quiet = elsewhere?.quiet ?? [];
  // Named only where the window can bring the project forward.
  const others = elsewhere?.onOpen === undefined ? [] : othersWaiting(plane, elsewhere);
  /**
   * **Whose asks are listed** (qw359). Each scope the window can list is counted, and only
   * those are offered: the project's always; the workspace's where the window says which is
   * in front; every project's where it says what waits elsewhere.
   */
  const picked = useInboxScope(plane);
  const inHere = (ask: Shown) => workspaceOf?.(ask.session) === here;
  const counts: ScopeCounts = {
    project: asks?.length ?? 0,
    ...(here !== undefined && workspaceOf !== undefined
      ? { workspace: (asks ?? []).filter(inHere).length }
      : {}),
    ...(elsewhere !== undefined
      ? {
          all:
            (asks?.length ?? 0) + asking.length + others.reduce((sum, one) => sum + one.count, 0),
        }
      : {}),
  };
  const scope = scopeDrawn(picked, counts);
  const listed = asks === undefined ? [] : scope === "workspace" ? asks.filter(inHere) : asks;
  /** What waits in other projects is listed among every project's alone. */
  const askingShown = scope === "all" ? asking : [];
  const othersShown = scope === "all" ? others : [];
  /** This project's name, as its tab says it: said on a row only among other projects'. */
  const project = elsewhere?.waitingHere?.find((one) => one.plane === plane)?.project;
  const whereOf = (session: number): string => {
    const filed = workspaceOf?.(session);
    return [
      scope === "all" ? project : undefined,
      filed === undefined ? undefined : (workspaceNamed?.(filed) ?? filed),
      personaOf?.(session),
    ]
      .filter((part): part is string => typeof part === "string" && part !== "")
      .join(" · ");
  };
  const now = useNow();
  /** The nearest wider scope with something waiting: an empty scope's way out. */
  const wider = widerWaiting(scope, counts);
  const widen = () => {
    if (wider !== undefined) keepInboxScope(plane, wider.scope);
  };
  const groups = byChat(listed, (ask) => ageOf(plane, ask));
  const recent = useAnswered(plane);
  // Read only while nothing waits, where the empty Inbox lists them (D-1700-7).
  const allowedLately = useAllowedLately(
    plane,
    asks !== undefined && asks.length === 0 && asking.length === 0 && others.length === 0,
  );
  /** A way out of a Notice drawn here was pressed on an ask: answered here once it goes. */
  const touch = useAnsweredWhenGone(plane, asks);
  /** Each chat's rows: a chat's held dispatches are one Notice, which asks about the first
   *  and says how many wait behind it. */
  const rows = groups.map((group) => ({
    ...group,
    asks: group.asks.filter(
      (ask, at) =>
        ask.source !== "dispatch" ||
        group.asks.findIndex((one) => one.source === "dispatch") === at,
    ),
  }));
  const shapeOfAsk = (ask: Shown) => shapeOf(ask, whyOf, onIgnore, asked);
  const shapes = new Map(
    rows.flatMap((group) => group.asks).map((ask) => [askKey(ask), shapeOfAsk(ask)]),
  );
  // **One Tab stop for the list** (`docs/ui-primitives.md`, ADR 0037): the window's roving
  // focus, so ↑ and ↓, Home and End move through the asks and their buttons in order.
  const stops = [
    ...rows
      .flatMap((group) => group.asks)
      .flatMap((ask) => {
        const shape = shapes.get(askKey(ask)) ?? shapeOfAsk(ask);
        return stopIds(askKey(ask), shape, shape.answers ? shape.offers : []);
      }),
    ...elsewhereStops(askingShown),
    ...hereStops(othersShown),
    ...updateStops(updates),
  ];
  // The keyboard comes in on the first ask, or the first update where none asks: an item, and
  // never one of its buttons or Mark all read.
  const firstAsk = rows[0]?.asks[0];
  const firstUpdate = updates?.rows?.[0]?.update;
  const stop = useTabStop(
    firstAsk !== undefined
      ? askKey(firstAsk)
      : askingShown[0] !== undefined
        ? elsewhereKey(askingShown[0])
        : othersShown[0] !== undefined
          ? hereKey(othersShown[0].plane)
          : firstUpdate === undefined
            ? undefined
            : `update:${firstUpdate.key}`,
    stops,
  );

  const leave = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    onLeave();
  };

  /**
   * **The keyboard stays in the list after an answer** (#1700): an ask answered here leaves
   * the list, and the button the keyboard was on goes with it. Focus on an element that goes is
   * focus on the page, where the next key does nothing, so it comes back to the first item.
   * Only then: an element the keyboard left for the page while it is still drawn (a press on
   * nothing) is the person's own move, and the list never takes the keyboard back for it.
   */
  const listAt = useRef<HTMLDivElement>(null);
  const lastOn = useRef<Element | null>(null);
  useLayoutEffect(() => {
    const was = lastOn.current;
    if (was === null || was.isConnected) return;
    lastOn.current = null;
    const on = document.activeElement;
    if (on !== null && on !== document.body) return;
    listAt.current?.querySelector<HTMLElement>("li.inbox-ask, li.inbox-update")?.focus();
  });

  const [noticed, setNoticed] = useState(0);
  const {
    at: noticesAt,
    allows: noticesAllow,
    said: noticesSaid,
  } = useNoticeSettle<HTMLElement>("moves");
  const counted = useCallback(
    (count: number, listed: number) => {
      // The section is drawn while it lists anything, counted or not: a doctor finding alone
      // is still a Notice to read here.
      setNoticed(listed);
      onNotices?.(count);
    },
    [onNotices],
  );

  return (
    <section className="inbox" data-view="inbox" aria-label="Inbox">
      {/* Drawn while any scope lists something, as the Chats view's is: nothing to narrow
          otherwise, and an empty Inbox is its one sentence. */}
      {Object.keys(counts).length > 1 && Object.values(counts).some((count) => count > 0) && (
        <div className="inbox-scope">
          <ScopeSwitch
            label={SCOPE_LABEL}
            scopes={INBOX_SCOPES.flatMap((one) => {
              const count = counts[one.scope];
              return count === undefined
                ? []
                : [{ ...one, count, name: `${one.says}, ${count} waiting on you` }];
            })}
            scope={scope}
            onPick={(to) => keepInboxScope(plane, to)}
          />
        </div>
      )}
      <RovingFocusGroup.Root asChild orientation="vertical" loop={false} {...stop}>
        <div
          className="inbox-list"
          ref={listAt}
          onKeyDown={leave}
          onFocus={(event) => {
            lastOn.current = event.target;
          }}
          onBlur={(event) => {
            // Leaving for somewhere else lets go; an element taken away is still remembered.
            if (event.relatedTarget !== null && !listAt.current?.contains(event.relatedTarget))
              lastOn.current = null;
          }}
        >
          {asks === undefined ? (
            // Not read yet is not empty; what the window already knows waits wider is offered.
            <EmptyScope said={NOT_READ_YET} wider={wider} onWiden={widen} />
          ) : rows.length === 0 && askingShown.length === 0 && othersShown.length === 0 ? (
            wider === undefined ? (
              <Empty
                recent={recent}
                allowed={allowedLately}
                quiet={quiet.length > 0 ? quietSaid(quiet) : undefined}
              />
            ) : (
              <EmptyScope
                said={`Nothing is waiting on you in ${scopeOf(scope)}.`}
                wider={wider}
                onWiden={widen}
              />
            )
          ) : (
            rows.map((group) => (
              <section
                key={group.session}
                className="inbox-chat"
                aria-label={chainSaid(group.chain)}
                data-session={group.session}
              >
                <h3 className="inbox-chain">
                  {(() => {
                    const persona = personaOf?.(group.session);
                    return persona ? <PersonaMark persona={persona} /> : null;
                  })()}
                  {chainSaid(group.chain)}
                </h3>
                <ul>
                  {group.asks.map((ask) => (
                    <Ask
                      key={askKey(ask)}
                      plane={plane}
                      ask={ask}
                      dispatches={(asks ?? []).filter(
                        (one) => one.session === ask.session && one.source === "dispatch",
                      )}
                      shape={shapes.get(askKey(ask)) ?? shapeOfAsk(ask)}
                      where={whereOf(ask.session)}
                      now={now}
                      onGo={() => onGo(ask.session)}
                      onAnswered={onAnswered}
                      onIgnore={onIgnore}
                      onTouched={(answer) => touch(ask, answer)}
                      above={stops.slice(0, stops.indexOf(askKey(ask))).join("\n")}
                    />
                  ))}
                </ul>
              </section>
            ))
          )}
          {elsewhere !== undefined &&
            askingShown.map((item) => (
              <ElsewhereGroup key={elsewhereKey(item)} item={item} onPress={elsewhere.onPress} />
            ))}
          {elsewhere?.onOpen !== undefined && othersShown.length > 0 && (
            <OtherProjects others={othersShown} onOpen={elsewhere.onOpen} now={now} />
          )}
          {(notices !== undefined || quiet.length > 0 || windowLines !== undefined) && (
            <section
              className="inbox-notices"
              aria-label={NOTICES}
              hidden={noticed === 0 && windowLinesCount === 0}
              ref={noticesAt}
            >
              <PanelSectionTitle level={3} className="inbox-chain">
                {NOTICES}
              </PanelSectionTitle>
              <NothingDone said={noticesSaid} />
              {/* The window's own lines, listed first: the window draws its one list here. */}
              {placeWindowLines !== undefined && (
                <div className="inbox-window-lines" ref={placeWindowLines} />
              )}
              <NoticeWayGuard value={noticesAllow}>
                <NoticeList onCount={counted} onAnswer={onNoticeAnswer}>
                  {notices}
                  {elsewhere !== undefined && (
                    <QuietNotices plane={plane} quiet={quiet} onPress={elsewhere.onPress} />
                  )}
                </NoticeList>
              </NoticeWayGuard>
            </section>
          )}
          {updates !== undefined && <UpdateList updates={updates} stops={stops} />}
        </div>
      </RovingFocusGroup.Root>
    </section>
  );
}

/** Where the window's own lines are drawn in a project's Inbox, and how many there are. */
export type WindowLines = {
  /** Handed the element the window's list is drawn into, and `null` once it goes. */
  at: (element: HTMLDivElement | null) => void;
  /** How many lines the window lists now. */
  count: number;
};

/**
 * **The asks whose answers are Allow and Keep blocked** (ADR 0097 §6): a permission prompt, a
 * sandbox's host, folder or local service, and a browser's sites. On these, each answer that
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
]);

/** What one ask's row offers, decided once for its stops and its drawing. */
type Shape = {
  /** Why the chat waits, said in place of what it asked. */
  why?: string;
  /** The line drawn: the reason, or what it asked, each run of stacked marks cut (#1146). */
  line: string;
  /** The answers drawn: none that allows, where the line is not what the ask said whole. */
  offers: readonly Offered[];
  /** Its answers are buttons here. */
  answers: boolean;
  /** It gets a reply box. */
  reply: boolean;
  /** It gets a box for why it is kept blocked, which its chat is told with the answer
   *  (#1987). */
  reason: boolean;
  /** It has the queue's Ignore. */
  ignore: boolean;
  /** It has Go to chat: every ask but a profile's, which is about no chat (#2035). */
  go: boolean;
};

function shapeOf(
  ask: Shown,
  whyOf: ((session: number) => string | undefined) | undefined,
  onIgnore: ((session: number) => void) | undefined,
  asked: ((session: number) => boolean) | undefined,
): Shape {
  const waits = ask.source === "question" || ask.source === "terminal";
  const why = ask.source === "question" ? whyOf?.(ask.session) : undefined;
  // **A line cut is not the ask, so nothing that lets it run is offered on it** (#1146): the
  // core offers Allow only where the line says the action word for word
  // (`hooked::shown_in_full`), and a run of combining marks cut here would undo that.
  const { text: line, capped } = capMarks(why ?? ask.says);
  // An edit whose change is not shown is never allowed from here (#1146), whatever it offers.
  const unshown = ask.writes?.kind === "not-shown";
  return {
    why,
    line,
    offers:
      (capped && why === undefined) || unshown
        ? ask.options.filter((one) => !one.allows)
        : ask.options,
    answers:
      ask.source !== "dispatch" && ask.options.length > 0 && ask.answer.via !== "in-its-pane",
    // A chat that both asked and has a reason (a report with nowhere to go, say) still asked:
    // while it is itself waiting, it gets the box beside the reason (#1700).
    reply: ask.source === "question" && (why === undefined || asked?.(ask.session) === true),
    reason:
      (ask.answer.via === "sandbox-block" ||
        ask.answer.via === "browser" ||
        ask.answer.via === "browser-sites") &&
      ask.options.some((one) => one.id === "keep"),
    // A chat that only waits for the person's next word asks again at its next stop, so it
    // may be put away, as the queue always let it be. A decision never is.
    ignore: waits && onIgnore !== undefined,
    go: ask.source !== "sign-in" && ask.source !== "sandbox-only",
  };
}

/** The stops of one ask's row, in the order it draws them. */
function stopIds(key: string, shape: Shape, options: readonly Offered[] = []): string[] {
  return [
    key,
    ...(shape.reason ? [`${key}:reason`] : []),
    ...options.map((option) => `${key}:${option.id}`),
    ...(shape.reply ? [`${key}:reply`, `${key}:send`] : []),
    // A profile's Ask is about no chat (#2035): there is no chat to go to.
    ...(shape.go ? [`${key}:go`] : []),
    ...(shape.ignore ? [`${key}:ignore`] : []),
  ];
}

/** One stop of the list's roving focus. */
function Stop({
  id,
  focusable = true,
  children,
}: {
  id: string;
  focusable?: boolean;
  children: ReactElement;
}) {
  return (
    <RovingFocusGroup.Item asChild tabStopId={id} focusable={focusable}>
      {children}
    </RovingFocusGroup.Item>
  );
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

/** What a row says of where it waits. */
const SOURCE_SAID: Record<Shown["source"], string> = {
  permission: "Asks your permission",
  dispatch: "Asks to dispatch",
  "sandbox-host": "Asks to reach a host",
  "sandbox-write": "Asks to write in a folder",
  "sandbox-local": "Asks to reach a service on this machine",
  "workspace-removal": "Asks you to remove a workspace",
  "proposed-change": "Proposes a change for you to apply",
  "runner-write": "Asks, from a pool's runner, to change this project",
  "sign-in": "Asks you to sign a profile in",
  "sandbox-only": "Asks you to make a sandbox-only profile",
  browser: "Asks what its browser may reach",
  "browser-needs-you": "Needs you in its browser",
  "browser-site": "Asks to let its browser reach a new site",
  "pool-login": "Asks to log its browser in on a pool's runner",
  "pool-tool": "Asks to send a tool its vault values for a run on a pool's runner",
  terminal: "In its terminal",
  question: "Waiting on your reply",
};

function Ask({
  plane,
  ask,
  dispatches,
  shape,
  onGo,
  onAnswered,
  onIgnore,
  onTouched,
  above,
  where,
  now,
}: {
  plane: PlaneId;
  ask: Shown;
  /** Where its chat is: project · workspace · persona, as much as the window says. */
  where: string;
  /** The time its age is said against. */
  now: number;
  /** Its chat's dispatch asks, which its dispatch Notice draws its answers from (#1700). */
  dispatches: readonly Shown[];
  shape: Shape;
  /** The stops drawn above it: one changing moves it (`useSettledPlace`). */
  above: string;
  onGo: () => void;
  onAnswered?: (ask: Shown, option: Offered, live: boolean) => void;
  onIgnore?: (session: number) => void;
  /** A way out of the Notice drawn in the row was pressed, with its words. */
  onTouched: (answer: string) => void;
}) {
  const id = useId();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  /** Why the person keeps it blocked, if they say: told to the chat with the answer. */
  const [reason, setReason] = useState("");
  const who = chainSaid(ask.chain);
  const key = askKey(ask);
  /** Its words are drawn whole: it is a dispatch's own Notice, or an answer here allows. */
  const whole =
    ask.source === "dispatch" || (shape.answers && shape.offers.some((one) => one.allows));

  // An edit's change arrives after its row (#1146): its Allow waits to settle from then.
  const [changeDrawn, setChangeDrawn] = useState(false);
  const onChangeDrawn = useCallback(() => setChangeDrawn(true), []);
  const here = [
    above,
    shape.line,
    ...shape.offers.map((one) => one.label),
    changeDrawn ? "drawn" : "",
  ].join("\n");
  const { drawn, tooSoon } = useSettledPlace<HTMLLIElement>(here);
  /** Whether `option` is an Allow on an ask of Allow and Keep blocked (ADR 0097 §6). */
  const grants = (option: Offered) => option.allows && ALLOW_OR_KEEP.has(ask.source);
  const answer = (option: Offered) => {
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
    void answerTaken(plane, ask, option.id, option.id === "keep" ? reason : undefined)
      .then(({ refused, live, opened }) => {
        if (refused !== undefined) {
          setSaid(refused);
          return;
        }
        // A dialog of the window's was opened, where the person decides (qw356): the ask
        // still waits, and nothing was answered here.
        if (opened === true) return;
        noteAnswered(plane, {
          at: Date.now(),
          chain: ask.chain,
          says: ask.says,
          answer: option.label,
        });
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
      <Reply plane={plane} ask={ask} stop={key} />
    ) : shape.answers ? (
      <WithWrites ask={ask} onDrawn={onChangeDrawn}>
        {shape.reason && (
          <Stop id={`${key}:reason`}>
            <TextField
              className="inbox-reason"
              aria-label={`Why you keep it blocked, told to ${who}`}
              value={reason}
              placeholder="Why keep it blocked? (optional, the chat is told)"
              maxLength={REASON_TYPED_MOST}
              spellCheck
              onChange={setReason}
              onKeyDown={keepHomeAndEnd}
            />
          </Stop>
        )}
        <ButtonRow className="inbox-answers" role="group" aria-label={`Answers to ${who}`}>
          {shape.offers.map((option) => (
            // Never `disabled` while busy: a disabled button drops the keyboard to the page
            // (#1700). It says it is busy, and `answer` sends nothing a second time.
            <Stop key={option.id} id={`${key}:${option.id}`}>
              <Button
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
                onPress={() => answer(option)}
              >
                {option.label}
              </Button>
            </Stop>
          ))}
        </ButtonRow>
      </WithWrites>
    ) : null;

  return (
    <Stop id={key}>
      <li
        ref={drawn}
        className="inbox-ask"
        data-source={ask.source}
        aria-labelledby={`${id}-says`}
        aria-describedby={`${id}-whose`}
      >
        {/* **The card's top row** (R132): the chat's name in bold, the workspace quiet at its
            right. */}
        <p className="inbox-ask-top">
          <span className="inbox-who ui-text-body ui-text-strong">{who}</span>
          {where !== "" && (
            <Tooltip tip={where}>
              <span className="inbox-where inbox-secondary ui-text-meta">{where}</span>
            </Tooltip>
          )}
        </p>
        <p className="inbox-says">
          <KindMark kind={kindOfAsk(ask.source)} />
          {/* The ask's words are the source's, drawn as text and never as a control. What an
              Allow here would let run is never cut (#1146): it is read whole first. */}
          <Tooltip tip={shape.line}>
            <span
              id={`${id}-says`}
              className={`ask-says inbox-primary ui-text-body ui-text-strong${whole ? "" : " inbox-clamped"}`}
            >
              {shape.line}
            </span>
          </Tooltip>
          <Tooltip tip={SOURCE_SAID[ask.source]}>
            <span className="inbox-meta ui-text-meta">
              <span id={`${id}-whose`} className="inbox-source">
                {SOURCE_SAID[ask.source]}
              </span>
              <RowAge when={ageOf(plane, ask)[0]} now={now} since="Waiting since" lead />
            </span>
          </Tooltip>
        </p>
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
        {body}
        {said !== undefined && (
          <p className="inbox-refused" role="status">
            {said}
          </p>
        )}
        {/* Its ways on, side by side at its foot: one line, never one each. */}
        <ButtonRow size="sm" className="inbox-acts">
          {shape.go && (
            <Stop id={`${key}:go`}>
              <Button
                size="sm"
                // R132: the way on is the card's primary press; no answer is drawn this way.
                variant={shape.answers ? "ghost" : "primary"}
                aria-label={`Go to chat ${who}`}
                onPress={onGo}
                className="inbox-go"
              >
                Go to chat
              </Button>
            </Stop>
          )}
          {shape.ignore && onIgnore !== undefined && (
            <Stop id={`${key}:ignore`}>
              <Button
                size="sm"
                variant="ghost"
                aria-label={`Ignore ${who} until it asks again`}
                tooltip={`Ignore ${who} until it asks again`}
                onPress={() => onIgnore(ask.session)}
                className="inbox-go"
              >
                Ignore
              </Button>
            </Stop>
          )}
        </ButtonRow>
      </li>
    </Stop>
  );
}

/**
 * **A reply box for a chat waiting on the person's reply** (I-3): what is typed is sent to the
 * chat as the person's message, one paste and Enter (`replyBytes`), through the pane's own
 * input path. Short replies only; anything longer is better written in the chat itself.
 */
function Reply({ plane, ask, stop }: { plane: PlaneId; ask: Shown; stop: string }) {
  const [text, setText] = useState("");
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const box = useRef<HTMLInputElement>(null);
  const who = chainSaid(ask.chain);
  const send = () => {
    const bytes = replyBytes(text);
    if (bytes === undefined || busy) return;
    // Send reply is disabled while it sends and once the box is empty, and a disabled button
    // drops the keyboard to the page (#1700): it goes back to the box instead.
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
  return (
    <Form
      className="inbox-reply"
      onSubmit={(event) => {
        event.preventDefault();
        send();
      }}
    >
      {/* A stop of the list too, so ↑ and ↓ reach it; Home and End there are the box's own. */}
      <Stop id={`${stop}:reply`}>
        <TextField
          inputRef={box}
          className="inbox-reply-box"
          aria-label={`Reply to ${who}`}
          value={text}
          placeholder="Your reply"
          spellCheck
          onChange={setText}
          onKeyDown={keepHomeAndEnd}
        />
      </Stop>
      <Stop id={`${stop}:send`} focusable={!busy && replyBytes(text) !== undefined}>
        <Button type="submit" isDisabled={busy || replyBytes(text) === undefined}>
          Send reply
        </Button>
      </Stop>
      {said !== undefined && (
        <p className="inbox-refused" role="status">
          {said}
        </p>
      )}
    </Form>
  );
}

/** The most characters the reason box takes; the chat is told at most 200 of them. */
const REASON_TYPED_MOST = 400;

/** What an Allow pressed on an ask drawn or moved just now says: nothing was allowed. */
export const MOVED_JUST_NOW =
  "This ask was drawn or moved just now, so nothing was allowed. Read it and press again.";

export { grants, NOTICE_MOVED_JUST_NOW } from "./noticeSettle";

/** What a reply refused because its chat moved on says. */
export const MOVED_ON =
  "This chat no longer waits on a reply, so nothing was sent. Go to the chat to see what it does now.";

/** What a reply refused because the check could not be made says. */
export const NOT_CHECKED =
  "purlis could not check that this chat still waits on a reply, so nothing was sent. Try again, or reply in the chat itself.";

/** Whether `ask` still waits on a reply, the project's asks read again now: `undefined` where
 *  they could not be read, which sends nothing either. */
async function stillWaits(plane: PlaneId, ask: Shown): Promise<boolean | undefined> {
  const answer = await commands.asksWaiting(plane).catch(() => undefined);
  const asks =
    answer?.status === "ok" ? (answer.data as { asks?: readonly Shown[] } | null)?.asks : undefined;
  if (asks === undefined) return undefined;
  return asks.some((one) => one.session === ask.session && one.ask === ask.ask);
}

/**
 * **Home and End move the cursor in the reply box** (#1700): the box is a stop of the list's
 * roving focus, which takes Home and End for the list's ends. Done here instead, so the list
 * never sees them: the cursor goes to the start or the end, Shift extending the selection.
 */
function keepHomeAndEnd(event: KeyboardEvent<HTMLInputElement>) {
  if (event.key !== "Home" && event.key !== "End") return;
  if (event.metaKey || event.ctrlKey || event.altKey) return;
  event.preventDefault();
  const box = event.currentTarget;
  const to = event.key === "Home" ? 0 : box.value.length;
  if (!event.shiftKey) {
    box.setSelectionRange(to, to);
    return;
  }
  // Extending: the end the selection is anchored at stays where it is.
  const anchor = box.selectionDirection === "backward" ? box.selectionEnd : box.selectionStart;
  const from = anchor ?? to;
  box.setSelectionRange(Math.min(from, to), Math.max(from, to), to < from ? "backward" : "forward");
}

/** An empty Inbox: the sentence, and what was answered here lately, read-only (I-12). Where a
 *  chat cannot say it waits, the sentence says that instead. */
function Empty({
  recent,
  allowed,
  quiet,
}: {
  recent: readonly Answered[];
  /** What was allowed on this machine in the last day, from the grants (D-1700-7). */
  allowed: readonly AllowedLately[];
  quiet?: string;
}) {
  return (
    <>
      {/* Never "nothing" over a chat that cannot say it waits (charter-app#52): the faint
          hand's sentence, and the chats are the Notices below. */}
      <p className="inbox-none">{quiet ?? NOTHING_WAITS}</p>
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

/** What the scope switch is called. */
export const SCOPE_LABEL = "Show what needs you in";

/**
 * **A scope with nothing in it, while a wider one has something** (qw359): it says so, says how
 * many wait in the nearest wider scope, and widens in one press, so a narrow pick never reads
 * as "nothing needs you".
 */
function EmptyScope({
  said,
  wider,
  onWiden,
}: {
  /** What the scope says of itself: that nothing waits in it, or that it is not read yet. */
  said: string;
  wider: { scope: InboxScope; count: number } | undefined;
  onWiden: () => void;
}) {
  if (wider === undefined) return <p className="inbox-none">{said}</p>;
  return (
    <div className="inbox-scope-empty">
      <p className="inbox-none">{said}</p>
      <p className="inbox-none">
        {`${wider.count} ${wider.count === 1 ? "waits" : "wait"} in ${scopeOf(wider.scope)}.`}
      </p>
      <Button size="sm" onPress={onWiden}>
        {`Show ${scopeOf(wider.scope)}`}
      </Button>
    </div>
  );
}

/** What the empty Inbox's look back at the grants of the last day is called (D-1700-7). */
export const ALLOWED_LATELY = "Allowed in the last day";

/** One update as the Inbox draws it, with what can be done with it there. */
export type UpdateRow = {
  update: InboxUpdate;
  /** Go to chat, where it is about a chat that is still there. */
  go?: () => void;
  /** A sentence of the source's own under it, before its answers. */
  more?: string;
  /** Its source's own answers, in their order: a refused dispatch's three. */
  answers?: readonly UpdateAnswer[];
  /** Put it away: and, where its source lists it, put away there too. */
  dismiss: () => void;
  /** What Dismiss does, as its tooltip, where it does more than put the update away. */
  dismissSays?: string;
};

/** One of an update's answers. */
export type UpdateAnswer = {
  label: string;
  /** Its accessible name: what it does, whole. */
  name: string;
  /** What it does, as its tooltip. */
  title: string;
  /** Whether it allows something: drawn as an Allow is. */
  allows?: boolean;
  press: () => void;
};

/** The project's updates and the two acts on all of them. */
export type Updates = {
  /** Newest first; nothing before they were read. */
  rows: readonly UpdateRow[] | undefined;
  /** Every update listed, marked read: none is put away. */
  onMarkAllRead: () => void;
  /** Every update listed, put away. No ask is touched. */
  onDismissAll: () => void;
};

/** The stops of the updates' section, in the order it draws them. */
function updateStops(updates: Updates | undefined): string[] {
  const rows = updates?.rows ?? [];
  if (rows.length === 0) return [];
  return [
    READ_ALL_STOP,
    DISMISS_ALL_STOP,
    ...rows.flatMap(({ update, go, answers = [] }) => {
      const key = `update:${update.key}`;
      return [
        key,
        ...answers.map((_, at) => `${key}:${at}`),
        ...(go === undefined ? [] : [`${key}:go`]),
        `${key}:dismiss`,
      ];
    }),
  ];
}

/** The time now: when an update was drawn where it stands, or when an answer was pressed. */
const placedAt = () => performance.now();

const READ_ALL_STOP = "inbox:read-all";
const DISMISS_ALL_STOP = "inbox:dismiss-all";

/** What the updates' section is called. */
export const UPDATES = "Updates";

/** The updates, after the asks: newest first, each with Dismiss (#1693). */
function UpdateList({ updates, stops }: { updates: Updates; stops: readonly string[] }) {
  const rows = updates.rows ?? [];
  if (rows.length === 0) return null;
  const unread = rows.some(({ update }) => !update.read);
  return (
    <section className="inbox-updates" aria-label={UPDATES}>
      <div className="inbox-updates-head">
        <PanelSectionTitle level={3} className="inbox-chain">
          {UPDATES}
        </PanelSectionTitle>
        <Stop id={READ_ALL_STOP} focusable={unread}>
          <Button
            size="sm"
            variant="ghost"
            isDisabled={!unread}
            tooltip="Mark every update read. Nothing that waits on you is answered."
            onPress={updates.onMarkAllRead}
            className="inbox-go"
          >
            Mark all read
          </Button>
        </Stop>
        <Stop id={DISMISS_ALL_STOP}>
          <Button
            size="sm"
            variant="ghost"
            tooltip="Put every update away. Nothing that waits on you is answered."
            onPress={updates.onDismissAll}
            className="inbox-go"
          >
            Dismiss all
          </Button>
        </Stop>
      </div>
      <ul>
        {rows.map((row) => (
          <UpdateItem
            key={row.update.key}
            row={row}
            above={stops.slice(0, stops.indexOf(`update:${row.update.key}`)).join("\n")}
          />
        ))}
      </ul>
    </section>
  );
}

/** A row's answers that grant: an ask's or an update's Allow, or a way out of its Notice's. */
const granting = (row: Element) => row.querySelectorAll('[data-variant="grants"], .allows');
const standsFirst = () => false;

/**
 * **When a row was last drawn where it stands, or moved** (#1693, #1695): what an Allow on it
 * waits {@link SETTLE_MS} after, so what is under the pointer is what the person read.
 *
 * `here` is what the row stands under and offers: the stops above it and its own words and
 * answers. It is taken in the very render that draws it, so no press lands on a place whose time
 * is not known yet. **Where each of its answers that grant was last drawn, and since when**
 * (#3167, `useWayPlaces`): something above it can grow without a stop of its own changing (a
 * sentence said under another row, the recent answers of an empty list), its own words can wrap
 * again at a new width, and the list can be made to scroll back; none of these need reach this
 * row's render. So each answer's own box is read after each render, on every change that can
 * move it, and again at the press: one that differs is a move.
 */
function useSettledPlace<T extends HTMLElement>(here: string) {
  const [placed, setPlaced] = useState(() => ({ here, at: placedAt() }));
  if (placed.here !== here) setPlaced({ here, at: placedAt() });
  const drawn = useRef<T>(null);
  // Its first drawing is `placed`'s: an answer read for the first time is no move of its own.
  const { movedAt } = useWayPlaces(drawn, granting, standsFirst);
  const tooSoon = () => placedAt() - Math.max(placed.at, movedAt()) < SETTLE_MS;
  return { drawn, tooSoon };
}

/**
 * **What a press that did nothing says, where it was pressed** (#1693, #1695): an answer on an
 * update, or a way out of a granting Notice, pressed on something drawn or moved just now.
 */
function NothingDone({ said }: { said: string | undefined }) {
  return said === undefined ? null : (
    <p className="inbox-refused" role="status">
      {said}
    </p>
  );
}

function UpdateItem({ row, above }: { row: UpdateRow; above: string }) {
  const { update, go, more, answers = [], dismiss, dismissSays } = row;
  const id = useId();
  const [said, setSaid] = useState<string>();
  /**
   * **When this update was last drawn where it stands**: at its first drawing, and whenever
   * what stands above it changes, which moves it. Set in the very render that draws it, as a
   * dispatch's Notice sets its own (`TaskBlocksNotice`), so no press lands on a place whose
   * time is not known yet.
   */
  // What the row offers is part of where it stands: an answer whose words or tooltip changed
  // under the pointer is a new place, though nothing above it moved.
  const here = [above, more ?? "", ...answers.map(({ name, title }) => `${name}\t${title}`)].join(
    "\n",
  );
  const { drawn, tooSoon } = useSettledPlace<HTMLLIElement>(here);
  /** An answer that allows something, pressed too soon after the update was drawn or moved,
   *  is not sent: what is under the pointer may not be what the person read. */
  const press = (answer: UpdateAnswer) => {
    if (answer.allows && tooSoon()) {
      setSaid(
        "This update was drawn or moved just now, so nothing was allowed. Read it and press again.",
      );
      return;
    }
    setSaid(undefined);
    answer.press();
  };
  const key = `update:${update.key}`;
  const who = chainSaid(update.chain);
  const about = who === "" ? update.says : `${who}: ${update.says}`;
  return (
    <Stop id={key}>
      <li
        ref={drawn}
        className="inbox-update"
        data-kind={update.kind}
        data-read={update.read || undefined}
        aria-labelledby={`${id}-says`}
        aria-describedby={`${id}-kind`}
      >
        <p className="inbox-says">
          <KindMark kind={kindOfUpdate(update.kind)} />
          {/* Every word here is its source's, drawn as text and never as a control. An update
              whose answer allows is read whole, as an ask is (#1146). One read wears no role:
              its step back (`.inbox-update[data-read]`) is not a rank of the scale (#1920). */}
          <Tooltip tip={about}>
            <span
              id={`${id}-says`}
              className={`ask-says inbox-primary${update.read ? "" : " ui-text-body ui-text-strong"}${answers.some((one) => one.allows) ? "" : " inbox-clamped"}`}
            >
              {about}
            </span>
          </Tooltip>
          <span className="inbox-meta ui-text-meta">
            <span id={`${id}-kind`} className="inbox-source">
              {KIND_SAID[update.kind]}
              {!update.read && <span className="inbox-unread"> · new</span>}
            </span>
            <span className="inbox-meta-dot">{" · "}</span>
            <time dateTime={new Date(update.at * 1000).toISOString()}>
              {timeSaid(update.at * 1000)}
            </time>
          </span>
        </p>
        {more !== undefined && <p className="inbox-more">{more}</p>}
        {answers.length > 0 && (
          <ButtonRow
            className="inbox-answers"
            role="group"
            aria-label={`Answers to ${update.says}`}
          >
            {answers.map((answer, at) => (
              <Stop key={answer.label} id={`${key}:${at}`}>
                <Button
                  // An answer that allows is a grant, drawn as every grant is (ADR 0097 §6).
                  variant={answer.allows ? "grants" : "secondary"}
                  className={answer.allows ? "inbox-answer allows" : "inbox-answer"}
                  aria-label={answer.name}
                  tooltip={answer.title}
                  onPress={() => press(answer)}
                >
                  {answer.label}
                </Button>
              </Stop>
            ))}
          </ButtonRow>
        )}
        <NothingDone said={said} />
        <ButtonRow size="sm" className="inbox-acts">
          {go !== undefined && (
            <Stop id={`${key}:go`}>
              <Button
                size="sm"
                variant="ghost"
                aria-label={`Go to chat ${who}`}
                onPress={go}
                className="inbox-go"
              >
                Go to chat
              </Button>
            </Stop>
          )}
          <Stop id={`${key}:dismiss`}>
            <Button
              size="sm"
              variant="ghost"
              aria-label={`Dismiss: ${about}`}
              tooltip={dismissSays ?? "Put this update away"}
              onPress={dismiss}
              className="inbox-go"
            >
              Dismiss
            </Button>
          </Stop>
        </ButtonRow>
      </li>
    </Stop>
  );
}

/**
 * **Where a click on an ask's notification lands** (#1694, I-7): chat `session`'s group in the
 * Inbox drawn now, brought on screen, its first ask given the keyboard, from which the keys go
 * on as from any ask. Answers whether the chat has a group to land on: its asks may have been
 * answered elsewhere since.
 */
export function landOnGroup(session: number): boolean {
  const group = document.querySelector<HTMLElement>(
    `section.inbox .inbox-chat[data-session="${session}"]`,
  );
  const first = group?.querySelector<HTMLElement>("li");
  if (!group || !first) return false;
  group.scrollIntoView?.({ block: "nearest" });
  first.focus();
  return true;
}

/**
 * **The same landing, through a scope that leaves the chat out** (qw359): a notification is
 * about one ask, and This workspace may not list its chat. So where the group is not drawn and
 * the scope is the workspace's, the scope widens to the project's, as revealing a row a filter
 * hides clears the filter, and the landing is tried again once that is drawn.
 */
export function landOnGroupIn(plane: PlaneId, session: number): void {
  if (landOnGroup(session) || keptInboxScope(plane) !== "workspace") return;
  keepInboxScope(plane, "project");
  requestAnimationFrame(() => landOnGroup(session));
}
