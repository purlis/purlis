// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Profiler } from "react";
import { expect } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { Moved, OpenChat } from "./bindings";
import { SETTLE_MS as ASKS_SETTLE_MS } from "./asks";
import { CHAT_EVENTS } from "./chatEvents";
import { aBatchOf } from "./chatEvents.testkit";
import { panes } from "./countedPane.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { commitRoots, counts, renderedBy } from "./reactCommits.testkit";
import { stripNamed } from "./test-strips";
import { themeInForce } from "./theme/inForce.testkit";

/**
 * **What a project switch costs the window, measured in the real `App`**.
 *
 * The operator's report: switching between projects freezes for one to three seconds, worse the
 * more chats are open. This mounts a window holding `projects` projects, each with `chats` chats
 * spread over `workspaces` workspaces and some of them asking, then presses another project's
 * tab and reads what React did for it:
 *
 * - how long the press took until React had committed and run its effects (`syncMs`);
 * - how many components were drawn again, how many were mounted and how many unmounted, read
 *   off React's own commits (the DevTools hook, which is how the React profiler counts them);
 * - how many DOM nodes were put in and taken out, which for a view that is kept is one element
 *   moved with everything in it;
 * - how many panes (terminals) were started and stopped;
 * - how many times the layout was read;
 * - which commands the window sent the core, and how many.
 *
 * **The counts are the measurement; the milliseconds are jsdom's.** jsdom lays nothing out and
 * paints nothing, and React here is its development build, so a time read here is neither the
 * WebView's time nor proportional to it. A count is the same on every machine, which is what a
 * guard can hold (ADR 0086: no absolute timing on a shared runner).
 */

export type Shape = {
  /** How many projects the window holds. */
  projects: number;
  /** How many chats each project has open. */
  chats: number;
  /** How many workspaces each project has; the chats are spread over them in turn. */
  workspaces: number;
  /** Overrides `chats` for one project, by its place: for telling whose chats a switch costs. */
  chatsIn?: Record<number, number>;
  /** How many panes the tab in front holds in each project switched between: the first chat's
   *  pane, split until there are this many. One, unless said. */
  panes?: number;
};

/** The test's own clock, read for `syncMs`: the real one, taken before a test can hold the
 *  window's still (`projectSwitch.cost.test.tsx`). */
const clock = performance.now.bind(performance);

export const rootOf = (n: number) => `/home/dev/project-${n}`;
export const nameOf = (n: number) => `project-${n}`;

function chatsOf(root: string, shape: Shape, count: number): OpenChat[] {
  return Array.from({ length: count }, (_, at) => ({
    session: at + 1,
    id: `${root}#${at + 1}`,
    name: `steward ${at + 1}`,
    cwd: `${root}/workspaces/w${at % shape.workspaces}`,
    harness: "claude",
    in_front: at === 0,
    resumed: null,
    fresh: null,
    profile: null,
    persona: "steward",
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  }));
}

