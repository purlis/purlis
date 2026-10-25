// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  choose,
  heldBy,
  closeTab,
  focusWorkspace,
  press,
  projectRoot,
  stateOf,
  tabTexts,
  untilInFront,
} from "../acting.js";
import { showView } from "../opening.js";

/**
 * **Past tasks, found by workspace and by date, each drawn with its persona's mark** (#1510,
 * #1457; 43d61151, c3176bcc), in the built app against the real core.
 *
 * A dispatch is recorded by the app that made it, in the project's own dispatch store
 * (`<state>/app/dispatches/<id>.json`, `purlis_core::dispatchrecord`), and a scenario cannot
 * make a persona chat dispatch. So the spec writes three ended tasks' records into that store,
 * as the app writes them: one that worked in `alpha` today, one in `beta` today, and one in
 * `alpha` ten days ago. From there it is the app's: the Sessions view's heading opens the
 * Dispatches tab from `alpha`'s strip, narrowed to `alpha`; *Every workspace* widens it; the date
 * filter keeps today's or the earlier one; and every row draws the persona's shared mark.
 *
 * A dispatch that did not start (c79098ef) is held in the app's memory while it waits on the
 * person or on this machine, and no record on the disk makes one, so only its absence is read
 * here: a project with none draws no *Not started* list.
 *
 * **It leaves the project as it found it**: the three records are taken out of the store, the
 * filters are put back, and the tab is closed.
 */

const DAY_MS = 24 * 60 * 60 * 1000;

type Made = { id: string; task: string; workspace: string; persona: string; daysAgo: number };

const TODAY_ALPHA: Made = {
  id: "01K7E2E0000000000000VHTDY1",
  task: "vh today in alpha",
  workspace: "alpha",
  persona: "devops",
  daysAgo: 0,
};
const TODAY_BETA: Made = {
  id: "01K7E2E0000000000000VHBTA1",
  task: "vh today in beta",
  workspace: "beta",
  persona: "steward",
  daysAgo: 0,
};
const EARLIER_ALPHA: Made = {
  id: "01K7E2E0000000000000VHERX1",
  task: "vh earlier in alpha",
  workspace: "alpha",
  persona: "devops",
  daysAgo: 10,
};
const MADE = [TODAY_ALPHA, TODAY_BETA, EARLIER_ALPHA];

const TAB = "Dispatches";
const PANE = '[data-testid="view-pane-charter-dispatches-"]';
const BY_WORKSPACE = `${PANE} [data-scope="select"][data-part="root"][aria-label="Filter by workspace"]`;
const BY_DATE = `${PANE} [data-scope="select"][data-part="root"][aria-label="Filter by date"]`;

/** An ended task's record, as the app writes one (`dispatchrecord::Record`, version 1). */
function recordOf(made: Made): string {
  const started = new Date(Date.now() - made.daysAgo * DAY_MS);
  const ended = new Date(started.getTime() + 5 * 60 * 1000);
  return JSON.stringify({
    v: 1,
    id: made.id,
    mode: "task",
    asker: { chat: 9301, name: "vh asking chat", workspace: made.workspace },
    persona: made.persona,
    worker: { chat: 9302, name: `${made.persona} ${made.task}`, persona: made.persona },
    task: made.task,
    place: { workspace: made.workspace },
    brief: `Do ${made.task}.`,
    started: started.toISOString(),
    ended: ended.toISOString(),
    report: { outcome: "done", text: `Did ${made.task}.`, changed: {} },
  });
}

/** Which of this spec's own dispatches the tab lists now, read in one pass. */
async function listed(): Promise<string[]> {
  const ids = await browser.execute(
    (pane: string) =>
      [...document.querySelectorAll(`${pane} tr[data-testid^="dispatch-"]`)].map(
        (row) => row.getAttribute("data-testid") ?? "",
      ),
    PANE,
  );
  return MADE.filter((made) => ids.includes(`dispatch-${made.id}`)).map((made) => made.task);
}

/** Waits until the tab lists exactly `tasks` of this spec's own. */
async function untilListed(tasks: string[]): Promise<void> {
  let last: string[] = [];
  await browser
    .waitUntil(
      async () => {
        last = await listed();
        return [...last].sort().join("|") === [...tasks].sort().join("|");
      },
      { timeout: 20_000, interval: 250 },
    )
    .catch(() => {
      throw new Error(`the tab listed ${JSON.stringify(last)}, not ${JSON.stringify(tasks)}`);
    });
}

describe("the Dispatches tab's past tasks", function () {
  this.timeout(180_000);

  let store = "";

  before(async () => {
    store = join(stateOf(await projectRoot()), "app", "dispatches");
    mkdirSync(store, { recursive: true });
    for (const made of MADE) writeFileSync(join(store, `${made.id}.json`), recordOf(made));

    // Opened from alpha's strip, by the Sessions view's heading button: on the line heading
    // its half of the Panel (#1738), or on the panel's own heading where it is drawn elsewhere.
    await focusWorkspace("alpha");
    await showView("Sessions");
    await press(':is(.half-head, [data-testid="panel-sessions"])', "Open dispatches");
    await untilInFront(TAB);
    await $(BY_WORKSPACE).waitForExist({ timeout: 20_000 });
  });

  after(async () => {
    for (const made of MADE) rmSync(join(store, `${made.id}.json`), { force: true });
    if ((await tabTexts()).includes(TAB)) await closeTab(TAB);
    // The Panel's right half opens on Memory (#1738).
    await showView("Memory");
  });

  it("opens narrowed to the workspace it was opened from", async () => {
    expect(await heldBy(BY_WORKSPACE)).toBe("alpha");
    await untilListed([TODAY_ALPHA.task, EARLIER_ALPHA.task]);
  });

  it("widens to every workspace, and narrows to another", async () => {
    await choose(BY_WORKSPACE, "");
    await untilListed(MADE.map((made) => made.task));

    await choose(BY_WORKSPACE, "beta");
    await untilListed([TODAY_BETA.task]);

    await choose(BY_WORKSPACE, "");
  });

  it("keeps today's tasks, or the earlier ones, by the date filter", async () => {
    await choose(BY_DATE, "today");
    await untilListed([TODAY_ALPHA.task, TODAY_BETA.task]);

    await choose(BY_DATE, "earlier");
    await untilListed([EARLIER_ALPHA.task]);

    await choose(BY_DATE, "");
    await untilListed(MADE.map((made) => made.task));
  });

  it("draws each task's persona with the persona's own mark", async () => {
    for (const made of MADE) {
      const mark = await $(
        `${PANE} tr[data-testid="dispatch-${made.id}"] .dispatch-persona .persona-mark`,
      );
      await mark.waitForExist({ timeout: 10_000 });
      expect(await mark.getAttribute("data-persona")).toBe(made.persona);
      // The shared mark draws an icon, an image or initials: never the old stand-in glyph.
      expect(["icon", "image", "initials"]).toContain(await mark.getAttribute("data-mark"));
    }
  });

  it("draws no Not started list while no dispatch waits", async () => {
    expect(
      await $(`${PANE} section[aria-label="Dispatches that did not start"]`).isExisting(),
    ).toBe(false);
  });
});
