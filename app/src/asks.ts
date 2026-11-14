// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The asks registry, as the window holds it** (#1690, spec #1688): everything that waits on
 * the person in each project, and how each is answered.
 *
 * The core derives a project's list from every source that waits today (`asks_waiting`): a
 * permission prompt held on a chat's hook, a dispatch held for a grant, a host a chat's sandbox
 * refused, a workspace a chat asked the person to remove, a prompt in a harness's own terminal,
 * a chat waiting on the person's reply. It keeps
 * no store, so the window reads the list again whenever one of those may have moved, and an
 * ask leaves it the moment its source stops waiting, wherever it was answered.
 *
 * **Answering goes through the path the ask names** ({@link answerThrough}): the command that
 * source's own Notice answers with, checked and audited as it is there. The registry adds no
 * route of its own. Every one of those commands is the window's alone.
 *
 * **What an ask says is data**: its chain of names, its line, a host. The window draws each as
 * text; none of it is ever markup or a control.
 */
import { useEffect, useMemo, useState } from "react";
import {
  commands,
  type Asking,
  type EntriesOf,
  type GrantLevel,
  type PlaneChanged,
  type Press,
  type RunGrantPress,
  type RunLink,
  type RunNeedLine,
  type RunValue,
  type Shown,
} from "./bindings";
import type { QuestionsHeld } from "./taskQuestions";
import { listen } from "./here";
import { ASKS_CHANGED, answerAsk } from "./permissionAsks";
import { listenForChats } from "./chatEvents";
import { askRemoveWorkspace } from "./removeWorkspaceAsk";
import { askOpenBrowser, OPEN_VIEW, TAKE_OVER } from "./browserNeedsYou";
import { onSandboxCommand, runGrantsMoved, sandboxCommandReturned } from "./sandboxAsked";
import { askOpenProposal } from "./proposedChangeOpen";
import { askReviewPin } from "./proposedPinOpen";
import { proposeOldKeysRemoval } from "./oldHarnessPlugins";
import { askOpenRunnerWrite } from "./runnerWriteOpen";
import { askOpenRunnerDispatch } from "./runnerDispatchOpen";
import { askProfileSignIn, askSettingsLink } from "./settings/links";
import {
  askSigningSetUp,
  setUpForgeOf,
  signAndRetryAll,
  SIGNING_WAITING_CHANGED,
} from "./signingAsked";
import { refusalsAnswered } from "./vaultRefusals";
import { asDrawn, asNamed } from "./projectExpects";
import { concerns, followPlaneChanged, SETTINGS } from "./planeChanged";

/** The event the core sends each time a command's confirmation comes or goes (`poolsignin_confirm`). */
export const POOL_SIGN_IN_CONFIRM = "pool-sign-in-confirm";

/**
 * The events after which a project's asks may read differently: its hooks' asks, a dispatch
 * held for a grant, a block heard. A chat moving on the board is one too, and comes in a batch
 * of chat events (`chatEvents.ts`).
 */
export const ASKS_MAY_HAVE_MOVED = [
  ASKS_CHANGED,
  "dispatch-grant-needed",
  "chat-sandbox-blocked",
  // A vault a chat was refused for its persona is an ask (#5160).
  "chat-vault-refused",
] as const;

/** How long the window waits for a burst of moves to settle before it reads a list again. */
export const SETTLE_MS = 50;

/** How long after a hold's end the list is read again: the proxy gives up on its own clock. */
export const HOLD_SLACK_MS = 1000;

/** Whatever reads the lists again when the window itself moved a source. */
const moved = new Set<(plane: string) => void>();

/**
 * **The window itself answered something `plane` waits on** (#1692): a Notice's Keep blocked,
 * an Allow in the Inbox, a dispatch answered on its tab. Some of those move the core with no
 * event of their own, so the window says so here and every list of `plane`'s asks is read
 * again: an answer in one place clears the ask everywhere it is drawn.
 */
export function asksMoved(plane: string): void {
  for (const reader of moved) reader(plane);
}

/** Calls `reader` each time the window moved a source ([`asksMoved`]); answers its unsubscribe. */
export function onAsksMoved(reader: (plane: string) => void): () => void {
  moved.add(reader);
  return () => {
    moved.delete(reader);
  };
}

/**
 * **The chats of `rows` not snoozed** (#5999, ADR 0121 decision 14): a chat in the board's queue
 * whose every ask Snooze all put away is neither counted nor told to the other windows, whose ✋
 * would count it. One that has asked something new since, or of a project whose list is not read
 * yet, stays.
 */