/** What the picker offers a new chat: one profile, as a machine with one harness has. */
const START_OPTIONS = {
  profiles: [
    {
      name: "claude",
      kind: "claude",
      shown: "claude",
      source: "built-in",
      is_default: true,
      approval: null,
    },
  ],
  refused: [],
  personas: ["steward"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  declares_none: true,
};

/** What the window sent the core, in order. */
export type Asked = { cmd: string; plane: string | undefined };

/** A core holding the projects `shape` names, with every call kept. */
type Core = {
  asked: Asked[];
  /** Says `plane-changed` for `plane`'s answer `answer`, as the core's watcher would. */
  planeChanged: (plane: string, answer: string) => Promise<void>;
  /** Says `chat-moved` with `payload`, as the core does for each move of a chat. */
  chatMoved: (payload: unknown) => void;
  /** Holds every `plane_sidebar` answer from now on until the function it answers is called. */
  holdSidebar: () => () => void;
  /** Writes every `plane_sidebar` answer from now on with each object's keys in reverse order:
   *  the same answer, as a core built another way could send it. */
  reverseSidebarKeys: () => void;
};

/** `value` with the keys of every object in it in reverse order. */
const keysReversed = (value: unknown): unknown =>
  Array.isArray(value)
    ? value.map(keysReversed)
    : value !== null && typeof value === "object"
      ? Object.fromEntries(
          Object.entries(value)
            .reverse()
            .map(([key, inner]) => [key, keysReversed(inner)]),
        )
      : value;

/**
 * What the core answers to a command the core above answers with nothing, by the command's name:
 * for a test about what the window does with that answer. Read at each ask, so a test can change
 * an answer after the window is mounted.
 */
export type Answers = Record<string, (given: Record<string, unknown>) => unknown>;

function core(shape: Shape, answers: Answers = {}): Core {
  const asked: Asked[] = [];
  const listening = new Map<string, number[]>();
  let held: Promise<void> | undefined;
  let reversed = false;
  const roots = Array.from({ length: shape.projects }, (_, n) => rootOf(n));
  const chats = new Map(
    roots.map((root, n) => [root, chatsOf(root, shape, shape.chatsIn?.[n] ?? shape.chats)]),
  );
  // Past every session a project was given: a chat a split starts is a new one.
  let started = 1_000;
  mockIPC(async (cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    const plane = given.plane as string | undefined;
    asked.push({ cmd, plane });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as { event: string; handler: number };
      listening.set(event, [...(listening.get(event) ?? []), handler]);
      return 1;
    }
    const open = chats.get(plane ?? "") ?? [];
    if (cmd === "plane_sidebar" && held !== undefined) await held;
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return { windows: [{ planes: roots, active: 0 }], dropped: [], gone: [] };
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar") {
      const answer = {
        root: plane ?? "",
        workspaces: Array.from({ length: shape.workspaces }, (_, w) => ({
          name: `w${w}`,
          path: `${plane}/workspaces/w${w}`,
          vision: "Ship it",
          todos: ["one thing", "another"],
          chats: open.filter((one) => one.cwd === `${plane}/workspaces/w${w}`),
          colour: null,
          live: false,
        })),
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
      return reversed ? keysReversed(answer) : answer;
    }
    if (cmd === "opened_chats") return open;
    // Each chat in a tab of its own on its workspace's strip, as a record would keep them
    // (#4758): a chat gets a tab only from the stored layout.
    if (cmd === "reopened_layout")
      return aTabForEachChat(open, (session) => {
        const cwd = open.find((one) => one.session === session)?.cwd ?? "";
        return cwd.split("/workspaces/")[1] ?? null;
      });
    // A split starts a chat, on the profile its picker offers.
    if (cmd === "start_details") return [];
    if (cmd === "start_options" || cmd === "start_options_first") return START_OPTIONS;
    if (cmd === "start_chat") return { session: (started += 1) };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    if (cmd === "theme_in_force") return themeInForce(args);
    if (cmd === "project_theme_drawn") return null;
    // One chat in five waits on the person: a populated Inbox, and counts on the project tabs.
    if (cmd === "asks_waiting")
      return {
        plane,
        asks: open
          .filter((one) => one.session % 5 === 0)
          .map((one) => ({
            session: one.session,
            ask: `question:${one.session}`,
            says: "Waiting on your reply",
            options: [],
            source: "question",
            chain: [one.name],
            answer: { via: "in-its-pane" },
          })),
      };
    const answer = answers[cmd];
    if (answer !== undefined) return answer(given);
    return null;
  });
  return {
    asked,
    planeChanged: async (plane, answer) => {
      await act(async () => {
        for (const handler of listening.get("plane-changed") ?? [])
          window.__TAURI_INTERNALS__.runCallback(handler, {
            event: "plane-changed",
            id: 1,
            payload: { plane, changes: null, answers: [{ answer, workspace: null }] },
          });
      });
    },
    chatMoved: (payload) => {
      for (const handler of listening.get(CHAT_EVENTS) ?? [])
        window.__TAURI_INTERNALS__.runCallback(handler, {
          event: CHAT_EVENTS,
          id: 1,
          payload: aBatchOf("moved", payload as Moved),
        });
    },
    holdSidebar: () => {
      let release = () => {};
      held = new Promise((done) => {
        release = () => {
          held = undefined;
          done();
        };
      });
      return release;
    },
    reverseSidebarKeys: () => {
      reversed = true;
    },
  };
}

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

