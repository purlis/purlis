// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { ALLOWED_LIST, askSettingsLink, OPEN_ALLOWED_LIST } from "./settings/links";
import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { LoaderCircle } from "lucide-react";
import { type Offered, type PlaneId, type Shown } from "./bindings";
import {
  ElsewhereGroup,
  elsewhereKey,
  elsewherePlaces,
  herePlaces,
  OtherProjects,
  othersWaiting,
  QuietNotices,
  type Elsewhere,
} from "./InboxElsewhere";
import { useNow } from "./InboxRow";
import {
  INBOX_SCOPES,
  keepInboxScope,
  scopeDrawn,
  scopeOf,
  useInboxScope,
  widerWaiting,
  type ScopeCounts,
} from "./inboxScope";
import { quietSaid } from "./NeedsYou";
import { Notice, NoticeList, type NoticesClear } from "./Notice";
import {
  DISMISS_ALL,
  DISMISS_ALL_TIP,
  DismissUndo,
  holdOrCarryOut,
  useDismissHold,
  type DismissHold,
} from "./dismissHold";
import { PersonaMark } from "./PersonaMark";
import {
  askKey,
  byChat,
  dismissSaid,
  MOST_ANSWERED,
  noteSeen,
  ageOf,
  useAnswered,
  useAnsweredWhenGone,
} from "./inboxRules";
import { listedFrom, type UpdateFrom } from "./inboxUpdates";
import { useAllowedLately } from "./allowedLately";
import { NoticeWayGuard, useNoticeSettle } from "./noticeSettle";
import { useProjectShown } from "./projectShown";
import { ScopeSwitch } from "./ScopeSwitch";
import { Button } from "./ui/button/Button";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { RESULTS } from "./ActionResultNotice";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { List, ListRowSection } from "./ui/list/List";
import { SnoozeAll, type Snoozing } from "./AsksSnooze";

import { chainSaid, NothingDone } from "./inboxRowParts";
import { capMarks } from "./combiningMarks";
import { ASKS, Ask, Empty, EmptyScope, placeIds, SCOPE_LABEL, shapeOf } from "./InboxAsks";
import { heldBy, UpdateList, type UpdateRow, type Updates, updatePlaces } from "./InboxUpdatesList";
import { SeenRowsOf, type SeenRows } from "./seenRows";
export { chainSaid } from "./inboxRowParts";
export { grants, NOTICE_MOVED_JUST_NOW } from "./noticeSettle";

// The views' rows live in their own modules; what they exported is still read from here.
export {
  ALLOW_OR_KEEP,
  ALLOWED_LATELY,
  ASKS,
  ASK_FOLD_LINES,
  hiddenSaid,
  landOnGroup,
  landOnGroupIn,
  linesSaid,
  MOST_LINES,
  MOVED_JUST_NOW,
  MOVED_ON,
  NOT_CHECKED,
  REPLY_HINT,
  SCOPE_LABEL,
  NO_MESSAGE,
  NOTHING_WAITS,
} from "./InboxAsks";
export {
  heldUpdate,
  type UpdateAnswer,
  type UpdateRow,
  UPDATES,
  type Updates,
} from "./InboxUpdatesList";

/** What an Inbox says before its project's asks were read. */
export const NOT_READ_YET = "Reading what waits on you…";

/** What Updates' two lists are called, on its switch and where one is empty (#5646). */
export const FROM_LABEL = "Show updates from";
export const FROM_LISTS: readonly { scope: UpdateFrom; says: string; name: string }[] = [
  { scope: "mine", says: "Mine", name: "Mine, from your chats" },
  { scope: "project", says: "Project", name: "Project, from the project" },
];
export const NOTHING_FROM: Record<UpdateFrom, string> = {
  mine: "Nothing new from your chats",
  project: "Nothing new from the project",
};

/** What Updates' scope switch is called (#5646). */
export const UPDATES_SCOPE_LABEL = "Show updates in";

/** What an empty Notices view says (#5645): nothing stands wrong in this project. */
export const NOTHING_STANDS = "No Notices";

/** What the Notices icon's dot says (#5645): a Notice listed says something needs fixing. */
export const NEEDS_FIXING = "A Notice needs fixing";

