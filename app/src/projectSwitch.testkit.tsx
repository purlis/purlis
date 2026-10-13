// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { expect } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat } from "./bindings";
import { panes } from "./countedPane.testkit";
import { counts } from "./reactCommits.testkit";
import { stripNamed } from "./test-strips";

/**
 * **What a project switch costs the window, measured in the real `App`**.
 *
 * The operator's report: switching between projects freezes for one to three seconds, worse the
 * more chats are open. This mounts a window holding `projects` projects, each with `chats` chats
 * spread over `workspaces` workspaces and some of them asking, then presses another project's
 * tab and reads what React did for it:
 *
 * - how long the press took until React had committed and run its effects (`syncMs`), and until
 *   everything the press started had settled (`settledMs`);
 * - how many components were drawn again, how many were mounted and how many unmounted, read
 *   off React's own commits (the DevTools hook, which is how the React profiler counts them);
 * - how many DOM nodes were put in and taken out;
 * - how many panes (terminals) were started and stopped;
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
};

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

/** What the window sent the core, in order. */
export type Asked = { cmd: string; plane: string | undefined };

/** A core holding the projects `shape` names, with every call kept. */
function core(shape: Shape): Asked[] {
  const asked: Asked[] = [];
  const roots = Array.from({ length: shape.projects }, (_, n) => rootOf(n));
  const chats = new Map(
    roots.map((root, n) => [root, chatsOf(root, shape, shape.chatsIn?.[n] ?? shape.chats)]),
  );
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    const plane = given.plane as string | undefined;
    asked.push({ cmd, plane });
    if (cmd === "plugin:event|listen") return 1;
    const open = chats.get(plane ?? "") ?? [];
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return { windows: [{ planes: roots, active: 0 }], dropped: [], gone: [] };
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar")
      return {
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
    if (cmd === "opened_chats") return open;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
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
    return null;
  });
  return asked;
}

function projectTab(name: string): HTMLElement {
  const tab = within(stripNamed("Projects"))
    .getAllByRole("tab")
    .find((one) => one.querySelector(".project-name")?.textContent === name);
  if (!tab) throw new Error(`no project tab called ${name}`);
  return tab;
}

const inFront = () =>
  within(stripNamed("Projects"))
    .getAllByRole("tab")
    .find((tab) => tab.getAttribute("aria-selected") === "true")
    ?.querySelector(".project-name")?.textContent;

/** Everything the window has started has settled: nothing new was asked for a while. */
async function settled(asked: Asked[]): Promise<void> {
  let seen = -1;
  for (let rounds = 0; seen !== asked.length; rounds++) {
    if (rounds > 200) throw new Error(`the window never settled: ${asked.at(-1)?.cmd}`);
    seen = asked.length;
    await act(async () => {
      await new Promise((done) => setTimeout(done, 30));
    });
  }
}

/** What one switch cost. */
export type Switch = {
  to: string;
  /** The press, until React had committed and run its effects. */
  syncMs: number;
  /** The press, until everything it started had settled, less the waiting `settled` adds. */
  commits: number;
  rendered: number;
  mounted: number;
  unmounted: number;
  nodesAdded: number;
  nodesRemoved: number;
  panesStarted: number;
  panesStopped: number;
  /** Reads of the layout, by what was read, and how many in all. */
  layoutReads: Record<string, number>;
  layoutRead: number;
  /** Commands sent to the core, by name. */
  invokes: Record<string, number>;
  invoked: number;
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
 * Mounts the window, brings every project to the front once (so nothing measured afterwards is
 * a first visit), then switches to each of `visits` in turn and answers with what each cost.
 */
export async function switches(shape: Shape, visits: readonly number[]): Promise<Switch[]> {
  const asked = core(shape);
  render(<App />);
  await waitFor(() => expect(inFront()).toBe(nameOf(0)), { timeout: 20_000 });
  await settled(asked);
  for (let n = 1; n < shape.projects; n++) {
    fireEvent.click(projectTab(nameOf(n)));
    await settled(asked);
  }
  fireEvent.click(projectTab(nameOf(0)));
  await settled(asked);

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
    Object.assign(panes, { started: 0, stopped: 0 });
    watchLayoutReads();
    reads.clear();
    const from = asked.length;

    const pressed = performance.now();
    act(() => {
      fireEvent.click(tab);
    });
    const syncMs = performance.now() - pressed;
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
      paintedAtOnce,
    });
  }
  return out;
}