function projectTab(name: string): HTMLElement {
  const tab = within(stripNamed("Projects"))
    .getAllByRole("tab")
    .find((one) => one.getAttribute("aria-label") === name);
  if (!tab) throw new Error(`no project tab called ${name}`);
  return tab;
}

const inFront = () =>
  within(stripNamed("Projects"))
    .getAllByRole("tab")
    .find((tab) => tab.getAttribute("aria-selected") === "true")
    ?.getAttribute("aria-label");

/**
 * **Everything the window has started has settled**: a round in which nothing new was asked and
 * React committed nothing, the two things a switch is read by.
 *
 * **Each round spans an animation frame and then a timer** (#3032), as `afterPaint` does
 * (`projectShown.tsx`): what is read again once the project is painted is asked from a timer set
 * in a frame, so a round that only waited a timer could end before that frame came, on a runner
 * whose frames come late, and the switch's own reads and commits were counted for the next one.
 * Any `afterPaint` started before the round has run by its end.
 */
export async function settled(asked: Asked[]): Promise<void> {
  const now = () => `${asked.length}:${counts.commits}`;
  let seen = "";
  for (let rounds = 0; seen !== now(); rounds++) {
    if (rounds > 200) throw new Error(`the window never settled: ${asked.at(-1)?.cmd}`);
    seen = now();
    await act(async () => {
      await new Promise((done) => requestAnimationFrame(() => setTimeout(done, 30)));
    });
  }
}

/** What one switch cost. */
export type Switch = {
  to: string;
  /** The press, until React had committed and run its effects. */
  syncMs: number;
  /** How many times React committed for it, until everything it started had settled. */
  commits: number;
  rendered: number;
  /** How many times each component was drawn for it, by name. */
  renderedBy: Record<string, number>;
  /** Where each of its commits began: the topmost components drawn in it (`commitRoots`). */
  began: string[][];
  mounted: number;
  unmounted: number;
  nodesAdded: number;
  nodesRemoved: number;
  panesStarted: number;
  panesStopped: number;
  /** Reads of the layout, by what was read, and how many in all. */
  layoutReads: Record<string, number>;
  layoutRead: number;
  /** Commands sent to the core until everything settled, by name, and how many in all. */
  invokes: Record<string, number>;
  invoked: number;
  /** How many of them were sent before the press returned: before the project was painted. */
  invokedAtOnce: number;
  /** How many panes were on screen once the project was in front. */
  panes: number;
  /** Whether the project's chat strip and its pane were on screen when the press returned,
   *  before any command it sent had been answered. */
  paintedAtOnce: boolean;
};

/** Reads of the layout since the last reset, by what was read. In the WebView each one that
 *  follows a write lays the window out again before it answers; jsdom answers at once. */
const reads = new Map<string, number>();
let readsWatched = false;

function watchLayoutReads(): void {
  if (readsWatched) return;
  readsWatched = true;
  const count = (what: string) => {
    reads.set(what, (reads.get(what) ?? 0) + 1);
  };
  const rect = Element.prototype.getBoundingClientRect;
  Element.prototype.getBoundingClientRect = function (this: Element) {
    count("getBoundingClientRect");
    return rect.call(this);
  };
  const rects = Element.prototype.getClientRects;
  Element.prototype.getClientRects = function (this: Element) {
    count("getClientRects");
    return rects.call(this);
  };
  const style = window.getComputedStyle;
  window.getComputedStyle = ((el: Element, pseudo?: string | null) => {
    count("getComputedStyle");
    return style.call(window, el, pseudo);
  }) as typeof window.getComputedStyle;
  for (const [proto, names] of [
    [HTMLElement.prototype, ["offsetWidth", "offsetHeight", "offsetLeft", "offsetTop"]],
    [Element.prototype, ["clientWidth", "clientHeight", "scrollWidth", "scrollHeight"]],
  ] as const) {
    for (const name of names) {
      const held = Object.getOwnPropertyDescriptor(proto, name);
      if (!held?.get) continue;
      const get = held.get;
      Object.defineProperty(proto, name, {
        ...held,
        get(this: Element) {
          count(name);
          return get.call(this) as number;
        },
      });
    }
  }
}