export function notSnoozed<Row extends { plane: string; session: number }>(
  rows: readonly Row[],
  held: Readonly<Record<string, QuestionsHeld>>,
): Row[] {
  return rows.filter(({ plane, session }) => {
    const list = held[plane];
    if (list === undefined) return true;
    const ofIt = (one: Shown) => one.session === session;
    // Nor one whose every ask a cool-off holds (#6010).
    return (
      list.asks.some(ofIt) || !((list.snoozed ?? []).some(ofIt) || (list.cooling ?? []).some(ofIt))
    );
  });
}

/** The earlier of two times, either of which may be missing. */
const firstOf = (one: number | null, two: number | null): number | null =>
  one === null ? two : two === null ? one : Math.min(one, two);

/** Each project's asks, by its id: the last whole list the core derived for it. */
export function useAsks(planes: readonly string[]): {
  held: Record<string, readonly Shown[]>;
  /** **Each project's asks with its task questions still with their asking chat** (#4564):
   *  what a task question's one state is derived from (`taskQuestions.standingOf`). */
  questions: Record<string, QuestionsHeld>;
  /** Reads `plane`'s list again: after an answer, whatever it answered. */
  reread: (plane: string) => void;
} {
  const [questions, setHeld] = useState<Record<string, QuestionsHeld>>({});
  const held = useMemo(
    () => Object.fromEntries(Object.entries(questions).map(([plane, one]) => [plane, one.asks])),
    [questions],
  );
  const [reader] = useState(() => listReader(setHeld));
  const key = planes.join("\n");
  useEffect(() => {
    const wanted = new Set(key === "" ? [] : key.split("\n"));
    reader.watch(wanted);
    for (const plane of wanted) reader.read(plane);
  }, [key, reader]);
  useEffect(() => {
    // **A project's settings changed on disk** (a pull, a branch switched, a hand's edit): what
    // its personas wait for, and what it expects of this machine, may read differently, and no
    // event of the asks says so (ADR 0121, amended 2026-11-12).
    const stops = (key === "" ? [] : key.split("\n")).map((plane) =>
      followPlaneChanged(
        [plane],
        () => SETTINGS,
        () => reader.soon(plane),
      ),
    );
    return () => {
      for (const stop of stops) stop();
    };
  }, [key, reader]);
  useEffect(() => {
    // Listening again after a cleanup (React's StrictMode runs every effect twice) reads what it
    // watches again, since a read made while it was stopped was dropped.
    reader.start();
    moved.add(reader.soon);
    const stops = ASKS_MAY_HAVE_MOVED.map((event) =>
      listen<{ plane: string }>(event, (said) => {
        const plane = said.payload?.plane;
        if (typeof plane === "string") reader.soon(plane);
      }).catch(() => undefined),
    );
    stops.push(listenForChats("moved", (told) => reader.soon(told.plane)).catch(() => undefined));
    // A command's confirmation came or went (#6000), or the window answered something of a
    // sandbox's, such as what a project expects: it is no one project's word, so each list is
    // read again.
    stops.push(listen(POOL_SIGN_IN_CONFIRM, () => reader.soonAll()).catch(() => undefined));
    const offSandbox = onSandboxCommand(() => reader.soonAll());
    // **What a project expects is read again where its Notice reads it** (#6000): as its
    // settings change on disk (a pull or a hand comes to declare it) and as the window comes
    // back (an Allow in another window). The consent is an ask too, derived from the same state,
    // so a list not read then left its Notice up and the ✋ counting nothing.
    stops.push(
      listen<PlaneChanged>("plane-changed", (said) => {
        if (concerns(said.payload.answers, SETTINGS)) reader.soon(said.payload.plane);
      }).catch(() => undefined),
    );
    const cameBack = () => reader.soonAll();
    window.addEventListener("focus", cameBack);
    return () => {
      window.removeEventListener("focus", cameBack);
      offSandbox();
      moved.delete(reader.soon);
      reader.stop();
      for (const stop of stops) void stop.then((off) => off?.()).catch(() => undefined);
    };
  }, [reader]);
  return { held, questions, reread: reader.read };
}

