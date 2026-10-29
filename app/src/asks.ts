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
import { useEffect, useState } from "react";
import { commands, type Asking, type GrantLevel, type Shown } from "./bindings";
import { listen } from "./here";
import { ASKS_CHANGED, answerAsk } from "./permissionAsks";
import { askRemoveWorkspace } from "./removeWorkspaceAsk";
import { askOpenBrowser, OPEN_VIEW, TAKE_OVER } from "./browserNeedsYou";
import { sandboxCommandReturned } from "./sandboxAsked";
import { askOpenProposal } from "./proposedChangeOpen";
import { proposeOldKeysRemoval } from "./oldHarnessPlugins";
import { askOpenRunnerWrite } from "./runnerWriteOpen";
import { askProfileSignIn } from "./settings/links";

/**
 * The events after which a project's asks may read differently: its hooks' asks, a chat
 * moving on the board, a dispatch held for a grant, a block heard.
 */
export const ASKS_MAY_HAVE_MOVED = [
  ASKS_CHANGED,
  "chat-moved",
  "dispatch-grant-needed",
  "chat-sandbox-blocked",
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

/** Each project's asks, by its id: the last whole list the core derived for it. */
export function useAsks(planes: readonly string[]): {
  held: Record<string, readonly Shown[]>;
  /** Reads `plane`'s list again: after an answer, whatever it answered. */
  reread: (plane: string) => void;
} {
  const [held, setHeld] = useState<Record<string, readonly Shown[]>>({});
  const [reader] = useState(() => listReader(setHeld));
  const key = planes.join("\n");
  useEffect(() => {
    const wanted = new Set(key === "" ? [] : key.split("\n"));
    reader.watch(wanted);
    for (const plane of wanted) reader.read(plane);
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
    return () => {
      moved.delete(reader.soon);
      reader.stop();
      for (const stop of stops) void stop.then((off) => off?.()).catch(() => undefined);
    };
  }, [reader]);
  return { held, reread: reader.read };
}

/** Reads each watched project's list, at once or once a burst has settled. */
function listReader(
  setHeld: (
    change: (was: Record<string, readonly Shown[]>) => Record<string, readonly Shown[]>,
  ) => void,
) {
  let watched = new Set<string>();
  let live = true;
  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  /** The read due when the first hold a project's list names runs out (#1709). */
  const holds = new Map<string, ReturnType<typeof setTimeout>>();
  const read = (plane: string) => {
    if (!live || !watched.has(plane)) return;
    void commands.asksWaiting(plane).then(
      (answer) => {
        // A core that answers nothing for it (an older build, a test's mock) holds none.
        const asks = answer.status === "ok" ? (answer.data as Asking | null)?.asks : undefined;
        if (!(live && watched.has(plane) && asks)) return;
        setHeld((was) => ({ ...was, [plane]: asks }));
        lookAtHoldsEnd(plane, asks);
      },
      () => {},
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
  return {
    read,
    start() {
      if (live) return;
      live = true;
      for (const plane of watched) read(plane);
    },
    soon(plane: string) {
      if (timers.has(plane)) return;
      timers.set(
        plane,
        setTimeout(() => {
          timers.delete(plane);
          read(plane);
        }, SETTLE_MS),
      );
    },
    watch(planes: Set<string>) {
      watched = planes;
      setHeld((was) => Object.fromEntries(Object.entries(was).filter(([one]) => planes.has(one))));
    },
    stop() {
      live = false;
      for (const timer of [...timers.values(), ...holds.values()]) clearTimeout(timer);
      timers.clear();
      holds.clear();
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
): Promise<{ refused: string | undefined; live: boolean; opened?: boolean }> {
  const path = ask.answer;
  if (path.via === "in-its-pane") return { refused: IN_ITS_CHAT, live: false };
  if (!ask.options.some((one) => one.id === option))
    return {
      refused: `${JSON.stringify(option)} is not one of the answers this ask offers.`,
      live: false,
    };
  let live = false;
  let opened = false;
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
        // Make one makes a sandbox-only Codex profile (#2037), then the project's window opens
        // a shell tab where the core types Codex's own sign-in for it. Dismiss puts it away.
        if (option === "dismiss") return commands.dismissSandboxOnly(plane);
        if (option === "make") {
          const made = await commands.makeSandboxOnlyProfile(plane);
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
      case "pool-tool":
        // One run of a runner chat's declared tool (#2047): Allow this run has this desktop send
        // the tool its vault values on the pool's runner for that run alone; Keep off refuses it.
        // The command that waits goes on, or is refused, at once.
        if (option === "allow") return commands.answerPoolTool(plane, ask.session, path.run, true);
        if (option === "keep") return commands.answerPoolTool(plane, ask.session, path.run, false);
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
  return answer.status === "ok"
    ? { refused: undefined, live, ...(opened ? { opened } : {}) }
    : { refused: answer.error, live: false };
}
