// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  closeChat,
  type Content,
  contentsOf,
  insertPane,
  isOneJobPane,
  noTabs,
  noteUsed,
  oneJobEnded,
  openOneJob,
  openTab,
  openView,
  press,
  type Pressing,
  shellExited,
  type Tabs,
  type ViewRef,
} from "./tabs";

/**
 * **Shells and one-job programs** (#4761; spec #4750 stories 63 to 66 and 78, ADR 0062 and
 * 0081 as amended 2026-11-02). A terminal editor opened from a file, or a forge login, opens as
 * a split beside the focused pane with the keyboard, and its split goes when its program exits,
 * the keyboard back where it was. A shell whose program exits leaves its pane empty. The
 * window's pure layout model.
 */

const chat = (session: number): Content => ({ kind: "session", session, chat: String(session) });
const filedIn = () => "ops";
const pressing: Pressing = { filedIn, frontOf: () => undefined };
const PERSONA: ViewRef = { from: null, view: "persona", key: "steward" };

/** Chat 1 and chat 2 side by side in one tab, chat 2's pane focused. */
function twoPanes(): { tabs: Tabs; id: number; first: number; second: number } {
  const one = openTab(noTabs(), 1, "1", "steward");
  const id = one.order[0];
  const first = one.byId[id].focused;
  const tabs = insertPane(one, {
    tab: id,
    pane: first,
    direction: "row",
    side: "after",
    content: chat(2),
  });
  return { tabs, id, first, second: tabs.byId[id].focused };
}

describe("a one-job program", () => {
  it("opens beside the focused pane, in the same tab, with the keyboard", () => {
    const { tabs, id, first, second } = twoPanes();
    const onFirst = { ...tabs, byId: { ...tabs.byId, [id]: { ...tabs.byId[id], focused: first } } };

    const opened = openOneJob(onFirst, 9, "vim notes.md");

    expect(opened.order).toEqual([id]);
    const tab = opened.byId[id];
    const job = contentsOf(opened, id).find(
      (one) => one.content.kind === "session" && one.content.session === 9,
    );
    if (job === undefined) throw new Error("no pane shows the one-job program");
    expect(tab.focused).toBe(job.pane);
    expect(isOneJobPane(opened, job.pane)).toBe(true);
    expect(isOneJobPane(opened, second)).toBe(false);
    // Beside the pane that had the keyboard: the first, which is now a split of it and the job.
    expect(tab.layout).toMatchObject({
      kind: "split",
      children: [
        { kind: "split", direction: "row", children: [{ pane: first }, { pane: job.pane }] },
        { pane: second },
      ],
    });
  });

  it("goes on exit, and the keyboard goes back to the pane it opened from", () => {
    const { tabs, id, first, second } = twoPanes();
    const opened = openOneJob(tabs, 9, "vim notes.md");

    const ended = oneJobEnded(opened, 9, filedIn);

    expect(ended.byId[id].layout).toEqual(tabs.byId[id].layout);
    expect(ended.byId[id].focused).toBe(second);
    expect(contentsOf(ended, id).map((one) => one.pane)).toEqual([first, second]);
  });

  it("gives the keyboard to the pane left where the one it opened from has gone", () => {
    const { tabs, id, first } = twoPanes();
    const opened = openOneJob(tabs, 9, "vim notes.md");
    // Chat 2, whose pane it opened from, is ended meanwhile.
    const without2 = closeChat(opened, 2, filedIn);

    const ended = oneJobEnded(without2, 9, filedIn);

    expect(contentsOf(ended, id).map((one) => one.pane)).toEqual([first]);
    expect(ended.byId[id].focused).toBe(first);
  });

  it("opens in a tab of its own where no tab is in front, and takes that tab with it", () => {
    const opened = openOneJob(noTabs(), 9, "gh auth login");
    expect(opened.order).toHaveLength(1);

    const ended = oneJobEnded(opened, 9, filedIn);

    expect(ended.order).toEqual([]);
  });

  it("is never what a press replaces: the press goes to the pane used before it", () => {
    const { tabs, id, second } = twoPanes();
    // The window stamps the pane with the keyboard on every change: chat 2's, used last.
    const opened = openOneJob(noteUsed(tabs), 9, "vim notes.md");

    const { tabs: pressed } = press(opened, { session: 5, chat: "5" }, pressing);

    const shown = contentsOf(pressed, id).map((one) => one.content);
    expect(shown).toContainEqual(expect.objectContaining({ session: 9 }));
    expect(shown).toContainEqual(expect.objectContaining({ session: 5 }));
    expect(pressed.byId[id].focused).toBe(second);
  });

  it("changes nothing for a session no pane shows", () => {
    const { tabs } = twoPanes();
    expect(oneJobEnded(tabs, 9, filedIn)).toBe(tabs);
  });
});

describe("a shell whose program exits", () => {
  it("leaves its pane empty, in its place, with the keyboard where it was", () => {
    const { tabs, id, first, second } = twoPanes();

    const exited = shellExited(tabs, 2, filedIn);

    expect(contentsOf(exited, id)).toEqual([
      { pane: first, content: chat(1) },
      { pane: second, content: { kind: "empty", workspace: "ops" } },
    ]);
    expect(exited.byId[id].focused).toBe(tabs.byId[id].focused);
  });

  it("changes nothing for a shell no pane shows, nor for a view", () => {
    const tabs = openView(noTabs(), PERSONA, "steward", "ops");
    expect(shellExited(tabs, 2, filedIn)).toBe(tabs);
  });
});