/** Reads each watched project's list, at once or once a burst has settled. */
function listReader(
  setHeld: (change: (was: Record<string, QuestionsHeld>) => Record<string, QuestionsHeld>) => void,
) {
  let watched = new Set<string>();
  let live = true;
  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  /** The read due when the first hold a project's list names runs out (#1709). */
  const holds = new Map<string, ReturnType<typeof setTimeout>>();
  /**
   * **The newest read a project's list was drawn from** (#5582): the core reads the list off the
   * window's thread, so two reads may answer out of the order they were asked in, and an older
   * answer is never drawn over a newer one.
   */
  const asked = new Map<string, number>();
  const drawn = new Map<string, number>();
  const read = (plane: string) => {
    if (!live || !watched.has(plane)) return;
    const thisRead = (asked.get(plane) ?? 0) + 1;
    asked.set(plane, thisRead);
    void commands.asksWaiting(plane).then(
      (answer) => {
        // A core that answers nothing for it (an older build, a test's mock) holds none.
        const listed = answer.status === "ok" ? (answer.data as Asking | null) : undefined;
        const asks = listed?.asks;
        if (!(live && watched.has(plane) && asks)) return;
        if (thisRead < (drawn.get(plane) ?? 0)) return;
        drawn.set(plane, thisRead);
        // An older core lists no task question with its asking chat: none is.
        const with_asking_chat = listed?.with_asking_chat ?? [];
        // An older core, or a test's mock, lists no snoozed ask (#5647): none is.
        const next = {
          asks,
          with_asking_chat,
          snoozed: listed?.snoozed ?? [],
          snoozed_until: listed?.snoozed_until ?? null,
          // And none a cool-off holds (#6010).
          cooling: listed?.cooling ?? [],
          cooling_until: listed?.cooling_until ?? null,
        };
        // **An answer that says what the last one said changes nothing** (#4727): every burst of
        // a busy chat's moves reads the list again, and a fresh list each time drew the whole
        // window again for asks that had not moved. The core's answer is plain data, written
        // the same way each time, so its text is the comparison.
        setHeld((was) =>
          was[plane] !== undefined && JSON.stringify(was[plane]) === JSON.stringify(next)
            ? was
            : { ...was, [plane]: next },
        );
        lookAtHoldsEnd(plane, asks);
        // A cool-off ends with no event of its own either (#6010): read again at the first end.
        lookAtSnoozesEnd(plane, firstOf(next.snoozed_until, next.cooling_until));
      },
      () => {},
    );
  };
  /** The read due when the first snoozed chat a project's list names comes back (#6018). */
  const snoozes = new Map<string, ReturnType<typeof setTimeout>>();
  /**
   * **A snooze for a while runs out with no event of its own** (#6018, ADR 0121 decision 14):
   * the list is read again when the first snoozed chat it names comes back, and the core lets
   * that chat go, so its asks are counted and listed again then. The time is the core's
   * record's, so it holds across a relaunch.
   */
  const lookAtSnoozesEnd = (plane: string, until: number | null) => {
    clearTimeout(snoozes.get(plane));
    snoozes.delete(plane);
    if (until === null) return;
    snoozes.set(
      plane,
      setTimeout(
        () => {
          snoozes.delete(plane);
          read(plane);
        },
        Math.max(0, until * 1000 - Date.now()),
      ),
    );
  };
  /**
   * **A held connection's hold runs out with no event of its own** (#1709): the list is read
   * again once the first hold it names has ended, so the ask says then that it no longer waits.
   */
  const lookAtHoldsEnd = (plane: string, asks: readonly Shown[]) => {
    clearTimeout(holds.get(plane));
    holds.delete(plane);
    const ends = asks
      .map((ask) => ask.held_until)
      .filter((until): until is number => typeof until === "number");
    if (ends.length === 0) return;
    const wait = Math.max(0, Math.min(...ends) * 1000 + HOLD_SLACK_MS - Date.now());
    holds.set(
      plane,
      setTimeout(() => {
        holds.delete(plane);
        read(plane);
      }, wait),
    );
  };
  const soon = (plane: string) => {
    if (timers.has(plane)) return;
    timers.set(
      plane,
      setTimeout(() => {
        timers.delete(plane);
        read(plane);
      }, SETTLE_MS),
    );
  };
  return {
    read,
    start() {
      if (live) return;
      live = true;
      for (const plane of watched) read(plane);
    },
    soonAll() {
      for (const plane of watched) soon(plane);
    },
    soon,
    watch(planes: Set<string>) {
      watched = planes;
      setHeld((was) => Object.fromEntries(Object.entries(was).filter(([one]) => planes.has(one))));
    },
    stop() {
      live = false;
      for (const timer of [...timers.values(), ...holds.values(), ...snoozes.values()])
        clearTimeout(timer);
      timers.clear();
      holds.clear();
      snoozes.clear();
    },
  };
}

const LEVELS: readonly GrantLevel[] = ["once", "chat", "you", "machine", "project"];
const isLevel = (option: string): option is GrantLevel =>
  (LEVELS as readonly string[]).includes(option);

/**
 * The option of a workspace's removal that answers nothing and opens the window's own Delete
 * workspace (qw356): the core's `asking::REMOVE`.
 */
export const OPENS_DELETE_WORKSPACE = "remove";

/** **Raise the budget** on a chat paused at its budget (#6015): opens Settings › Project ›
 *  Dispatch at its limits, and answers the ask, which leaves the chat paused until a look finds
 *  it within the new limit (Q261). */
export const RAISE_THE_BUDGET = "raise";