/** What the Notices' section is called (#1695). */
export const NOTICES = "Notices";

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
 * Every ask has **Go to chat**, which for a task opens it in the pane it returns to (`onGo`).
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
 * this window knows of (the default, ADR 0121 decision 4), this project's or those of the
 * workspace in front on the strip, each with its count, the pick kept for the project as the
 * Chats view's is. Only the scopes the window can list are offered. A scope with nothing in it
 * says how many wait in the nearest wider one, with one press to widen. Notices and updates are
 * the project's and are listed whatever the scope.
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
  part,
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
  project: fromTheProject,
  notices,
  reading,
  windowLines,
  onNotices,
  onTrouble,
  onNoticeCauses,
  seen,
  onNoticeAnswer,
  elsewhere,
  here,
  workspaceOf,
  workspaceNamed,
  snoozing,
  hold,
}: {
  /**
   * **Which view of the right bar this draws** (ADR 0119, #5644, #5645): `asks`, the asks
   * grouped by chat with the scope switch and what waits in other windows; `updates`, the
   * updates; or `notices`, the project's Notices, the window's own lines and Results, in the
   * Panel's right half. Left out, all three, one after the other, as one list.
   */
  part?: "asks" | "updates" | "notices";
  plane: PlaneId;
  /** The project's asks, as the registry derived them last; nothing before the first read. */
  asks: readonly Shown[] | undefined;
  /** The chat that asked, in front: a task in the pane it returns to. */
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
  /**
   * **What teammates changed in the project** (#5646, ADR 0119): the teammate Notices (its
   * hosts, its presets, its dispatch grants), listed under Updates' Project and never counted.
   */
  project?: ReactNode;
  /** The project's Notices (#1695): every `Notice` drawn in it is listed here, in its order. */
  notices?: ReactNode;
  /** What is still being read that may list a Notice here, in words, one line each (#5325). */
  reading?: readonly string[];
  /** How many Notices the list holds that the status line counts (`countsInStatusBar`). */
  onNotices?: (count: number) => void;
  /** How many Notices the list holds that say something needs fixing (#5645). */
  onTrouble?: (count: number) => void;
  /** Hears the causes of the Notices it lists (#6002): what of them is New. */
  onNoticeCauses?: (causes: readonly string[]) => void;
  /** The project's seen record (`seen.ts`), which its rows read to draw what is New (#6002). */
  seen?: SeenRows;
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
  /** **Snooze all** (#5647): the asks it put away, and how to snooze and bring them back.
   *  Without it, Asks offers no Snooze all. */
  snoozing?: Snoozing;
  /** **Dismiss and Dismiss all wait for their Undo here** (#5650): Updates' lines and the
   *  Notices taken away at once, each one's own Dismiss carried out when the Undo goes. Without
   *  it, a Dismiss is carried out at once and no Undo stands. */
  hold?: DismissHold;
}) {
  const drawsAsks = part === undefined || part === "asks";
  const drawsUpdates = part === undefined || part === "updates";
  const drawsNotices = part === undefined || part === "notices";
  if (asks !== undefined && drawsAsks) noteSeen(plane, asks);
  /** How many lines the window lists in this Inbox, and where it draws them. */
  const windowLinesCount = windowLines?.count ?? 0;
  // **Only while this project is in front** (`projectShown.tsx`). The window has one list and
  // draws it in the place it was last handed; a project kept behind another keeps what it drew,
  // so it gives its place up as it leaves the front and hands it over again as it comes back.
  const shown = useProjectShown();
  const placeWindowLines = shown ? windowLines?.at : undefined;
  // The results the window keeps (#3344), under the Notices, in the same place-giving way.
  const placeResults = shown ? windowLines?.resultsAt : undefined;
  const resultsCount = windowLines?.results ?? 0;
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
  /**
   * **An ask's Ignore waits for its Undo** (story 54), as a Dismiss does: the ask goes at once
   * and the queue's Ignore is pressed only when the Undo goes, or the view does.
   */
  const ignoring = useDismissHold();
  const unheld = asks?.filter((ask) => !ignoring.held.has(askKey(ask)));
  const inHere = (ask: Shown) => workspaceOf?.(ask.session) === here;
  const counts: ScopeCounts = {
    project: unheld?.length ?? 0,
    ...(here !== undefined && workspaceOf !== undefined
      ? { workspace: (unheld ?? []).filter(inHere).length }
      : {}),
    ...(elsewhere !== undefined
      ? {
          all:
            (unheld?.length ?? 0) + asking.length + others.reduce((sum, one) => sum + one.count, 0),
        }
      : {}),
  };
  const scope = scopeDrawn(picked, counts);
  /**
   * **Updates' two lists** (#5646, ADR 0119): Mine, what the person's own chats and tasks did,
   * which it opens on; or Project, what is the project's. Narrowed by the scope switch as the
   * asks are, where the window says which workspace is in front: this workspace's are the lines
   * of the chats filed there. The whole Inbox (no `part`) lists every update, as it did.
   */
  const [from, setFrom] = useState<UpdateFrom>("mine");
  const ofList =
    part === "updates"
      ? (updates?.rows ?? []).filter((row) => listedFrom(row.update) === from)
      : (updates?.rows ?? []);
  const updateInHere = (row: UpdateRow) =>
    row.update.session !== null && workspaceOf?.(row.update.session) === here;
  const updateCounts: ScopeCounts = {
    project: ofList.length,
    ...(here !== undefined && workspaceOf !== undefined
      ? { workspace: ofList.filter(updateInHere).length }
      : {}),
  };
  const updateScope = scopeDrawn(picked, updateCounts);
  /** How many teammate Notices Project lists now, and what Dismiss all does to them. */
  const [teammates, setTeammates] = useState(0);
  const [teammatesClear, setTeammatesClear] = useState<NoticesClear>();
  const shownRows =
    updates?.rows === undefined
      ? undefined
      : part !== "updates"
        ? updates.rows
        : updateScope === "workspace"
          ? ofList.filter(updateInHere)
          : ofList;
  const updatesShown: Updates | undefined =
    updates === undefined || part !== "updates"
      ? updates
      : heldBy(hold, updates, shownRows, from === "project" ? teammatesClear : undefined);
  const listed = unheld === undefined ? [] : scope === "workspace" ? unheld.filter(inHere) : unheld;
  /** What waits in other projects is listed among every project's alone. */
  const askingShown = scope === "all" ? asking : [];
  const othersShown = scope === "all" ? others : [];
  /** This project's name, as its tab says it: said on a row only among other projects'. */
  const project = elsewhere?.waitingHere?.find((one) => one.plane === plane)?.project;
  const whereOf = (session: number): string => {
    // Session 0 is no chat's (a change purlis holds, a sign-in): it is filed nowhere (#5240).
    const filed = session === 0 ? undefined : workspaceOf?.(session);
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
    drawsAsks &&
      asks !== undefined &&
      asks.length === 0 &&
      asking.length === 0 &&
      others.length === 0,
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
  const ignore =
    onIgnore === undefined
      ? undefined
      : (ask: Shown) => ignoring.hold([askKey(ask)], () => onIgnore(ask.session));
  /** What the Undo of an Ignore says: whose ask went. */
  const ignoredSaid = () =>
    `Ignored ${(asks ?? [])
      .filter((ask) => ignoring.held.has(askKey(ask)))
      .map((ask) => chainSaid(ask.chain))
      .join(", ")}.`;
  const shapeOfAsk = (ask: Shown) => shapeOf(ask, whyOf, onIgnore, asked);
  /** **The asks whose Answer is open** (#5651), by key: their forms are drawn, and stops. */
  const [answering, setAnswering] = useState<ReadonlySet<string>>(() => new Set());
  const answerOpen = (key: string, open: boolean) =>
    setAnswering((was) => {
      const next = new Set(was);
      if (open) next.add(key);
      else next.delete(key);
      return next;
    });
  const shapes = new Map(
    rows.flatMap((group) => group.asks).map((ask) => [askKey(ask), shapeOfAsk(ask)]),
  );
  // **What is drawn above each row, in order** (#1693, #1695): an ask's links and answers, a
  // chat elsewhere's, an update's. A row stands under what is above it, and an Allow on it waits
  // to settle whenever that changes (`useSettledPlace`), whatever draws the rows.
  // Not read yet, this project draws no ask: what waits elsewhere is still listed (qw359).
  const askRows = drawsAsks && asks !== undefined ? rows : [];
  const places = [
    ...askRows
      .flatMap((group) => group.asks)
      .flatMap((ask) => {
        const shape = shapes.get(askKey(ask)) ?? shapeOfAsk(ask);
        const key = askKey(ask);
        return placeIds(key, shape, shape.answers ? shape.offers : [], answering.has(key));
      }),
    ...(drawsAsks ? elsewherePlaces(askingShown) : []),
    ...(drawsAsks ? herePlaces(othersShown) : []),
    ...(drawsUpdates ? updatePlaces(updatesShown) : []),
  ];
  /** Whether Asks lists any row: an ask of this project's, a chat elsewhere, another project. */
  const anyAsk =
    askRows.length > 0 ||
    (elsewhere !== undefined && askingShown.length > 0) ||
    (elsewhere?.onOpen !== undefined && othersShown.length > 0);

  const leave = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    onLeave();
  };

  /**
   * **The keyboard stays in the list after an answer** (#1700): an ask answered here leaves
   * the list, and the button the keyboard was on goes with it. Focus on an element that goes is
   * focus on the page, where the next key does nothing, so it comes back to the first row: the
   * row itself, never one of its answers.
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
    listAt.current?.querySelector<HTMLElement>('[role="row"][data-key]')?.focus();
  });

  const [noticed, setNoticed] = useState(0);
  /** What Dismiss all on the Notices does now (#5650): none while no Notice has a Dismiss. */
  const [noticesClear, setNoticesClear] = useState<NoticesClear>();
  const {
    at: noticesAt,
    allows: noticesAllow,
    said: noticesSaid,
  } = useNoticeSettle<HTMLElement>("moves");
  const counted = useCallback(
    (count: number, listed: number, trouble: number) => {
      // The section is drawn while it lists anything, counted or not: a doctor finding alone
      // is still a Notice to read here.
      setNoticed(listed);
      onNotices?.(count);
      onTrouble?.(trouble);
    },
    [onNotices, onTrouble],
  );

  const drawn = (
    // A part is named by its view's tab panel (#3330), so only the whole list names itself.
    <section
      className="inbox"
      data-view={part ?? "inbox"}
      aria-label={part === undefined ? "Inbox" : undefined}
    >
      {/* Drawn while any scope lists something, as the Chats view's is: nothing to narrow
          otherwise, and an empty Inbox is its one sentence. */}
      {drawsAsks &&
        Object.keys(counts).length > 1 &&
        Object.values(counts).some((count) => count > 0) && (
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
      {/* **Updates' scope, and its two lists** (#5646): the scope switch as Asks draws it, then
          Mine or Project, Mine first. */}
      {part === "updates" &&
        Object.keys(updateCounts).length > 1 &&
        Object.values(updateCounts).some((count) => count > 0) && (
          <div className="inbox-scope">
            <ScopeSwitch
              label={UPDATES_SCOPE_LABEL}
              scopes={INBOX_SCOPES.flatMap((one) => {
                const count = updateCounts[one.scope];
                return count === undefined
                  ? []
                  : [
                      {
                        ...one,
                        count,
                        name: `${one.says}, ${count} ${count === 1 ? "update" : "updates"}`,
                      },
                    ];
              })}
              scope={updateScope}
              onPick={(to) => keepInboxScope(plane, to)}
            />
          </div>
        )}
      {part === "updates" && (
        <div className="inbox-scope">
          <ScopeSwitch label={FROM_LABEL} scopes={FROM_LISTS} scope={from} onPick={setFrom} />
        </div>
      )}
      {drawsAsks && snoozing !== undefined && asks !== undefined && (
        <SnoozeAll listed={listed} snoozing={snoozing} />
      )}
      {drawsAsks && <DismissUndo hold={ignoring} cause="ask-ignored" said={ignoredSaid} />}
      {/* What was just dismissed, while its Undo stands (#5650). */}
      {(part === "updates" || part === "notices") && <DismissUndo hold={hold} />}
      {/* **Asks and Updates are the list family's** (#3123, ruling B): ↑ and ↓ move between the
          rows, ← and → through a row's own links and answers, a reply box keeps its own keys,
          and typing finds no row (`ui/list/rowKeys.ts`). Each is one Tab stop. */}
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
        {/* **What an answer said that the person must read** (#5890): Keep blocked on a vault
            the chat's persona already holds does not stop its runs, and says where to. Above
            whatever waits, until dismissed, since the row it was pressed on has gone. Drawn with the
            asks, under every scope filter, on purpose: it names its chat, and the person who
            pressed must read it. */}
        {drawsAsks && recent.some((one) => one.said !== undefined) && (
          <NoticeList>
            {recent.map((one) =>
              one.said === undefined ? null : (
                <Notice
                  key={`${one.at}:${chainSaid(one.chain)}:${one.says}`}
                  cause={`answer-said:${one.at}:${chainSaid(one.chain)}:${one.says}`}
                  // As the refused vault's own Notice says the same sentence: news.
                  tone="news"
                  // Where the sentence sends the person to the Allowed list, a press goes
                  // there (#5939); Dismiss stays beside it.
                  link={
                    one.allowedList === true
                      ? {
                          label: OPEN_ALLOWED_LIST,
                          onPress: () => askSettingsLink(plane, ALLOWED_LIST),
                        }
                      : undefined
                  }
                  onDismiss={() => dismissSaid(plane, one)}
                >
                  <strong>{chainSaid(one.chain)}</strong>
                  {": "}
                  {capMarks(one.said).text}
                </Notice>
              ),
            )}
          </NoticeList>
        )}
        {!drawsAsks ? null : asks === undefined ? (
          // Not read yet is not empty; what the window already knows waits wider is offered.
          <EmptyScope said={NOT_READ_YET} wider={wider} onWiden={widen} />
        ) : rows.length === 0 && askingShown.length === 0 && othersShown.length === 0 ? (
          wider === undefined ? (
            <Empty
              recent={recent.slice(0, MOST_ANSWERED)}
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
        ) : null}
        {drawsAsks && anyAsk && (
          <List aria-label={ASKS} className="inbox-rows" disallowTypeAhead>
            {askRows.map((group) => (
              <ListRowSection
                key={group.session}
                id={`chat:${group.session}`}
                className="inbox-chat"
                title={
                  <span className="inbox-chain">
                    {(() => {
                      const persona = personaOf?.(group.session);
                      return persona ? <PersonaMark persona={persona} /> : null;
                    })()}
                    {chainSaid(group.chain)}
                  </span>
                }
                named
                data-session={group.session}
              >
                {group.asks.map((ask) => (
                  <Ask
                    key={askKey(ask)}
                    plane={plane}
                    ask={ask}
                    dispatches={(asks ?? []).filter(
                      (one) => one.session === ask.session && one.source === "dispatch",
                    )}
                    shape={shapes.get(askKey(ask)) ?? shapeOfAsk(ask)}
                    open={answering.has(askKey(ask))}
                    onOpen={(open) => answerOpen(askKey(ask), open)}
                    where={whereOf(ask.session)}
                    now={now}
                    onGo={() => onGo(ask.session)}
                    onAnswered={onAnswered}
                    onIgnore={ignore === undefined ? undefined : () => ignore(ask)}
                    onTouched={(answer) => touch(ask, answer)}
                    above={places.slice(0, places.indexOf(askKey(ask))).join("\n")}
                  />
                ))}
              </ListRowSection>
            ))}
            {elsewhere !== undefined &&
              askingShown.map((item) => (
                <ElsewhereGroup key={elsewhereKey(item)} item={item} onPress={elsewhere.onPress} />
              ))}
            {elsewhere?.onOpen !== undefined && othersShown.length > 0 && (
              <OtherProjects others={othersShown} onOpen={elsewhere.onOpen} now={now} />
            )}
          </List>
        )}
        {/* **A chat that cannot say it waits is the ✋'s** (#1695, FROM_THE_HAND): in Asks,
              where the hand lands, after what waits elsewhere (#5644). Never counted on the
              status line, so the Notices' count is the same wherever it is listed. */}
        {part === "asks" && elsewhere !== undefined && quiet.length > 0 && (
          <NoticeList>
            <QuietNotices plane={plane} quiet={quiet} onPress={elsewhere.onPress} />
          </NoticeList>
        )}
        {drawsNotices &&
          (notices !== undefined ||
            (part === undefined && quiet.length > 0) ||
            windowLines !== undefined) && (
            <section
              className="inbox-notices"
              aria-label={NOTICES}
              hidden={
                noticed === 0 &&
                windowLinesCount === 0 &&
                resultsCount === 0 &&
                (reading === undefined || reading.length === 0)
              }
              ref={noticesAt}
            >
              <div className="inbox-updates-head">
                <PanelSectionTitle level={3} className="inbox-chain">
                  {NOTICES}
                </PanelSectionTitle>
                {/* **Dismiss all** (#5650): each listed Notice's own Dismiss, the Notices
                      without one left standing; an Undo stands where the view holds it. */}
                {part === "notices" && noticesClear !== undefined && (
                  <Button
                    size="sm"
                    variant="ghost"
                    tooltip={DISMISS_ALL_TIP}
                    onPress={() => {
                      const { keys, out } = noticesClear.take();
                      holdOrCarryOut(hold, keys, out);
                    }}
                    className="inbox-go"
                  >
                    {DISMISS_ALL}
                  </Button>
                )}
              </div>
              <NothingDone said={noticesSaid} />
              {/* What may yet list a Notice here, each with its own mark (#5325): not read yet
                  is not nothing to say. */}
              {reading?.map((said) => (
                <p key={said} className="pending" aria-busy="true">
                  <LoaderCircle className="node-icon spinning" aria-hidden="true" />
                  {said}
                </p>
              ))}
              {/* The window's own lines, listed first: the window draws its one list here. */}
              {placeWindowLines !== undefined && (
                <div className="inbox-window-lines" ref={placeWindowLines} />
              )}
              <NoticeWayGuard value={noticesAllow}>
                <NoticeList
                  onCount={counted}
                  onCauses={onNoticeCauses}
                  onAnswer={onNoticeAnswer}
                  hold={part === "notices" ? hold : undefined}
                  onClear={setNoticesClear}
                >
                  {notices}
                  {elsewhere !== undefined && part === undefined && (
                    <QuietNotices plane={plane} quiet={quiet} onPress={elsewhere.onPress} />
                  )}
                </NoticeList>
              </NoticeWayGuard>
              {/* **Results** (#3344): what the person's actions answered, newest first, each the
              notice that stood at the top of the content, with the same ways on. */}
              {placeResults !== undefined && (
                <section aria-label={RESULTS} hidden={resultsCount === 0}>
                  <PanelSectionTitle level={3} className="inbox-chain">
                    {RESULTS}
                  </PanelSectionTitle>
                  <div className="inbox-results" ref={placeResults} />
                </section>
              )}
            </section>
          )}
        {/* The teammate Notices, under Project alone (#5646): never counted, so no `onCount`
              reaches the status line. */}
        {drawsUpdates && updatesShown !== undefined && (
          <UpdateList updates={updatesShown} places={places}>
            {part === "updates" && from === "project" && fromTheProject !== undefined && (
              <NoticeList
                onCount={(_, listed) => setTeammates(listed)}
                hold={hold}
                onClear={setTeammatesClear}
              >
                {fromTheProject}
              </NoticeList>
            )}
          </UpdateList>
        )}
        {part === "updates" &&
          (updatesShown?.rows ?? []).length === 0 &&
          (from === "mine" || fromTheProject === undefined || teammates === 0) && (
            <EmptyState headline={NOTHING_FROM[from]} />
          )}
        {part === "notices" &&
          noticed === 0 &&
          windowLinesCount === 0 &&
          resultsCount === 0 &&
          (reading === undefined || reading.length === 0) && (
            <EmptyState headline={NOTHING_STANDS} />
          )}
      </div>
    </section>
  );
  // **What its rows read of what is New** (#6002): the project's seen record, where it is given.
  return seen === undefined ? (
    drawn
  ) : (
    <SeenRowsOf.Provider value={seen}>{drawn}</SeenRowsOf.Provider>
  );
}

/** Where the window's own lines are drawn in a project's Inbox, and how many there are. */
export type WindowLines = {
  /** Handed the element the window's list is drawn into, and `null` once it goes. */
  at: (element: HTMLDivElement | null) => void;
  /** How many lines the window lists now. */
  count: number;
  /** Handed the element the window's kept results are drawn into, under the Notices (#3344). */
  resultsAt?: (element: HTMLDivElement | null) => void;
  /** How many results the window keeps for this project. */
  results?: number;
};