/** How many nodes are in a tree, itself included. */
function nodesIn(node: Node): number {
  let count = 1;
  for (const child of node.childNodes) count += nodesIn(child);
  return count;
}

/**
 * How many times React has committed for the window {@link aWindow} mounted, as a `Profiler`
 * around it says.
 *
 * Read off a `Profiler` and not off {@link counts}: the DevTools hook those are counted by works
 * only when it is in place before `react-dom` is first loaded, and the test setup loads it first.
 */
export const windowCommits = { count: 0 };

/** The real window on a core holding `shape`, mounted with its first project in front and
 *  settled. */
export async function aWindow(shape: Shape, answers?: Answers): Promise<Core> {
  const held = core(shape, answers);
  render(
    <Profiler
      id="window"
      onRender={() => {
        windowCommits.count += 1;
      }}
    >
      <App />
    </Profiler>,
  );
  await waitFor(() => expect(inFront()).toBe(nameOf(0)), { timeout: 20_000 });
  await settled(held.asked);
  return held;
}

/**
 * Mounts the window, brings every project to the front once, then the ones in `visits` and the
 * first again, so nothing measured afterwards is a first visit and each of them is a project the
 * person was in lately. Then switches to each of `visits` in turn and answers with what each
 * cost. The window starts on the first project, so `visits` should not start with it.
 */
export async function switches(shape: Shape, visits: readonly number[]): Promise<Switch[]> {
  const { asked } = await aWindow(shape);
  for (let n = 1; n < shape.projects; n++) {
    fireEvent.click(projectTab(nameOf(n)));
    await settled(asked);
  }
  for (const n of [...new Set(visits), 0]) {
    fireEvent.click(projectTab(nameOf(n)));
    await settled(asked);
    // The tab in front, split until it holds as many panes as the shape says.
    for (let held = screen.getAllByTestId("pane").length; held < (shape.panes ?? 1); held++) {
      fireEvent.click(screen.getAllByRole("button", { name: "Split right" })[0]);
      fireEvent.click(await screen.findByRole("button", { name: "Start chat" }));
      await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(held + 1));
      await settled(asked);
    }
    // Each split offers its Undo for a few seconds (#4763). The line is put away now, as the
    // person would, so that its time running out is not counted as a switch's cost: a switch
    // slow enough here, forty panes on a busy runner, saw it go mid-switch and counted its 20
    // components among the unmounted.
    // The cause is on the card's message; its Dismiss is beside it, in Updates (#5644), which
    // may be behind Asks.
    for (const said of document.querySelectorAll('[data-cause="layout-changed"]'))
      fireEvent.click(
        within(said.parentElement as HTMLElement).getByRole("button", {
          name: "Dismiss",
          hidden: true,
        }),
      );
    await settled(asked);
    expect(document.querySelector('[data-cause="layout-changed"]')).toBeNull();
  }

  const out: Switch[] = [];
  for (const to of visits) {
    const tab = projectTab(nameOf(to));
    const nodes = { added: 0, removed: 0 };
    const watching = new MutationObserver((records) => {
      for (const record of records) {
        for (const node of record.addedNodes) nodes.added += nodesIn(node);
        for (const node of record.removedNodes) nodes.removed += nodesIn(node);
      }
    });
    watching.observe(document.body, { childList: true, subtree: true });
    Object.assign(counts, { rendered: 0, mounted: 0, unmounted: 0, commits: 0 });
    renderedBy.clear();
    commitRoots.length = 0;
    Object.assign(panes, { started: 0, stopped: 0 });
    watchLayoutReads();
    reads.clear();
    const from = asked.length;

    const pressed = clock();
    act(() => {
      fireEvent.click(tab);
    });
    const syncMs = clock() - pressed;
    const invokedAtOnce = asked.length - from;
    const paintedAtOnce =
      inFront() === nameOf(to) &&
      screen.queryByRole("tablist", { name: "Tabs" }) !== null &&
      screen.queryAllByTestId("pane").length > 0;
    await settled(asked);
    for (const record of watching.takeRecords()) {
      for (const node of record.addedNodes) nodes.added += nodesIn(node);
      for (const node of record.removedNodes) nodes.removed += nodesIn(node);
    }
    watching.disconnect();

    const invokes: Record<string, number> = {};
    for (const one of asked.slice(from)) invokes[one.cmd] = (invokes[one.cmd] ?? 0) + 1;
    out.push({
      to: nameOf(to),
      syncMs,
      commits: counts.commits,
      rendered: counts.rendered,
      renderedBy: Object.fromEntries(renderedBy),
      began: commitRoots.map((roots) => [...roots]),
      mounted: counts.mounted,
      unmounted: counts.unmounted,
      nodesAdded: nodes.added,
      nodesRemoved: nodes.removed,
      panesStarted: panes.started,
      panesStopped: panes.stopped,
      layoutReads: Object.fromEntries(reads),
      layoutRead: [...reads.values()].reduce((sum, one) => sum + one, 0),
      invokes,
      invoked: asked.length - from,
      invokedAtOnce,
      panes: screen.getAllByTestId("pane").length,
      paintedAtOnce,
    });
  }
  return out;
}