/** What is said when an ask names no path the window can send. */
export const IN_ITS_CHAT = "This is answered in its chat: go to it.";

/**
 * **Answers `ask` of `plane` with `option`, through the path it names**: nothing when it
 * applied, and the source's own sentence when it was refused. An option the ask does not
 * offer is refused here, before anything is sent.
 */
export async function answerThrough(
  plane: string,
  ask: Shown,
  option: string,
): Promise<string | undefined> {
  return (await answerTaken(plane, ask, option)).refused;
}

/**
 * **An answer noted against its request** (ADR 0121 decision 7, #6010): what offers a rule after
 * the same Allow twice, and starts a cool-off after Don't allow twice. Every answer is noted, with
 * when its ask was raised, for the week's measures All chats shows (ADR 0121 decision 18, #6021);
 * one the core names no request of counts there alone. A note that fails loses an offer or a
 * count, never an answer.
 */
function notedAgainst(plane: string, ask: Shown, option: string): void {
  const allows = ask.options.some((one) => one.id === option && one.allows);
  void commands
    .askAnswered(plane, ask.session, ask.rule?.request ?? "", option, allows, ask.since ?? null)
    .catch(() => undefined);
}

/** The press a run's notice's option sends (ADR 0112, #5242), by the option's id. */
const RUN_PRESS_OF: Readonly<Record<string, RunGrantPress | undefined>> = {
  "allow-once": "once",
  "allow-chat": "chat",
  "allow-always": "always",
  "allow-vault": "allow-vault",
  fill: "fill",
  keep: "refuse",
};

/** **What the person gave a run's notice besides the press** (#5242, #5080): a value typed or
 *  an entry picked for each field, by key, and the lines Remember writes. */
export type RunGiven = {
  values: RunValue[];
  links: RunLink[];
  remember: RunNeedLine[];
};

/** The press a login notice's option sends (#4595), by the option's id. */
const PRESS_OF: Readonly<Record<string, Press | undefined>> = {
  once: "once",
  chat: "chat",
  you: "always",
  keep: "keep",
};

/**
 * **The entries a Secret ask may be filled with** (#5073): every vault's keys that hold a value,
 * vault by vault, but the asked one. Names only. Refused for an ask that is not a Secret ask.
 */
export async function secretAskEntries(plane: string, ask: Shown): Promise<EntriesOf[]> {
  if (ask.answer.via === "run-grant") {
    // A run's notice's fields (#5080): the same entries, but its own fields'.
    const listed = await commands.runAskEntries(plane, ask.answer.id);
    if (listed.status === "error") throw new Error(listed.error);
    return listed.data;
  }
  if (ask.answer.via !== "secret") throw new Error("only a secret ask is filled with an entry");
  const listed = await commands.secretAskEntries(plane, ask.answer.id);
  if (listed.status === "error") throw new Error(listed.error);
  return listed.data;
}

/**
 * **Whether answering `ask` with `option` opens something in its project's window** (#1921): a
 * tab, a shell or a dialog there, where the person goes on (`answerTaken`'s `opened`). Asks lists
 * every open project's asks, and such an answer is not given from another project's Asks: that
 * project's own does it, where what it opens is drawn. Every other answer is a command to the
 * core, which holds every project of every window, and is given from any project's Asks.
 */
export function opensItsWindow(ask: Shown, option: string): boolean {
  switch (ask.answer.via) {
    case "proposed-change":
    case "runner-write":
    case "runner-dispatch":
      return option === "open";
    case "sign-in":
      return option === "sign-in" || option.startsWith("sign-in:");
    case "signing":
      return option === "set-up" || setUpForgeOf(option) !== undefined;
    case "old-harness-plugins":
      return option === "remove";
    case "sandbox-only":
      return option === "make";
    case "take-over":
      return true;
    default:
      return false;
  }
}

/**
 * {@link answerThrough}, and **whether the chat took the answer live** (#1666): a sandbox
 * host's Allow that the chat's proxy took while the command waited, so the command carries on
 * and nothing restarts. What the block Notice does after an Allow, a window that answers
 * through the registry does by this.
 *
 * `opened` says the option answered nothing: it opened a dialog of the window's where the
 * person decides (a workspace's removal, qw356), so the ask still waits and nothing is to be
 * noted as answered.
 */
export async function answerTaken(
  plane: string,
  ask: Shown,
  option: string,
  reason?: string,
  typed?: string,
  /** The entry a Secret ask is filled with, where the person picked one they keep (#5073). */
  picked?: { vault: string; key: string },
  /** The purpose of the line a Secret ask's Fill writes, where Remember is ticked (#5240). */
  remember?: string,
  /** What the person gave a run's notice (#5242, #5080): a value typed or an entry picked for
   *  each field, and the needs Remember writes. */
  run?: RunGiven,
  /** Where a Secret ask's value is stored, where the person changed it (#5072, Change). */
  named?: { vault?: string; key?: string },
): Promise<{
  refused: string | undefined;
  live: boolean;
  opened?: boolean;
  /** What the source said of the answer, for the person to read beside it: Keep blocked on a
   *  vault the chat's persona already holds says it does not stop the chat's runs (#5890). */
  said?: string;
  /** Whether `said` sends the person to Settings › Sandbox › Network › Allowed, so its Notice links there
   *  (#5939). */
  allowedList?: true;
}> {
  const path = ask.answer;
  if (path.via === "in-its-pane") return { refused: IN_ITS_CHAT, live: false };
  if (!ask.options.some((one) => one.id === option))
    return {
      refused: `${JSON.stringify(option)} is not one of the answers this ask offers.`,
      live: false,
    };
  let live = false;
  let opened = false;
  let said: string | undefined;
  let allowedList = false;
  /** Keeps what a source said of an answer that went through, and answers as it did. */
  const saying = <T extends { said: string; allowed_list: boolean } | null>(
    done: { status: "ok"; data: T } | { status: "error"; error: string },
  ): { status: "ok" } | { status: "error"; error: string } => {
    if (done.status === "error") return done;
    said = done.data?.said;
    allowedList = done.data?.allowed_list === true;
    return { status: "ok" };
  };
  const sent = async (): Promise<{ status: "ok" } | { status: "error"; error: string }> => {
    switch (path.via) {
      case "hook": {
        const refused = await answerAsk(plane, ask.session, ask.ask, option);
        return refused === undefined ? { status: "ok" } : { status: "error", error: refused };
      }
      case "dispatch":
        if (option === "keep") return commands.keepDispatchBlocked(plane, path.id);
        if (option === "never") return commands.neverDispatch(plane, path.id);
        if (isLevel(option)) return commands.allowDispatch(plane, path.id, option, [], path.shown);
        break;
      case "sandbox-block":
        // Keep blocked as the block's Notice keeps it (#1666): for a host, what the chat's
        // proxy holds on it is refused now, so its command waits no more; and the chat is told
        // in one line not to retry, with the person's reason if they gave one (#1987).
        if (option === "keep")
          return commands.keepSandboxBlock(plane, ask.session, path.shown, reason?.trim() || null);
        if (isLevel(option)) {
          const allowed = await commands.allowSandboxBlock(plane, ask.session, path.shown, option);
          if (allowed.status === "ok") live = allowed.data.live === true;
          return allowed;
        }
        break;
      case "proposed-change":
        // Apply writes the change the window showed, as the person; Dismiss writes nothing
        // (#1975). Both tell the chat on its next turn.
        if (option === "apply") return commands.applyProposedChange(plane, path.id, "row");
        // Too long for a row: its own tab shows it whole, and Apply is pressed there (PC-7).
        if (option === "open") {
          askOpenProposal(plane, path.id, path.path);
          opened = true;
          return { status: "ok" };
        }
        // A chat's proposed pin (#5963): Review… opens Pin to workspace, whose Pin alone pins it.
        if (option === "review-pin") {
          askReviewPin(plane, path.id);
          opened = true;
          return { status: "ok" };
        }
        if (option === "dismiss") return commands.dismissProposedChange(plane, path.id);
        break;
      case "runner-write":
        // A chat on a pool's runner asked to change this project (#2046): Apply makes the change
        // as the window would for a chat here; Dismiss makes nothing. The chat is told which.
        // Apply is offered on a row only where the row shows the change whole; otherwise Open
        // shows it whole in its own tab, where Apply is pressed.
        if (option === "apply") return commands.applyRunnerWrite(plane, path.id, "row");
        if (option === "open") {
          askOpenRunnerWrite(plane, path.id);
          opened = true;
          return { status: "ok" };
        }
        if (option === "dismiss") return commands.dismissRunnerWrite(plane, path.id);
        break;
      case "runner-dispatch":
        // A chat on a pool's runner asked to start a chat on this machine. A row cuts its brief,
        // so Open shows it whole in its own tab, where Approve is pressed; Refuse starts
        // nothing. The chat is told which.
        if (option === "open") {
          askOpenRunnerDispatch(plane, path.id);
          opened = true;
          return { status: "ok" };
        }
        if (option === "refuse") return commands.refuseRunnerDispatch(plane, path.id);
        break;
      case "sign-in":
        // Sign in sends nothing (#2035): the project's window opens a shell tab where the core
        // types the harness's own sign-in, to the plan the option names where it names one. The
        // Ask waits until the harness says the profile is signed in. Dismiss puts it away.
        // With a pool (#2049), on that pool's runner: the line typed is the runner's own sign-in.
        if (option === "dismiss")
          return commands.dismissSignIn(plane, path.profile, path.pool ?? null);
        if (option === "sign-in" || option.startsWith("sign-in:")) {
          const provider = option.startsWith("sign-in:") ? option.slice("sign-in:".length) : "";
          askProfileSignIn(
            plane,
            path.profile,
            provider === "" ? undefined : provider,
            path.pool ?? undefined,
          );
          opened = true;
          return { status: "ok" };
        }
        break;
      case "signing": {
        // The lanes a forge refused for unsigned commits (#5633): Set up signing opens the
        // dialog in the project's window, ending in Sign and retry all; once on, Sign and retry
        // all sends each again; once all went, Dismiss puts it away. Lanes on more than one
        // forge are set up a forge at a time, `set-up:<kind>:<host>` (#5790).
        if (option === "set-up" || setUpForgeOf(option) !== undefined) {
          askSigningSetUp(plane, setUpForgeOf(option));
          opened = true;
          return { status: "ok" };
        }
        if (option === "retry-all") {
          const done = await signAndRetryAll(plane);
          return typeof done === "string" ? { status: "error", error: done } : { status: "ok" };
        }
        if (option === "dismiss") {
          const gone = await commands.signingForget(plane, null);
          window.dispatchEvent(new Event(SIGNING_WAITING_CHANGED));
          return gone;
        }
        break;
      }
      case "old-harness-plugins": {
        // The old `[harness_plugins]` keys (#3467): Remove holds purlis's proposed change for
        // each file and opens the first one's tab, where Apply is pressed; nothing is written
        // before. Dismiss says it no more. Either is said once.
        if (option === "dismiss") return commands.dismissOldHarnessPlugins(plane);
        if (option === "remove") {
          const held = await proposeOldKeysRemoval(plane);
          if (held.status === "error") return held;
          opened = true;
          return held;
        }
        break;
      }
      case "sandbox-only": {
        // Make one makes a sandbox-only profile of the Ask's harness (#2037), then the
        // project's window opens a shell tab where the core types that harness's own sign-in
        // for it. Dismiss puts it away.
        if (option === "dismiss") return commands.dismissSandboxOnly(plane, path.harness);
        if (option === "make") {
          const made = await commands.makeSandboxOnlyProfile(plane, path.harness);
          if (made.status === "error") return made;
          askProfileSignIn(plane, made.data);
          opened = true;
          return { status: "ok" };
        }
        break;
      }
      case "browser":
        // What a chat's browser may reach (#2013): this project's sites and its dev servers,
        // also the open web, each for this chat or always for its persona (#2241), or Keep
        // off. The chat is told on its next turn, with the reason the person typed for Keep off.
        if (option === "project")
          return commands.allowChatBrowser(plane, ask.session, false, false);
        if (option === "open-web")
          return commands.allowChatBrowser(plane, ask.session, true, false);
        if (option === "project-always")
          return commands.allowChatBrowser(plane, ask.session, false, true);
        if (option === "open-web-always")
          return commands.allowChatBrowser(plane, ask.session, true, true);
        if (option === "keep")
          return commands.keepChatBrowserOff(plane, ask.session, reason?.trim() || null);
        break;
      case "pool-login":
        // A runner chat's browser login (#2050): Allow lets its persona's logins be filled on the
        // pool's runner while purlis runs and the pool stays as it is; Keep off refuses them there,
        // with the reason the person typed. The chat is told on its next turn.
        if (option === "allow") return commands.answerPoolLogin(plane, ask.session, true, null);
        if (option === "keep")
          return commands.answerPoolLogin(plane, ask.session, false, reason?.trim() || null);
        break;
      case "browser-login": {
        // A login a chat suggested (#4595): the press names what the notice drew, with what the
        // person typed. A site that isn't what they typed is drawn again, for another press.
        const press = PRESS_OF[option];
        if (press === undefined) break;
        const pressed = await commands.answerBrowserLogin(
          plane,
          ask.session,
          path.id,
          press,
          path.shown,
          typed ?? null,
          reason?.trim() || null,
        );
        if (pressed.status === "error") return pressed;
        if (pressed.data.kind === "drawn-again")
          return { status: "error", error: pressed.data.said };
        return { status: "ok" };
      }
      case "secret-fill": {
        // A secret a chat asks purlis to fill (#5115): the press names what the notice drew.
        const press = PRESS_OF[option];
        if (press === undefined) break;
        const pressed = await commands.answerSecretFill(
          plane,
          ask.session,
          path.id,
          press,
          path.shown,
          reason?.trim() || null,
        );
        if (pressed.status === "error") return pressed;
        // Drawn again as the project reads now (#5116): read and choose again.
        if (pressed.data.kind === "drawn-again")
          return { status: "error", error: pressed.data.said };
        return { status: "ok" };
      }
      case "pool-tool":
        // One run of a runner chat's declared tool (#2047): Allow this run has this desktop send
        // the tool its vault values on the pool's runner for that run alone; Keep off refuses it.
        // The command that waits goes on, or is refused, at once.
        if (option === "allow") return commands.answerPoolTool(plane, ask.session, path.run, true);
        if (option === "keep") return commands.answerPoolTool(plane, ask.session, path.run, false);
        break;
      case "run-grant": {
        // A run's one notice (ADR 0112, #5242): a program its persona doesn't declare, the keys
        // it lacks and a vault it may not use. An Allow is a Run grant for that chat, or, where
        // it offers none, the vault's own Allow or Fill; the values typed go with any answer, and
        // the command that waits goes on at once.
        const press = RUN_PRESS_OF[option];
        if (press === undefined) break;
        // An Allow gives a Run grant, and Keep blocked takes the chat's back: the Allowed list
        // reads again either way (#5617).
        // Keep blocked on a vault the persona holds by now says so, as the vault's Notice does.
        return commands
          .answerRunGrant(
            plane,
            path.id,
            press,
            run?.values ?? [],
            run?.links ?? [],
            run?.remember ?? [],
          )
          .then(saying)
          .finally(runGrantsMoved);
      }
      case "secret":
        // A key a chat's brokered run missed (#5068, ADR 0109): Fill stores the value typed in
        // the ask's masked box in the vault, and every command that waits on it runs; Not now
        // tells them it was declined. The chat never hears the value.
        // An entry the person picked makes the key a Linked entry reading it, never a copy
        // (#5073); the chat hears filled and the key's name, never the entry.
        // Remember, ticked, writes the line the person read once the value is stored (#5240).
        if (option === "fill" && picked !== undefined)
          return commands.linkSecretAsk(plane, path.id, picked.vault, picked.key, remember ?? null);
        if (option === "fill")
          return commands.answerSecretAsk(
            plane,
            path.id,
            typed ?? "",
            named?.vault ?? null,
            named?.key ?? null,
            remember ?? null,
          );
        if (option === "keep")
          return commands.answerSecretAsk(plane, path.id, null, null, null, null);
        break;
      case "vault":
        // A vault a chat was refused for its persona (#1430, #5160): its Notice's own two
        // commands. Allow lets that persona use the vault in this project on this machine, the
        // one grant the Notice makes; Keep blocked tells the chat not to retry, with the
        // person's reason.
        // Either way, the chat's own Notice reads what the core holds then.
        if (option === "allow")
          return commands
            .allowRefusedVault(plane, ask.session, path.vault)
            .finally(() => refusalsAnswered(plane));
        // Keep blocked also takes back the chat's Run grants on the vault: the Allowed list reads
        // again (#5617).
        if (option === "keep")
          return commands
            .keepVaultBlocked(plane, ask.session, path.vault, reason?.trim() || null)
            .then(saying)
            .finally(() => {
              refusalsAnswered(plane);
              runGrantsMoved();
            });
        break;
      case "take-over": {
        // A browser whose page needs the person (#2015): Take over takes the session host's
        // lease first, so their first click or key reaches the page, then opens its view. The
        // Ask waits on, until their Done in that view hands the browser back. Once they drive
        // it, its one answer opens the view again and sends nothing.
        if (option === OPEN_VIEW) {
          askOpenBrowser(plane, path.program);
          opened = true;
          return { status: "ok" };
        }
        if (option !== TAKE_OVER) break;
        const taken = await commands.takeOverBrowser(path.program);
        if (taken.status === "error") return taken;
        askOpenBrowser(plane, path.program);
        opened = true;
        return { status: "ok" };
      }
      case "browser-sites":
        // Sites new to the project a chat's browser held back (#2018): an Allow is a browser
        // site, for this chat, always here, or for everyone in the project (#2443), for the
        // browser alone; Keep blocked tells the chat not to retry, with the reason the person
        // typed. An Allow is told to what reads a chat's network, so its Network view lists the
        // site apart from then on (#2364).
        if (option === "chat" || option === "you" || option === "project") {
          const allowed = await commands.allowBrowserSites(plane, ask.session, path.sites, option);
          if (allowed.status === "ok") sandboxCommandReturned();
          return allowed;
        }
        if (option === "keep")
          return commands.keepBrowserSitesBlocked(
            plane,
            ask.session,
            path.sites,
            reason?.trim() || null,
          );
        break;
      case "sandbox-expects": {
        // What the project expects of this machine (#6000), answered here alone while it can be
        // (R-205): the window's two commands, sent the items as the row drew them. Either way
        // Settings › Sandbox reads again.
        const answered =
          option === "allow"
            ? commands.allowProjectExpects(plane, asDrawn(path.items))
            : option === "not-now"
              ? commands.declineProjectExpects(plane, asNamed(path.items))
              : undefined;
        if (answered === undefined) break;
        const done = await answered;
        if (done.status === "ok") sandboxCommandReturned();
        return done.status === "ok" ? { status: "ok" } : done;
      }
      case "persona-allow": {
        // A persona's hosts and tools waiting for an Allow here (ADR 0121, amended 2026-11-12):
        // its Notice's own Allow, sent the digest the row showed, so a list changed since is
        // refused and asks anew; Not now puts it away while the project is open. Either way its
        // Notice, drawn from this same ask, goes with it.
        const answered =
          option === "allow"
            ? commands.allowPersonaHosts(plane, path.persona, path.digest)
            : option === "not-now"
              ? commands.personaAllowNotNow(plane, path.persona, path.digest)
              : undefined;
        if (answered === undefined) break;
        const done = await answered;
        if (done.status === "ok" && option === "allow") sandboxCommandReturned();
        return done.status === "ok" ? { status: "ok" } : done;
      }
      case "paused":
        // A chat paused at its budget or as looping (#6015): Raise the budget opens Settings where
        // the limit is and answers the ask, and the chat goes on once a look finds it within the
        // new limit (Q261). Continue and It's fine let it go on, Stop stops it.
        if (option === RAISE_THE_BUDGET) {
          askSettingsLink(plane, { group: "project.dispatch", setting: "project.dispatch.limits" });
          opened = true;
        }
        return commands.answerPause(plane, ask.session, option);
      case "confirmation":
        // A command's confirmation in the window (#6000), answered here alone (#6136). The
        // command goes on, or stops, at once.
        if (option !== "yes" && option !== "no") break;
        await commands.answerPoolSignIn(path.id, option === "yes");
        return { status: "ok" };
      case "merge-hold":
        // A merge a chat's hook held (R-115): Merge is the app's own forge call, at the head the
        // ask was raised at; Don't merge merges nothing. The chat never merges.
        if (option === "merge")
          // It hands back the ruleset state its row showed, which the press is bound to (R-150).
          return commands.mergeHeldRequest(
            plane,
            ask.session,
            path.repo,
            path.number,
            path.head,
            path.facts.ruleset,
          );
        if (option === "dont-merge")
          return commands.dontMergeHeldRequest(
            plane,
            ask.session,
            path.repo,
            path.number,
            path.head,
          );
        break;
      case "trust-clone": {
        // A refusal whose only way out is Trust this clone (R-328, R-330): the clone Notice's own
        // press, at the address the refusal was raised at. It clears the ask, and the chat's next
        // GitHub App token is minted.
        if (option !== "press") break;
        const trusted = await commands.vouchClone(plane, path.folder, path.place);
        return trusted.status === "error" ? trusted : { status: "ok" };
      }
      case "trust-clones": {
        // The clones purlis has no record of, one ask for all of them (R-336): Trust all is the
        // clone Notice's own press on each, at the one address its remote names. Each clone it
        // trusts leaves the ask; one it can't is said, and the rest are still trusted.
        if (option !== "trust-all") break;
        const refused: string[] = [];
        for (const clone of path.clones) {
          if (clone.place === null) {
            refused.push(
              `${clone.folder}'s remote names more than one address, so purlis didn't trust it. Give origin one address, then trust it.`,
            );
            continue;
          }
          const trusted = await commands.vouchClone(plane, clone.folder, clone.place);
          if (trusted.status === "error") refused.push(trusted.error);
        }
        return refused.length === 0
          ? { status: "ok" }
          : { status: "error", error: refused.join(" ") };
      }
      case "workspace-removal":
        // Keep puts the question away. Remove… removes nothing (qw356): it opens the window's
        // own Delete workspace, where what goes is read and the person's press there decides.
        // The question waits until they do, or keep it.
        if (option === "keep")
          return commands.keepAskedWorkspace(plane, ask.session, path.workspace);
        if (option === OPENS_DELETE_WORKSPACE) {
          askRemoveWorkspace(plane, path.workspace);
          opened = true;
          return { status: "ok" };
        }
        break;
    }
    return { status: "error", error: `${JSON.stringify(option)} is no answer purlis sends.` };
  };
  const answer = await sent().catch((err: unknown) => ({
    status: "error" as const,
    error: String(err),
  }));
  if (answer.status === "ok" && !opened) notedAgainst(plane, ask, option);
  return answer.status === "ok"
    ? {
        refused: undefined,
        live,
        ...(opened ? { opened } : {}),
        ...(said === undefined ? {} : { said }),
        ...(said !== undefined && allowedList ? { allowedList: true as const } : {}),
      }
    : { refused: answer.error, live: false };
}