/** What a run of chat moves cost the window. */
export type Moves = {
  /** How many times React committed for them, until everything they started had settled. */
  commits: number;
  /** How many times the window's root (`App`) was drawn again for them. */
  windowDrawn: number;
  /** Commands sent to the core for them, by name. */
  invokes: Record<string, number>;
};

/**
 * Mounts the window holding `shape`, brings every project to the front once and the first back,
 * then has the core push `times` moves of one busy chat of project `of`: the same chat, working
 * throughout and waiting on nobody, each move newer than the last, as a harness at work makes
 * them. Answers with what those moves cost.
 */
export async function chatMoves(
  shape: Shape,
  { of, times }: { of: number; times: number },
): Promise<Moves> {
  const held = core(shape);
  const { asked } = held;
  render(<App />);
  await waitFor(() => expect(inFront()).toBe(nameOf(0)), { timeout: 20_000 });
  await settled(asked);
  for (const n of [...Array.from({ length: shape.projects }, (_, n) => n).slice(1), 0]) {
    fireEvent.click(projectTab(nameOf(n)));
    await settled(asked);
  }
  const plane = rootOf(of);
  const move = (at: number) =>
    held.chatMoved({
      plane,
      session: 2,
      state: "working",
      needs_you: false,
      queue: [],
      sequence: 1_000 + at,
      moved_at: 1_000 + at,
      reports: [],
      refusals: [],
    });

  // The first move says the chat is working; what is counted is every move after it.
  act(() => move(0));
  await settled(asked);

  Object.assign(counts, { rendered: 0, mounted: 0, unmounted: 0, commits: 0 });
  renderedBy.clear();
  const from = asked.length;
  for (let at = 1; at <= times; at++) act(() => move(at));
  // Past the wait after which a burst of moves has the asks read again (`asks.ts`), which is
  // longer than the one `settled` looks for.
  await act(async () => {
    await new Promise((done) => setTimeout(done, ASKS_SETTLE_MS * 4));
  });
  await settled(asked);
  const invokes: Record<string, number> = {};
  for (const one of asked.slice(from)) invokes[one.cmd] = (invokes[one.cmd] ?? 0) + 1;
  return { commits: counts.commits, windowDrawn: renderedBy.get("App") ?? 0, invokes };
}
