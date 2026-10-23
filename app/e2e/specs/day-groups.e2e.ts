// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { showView } from "../opening.js";
import { focusWorkspace, projectInFront } from "../reach.js";
import { tooltipsOf } from "../tooltips.js";

/**
 * **The right side's dated rows under Today, Yesterday and Earlier** (#1674, B-9), in the built
 * app, off a store on the disk.
 *
 * The fixture's one todo in `alpha` was written on 2026-03-02. This spec writes two more into
 * the project in front, stamped today and yesterday as the store stamps them, and reads the
 * Todos view: three headings in that order, each over the rows written on its day, every row
 * its title alone with its date on hover. What `dayGroups.ts` decides is its unit tests'; what
 * is the window's here is that a store written on the disk reaches the view through the
 * project's watch and is drawn under its days in the real engine.
 *
 * **It leaves the project as it found it**: both todos are taken off the disk, the view waits
 * until they are gone, and the Panel's right half is put back on Memory, the view it opens on.
 */

const TODOS = '[data-testid="panel-todos"]';
const TODAY = "Day groups: a todo written today";
const YESTERDAY = "Day groups: a todo written yesterday";
const EARLIER = "Review the rollout plan";

const two = (n: number) => String(n).padStart(2, "0");
/** `YYYY-MM-DD` and `HH:MM` of `when`, in local time, as the store writes a stamp. */
const dayOf = (when: Date) =>
  `${when.getFullYear()}-${two(when.getMonth() + 1)}-${two(when.getDate())}`;
const timeOf = (when: Date) => `${two(when.getHours())}:${two(when.getMinutes())}`;

/** A todo's file as the store writes one: its title, its stamp line, and its text. */
function aTodo(store: string, title: string, slug: string, when: Date): string {
  const stem = `${dayOf(when).replace(/-/g, "")}-${timeOf(when).replace(":", "")}00-${slug}`;
  const file = join(store, `${stem}.md`);
  writeFileSync(file, `# ${title}\n\n_${dayOf(when)} ${timeOf(when)} · persistent_\n\n${title}\n`);
  return file;
}

/** Each day heading the Todos view draws, with the titles under it, in order. */
async function days(): Promise<{ day: string; rows: { text: string; hover: string }[] }[]> {
  // Each row's hover is its tooltip (#2908), read as the keyboard reads it, in document order.
  const hovers = await tooltipsOf(`${TODOS} .panel-days .panel-row .row`);
  const drawn = await browser.execute((panel: string) => {
    const list = document.querySelector(`${panel} .panel-days`);
    if (!list) return [];
    const every = [...list.querySelectorAll(".panel-row .row")];
    return [...list.querySelectorAll("h3.panel-day")].map((heading) => {
      const rows = heading.nextElementSibling;
      return {
        day: heading.textContent ?? "",
        rows: [...(rows?.querySelectorAll(".panel-row") ?? [])].map((row) => {
          const own = row.querySelector(".row");
          return {
            text: row.querySelector(".row-text")?.textContent ?? "",
            at: own === null ? -1 : every.indexOf(own),
          };
        }),
      };
    });
  }, TODOS);
  return drawn.map(({ day, rows }) => ({
    day,
    rows: rows.map(({ text, at }) => ({ text, hover: (at < 0 ? null : hovers[at]) ?? "" })),
  }));
}

/** The titles the Todos view lists, whatever it groups them under. */
async function titles(): Promise<string[]> {
  return browser.execute(
    (panel: string) =>
      [...document.querySelectorAll(`${panel} .row-text`)].map((one) => one.textContent ?? ""),
    TODOS,
  );
}

describe("the right side's rows grouped by day", () => {
  const written: string[] = [];
  const now = new Date();
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);

  before(async () => {
    await focusWorkspace("alpha");
    // Todos is in the Panel's right half (#1738).
    await showView("Todos");
    await browser.waitUntil(async () => (await titles()).includes(EARLIER), {
      timeout: 30_000,
      interval: 250,
      timeoutMsg: "the Todos view never listed the fixture's todo",
    });
    const store = join(await projectInFront(), "workspaces", "alpha", "todos");
    written.push(aTodo(store, YESTERDAY, "day-groups-yesterday", yesterday));
    written.push(aTodo(store, TODAY, "day-groups-today", now));
  });

  after(async () => {
    for (const file of written) rmSync(file, { force: true });
    await browser.waitUntil(
      async () => {
        const left = await titles();
        return !left.includes(TODAY) && !left.includes(YESTERDAY);
      },
      { timeout: 60_000, interval: 250, timeoutMsg: "the day-groups todos stayed in the view" },
    );
    await showView("Memory");
  });

  it("draws Today, Yesterday and Earlier in that order, each over the rows of its day", async () => {
    let seen: Awaited<ReturnType<typeof days>> = [];
    await browser
      .waitUntil(
        async () => {
          seen = await days();
          return seen.map((one) => one.day).join(",") === "Today,Yesterday,Earlier";
        },
        { timeout: 60_000, interval: 250 },
      )
      .catch(() => {
        throw new Error(`the Todos view never drew three days; it drew ${JSON.stringify(seen)}`);
      });

    const under = (day: string) =>
      (seen.find((one) => one.day === day)?.rows ?? []).map((row) => row.text);
    expect(under("Today")).toContain(TODAY);
    expect(under("Yesterday")).toContain(YESTERDAY);
    expect(under("Earlier")).toContain(EARLIER);
    // Each under its own day and no other.
    expect([...under("Yesterday"), ...under("Earlier")]).not.toContain(TODAY);
    expect([...under("Today"), ...under("Earlier")]).not.toContain(YESTERDAY);
    expect([...under("Today"), ...under("Yesterday")]).not.toContain(EARLIER);
  });

  it("draws each row as its title alone, with the day it was written on hover", async () => {
    const seen = await days();
    const row = (day: string, text: string) =>
      seen.find((one) => one.day === day)?.rows.find((one) => one.text === text);
    expect(row("Today", TODAY)?.hover).toContain(dayOf(now));
    expect(row("Yesterday", YESTERDAY)?.hover).toContain(dayOf(yesterday));
    expect(row("Earlier", EARLIER)?.hover).toContain("2026-03-02");
    // The note is the hover and not a second piece of the line (#1674).
    const notes = await browser.execute(
      (panel: string) => document.querySelectorAll(`${panel} .row-note`).length,
      TODOS,
    );
    expect(notes).toBe(0);
  });

  it("puts the headings and their rows in one list that does not scroll sideways", async () => {
    const drawn = await browser.execute((panel: string) => {
      const list = document.querySelector<HTMLElement>(`${panel} .panel-days`);
      return {
        role: list?.getAttribute("role") ?? "",
        sideways: list === null ? -1 : list.scrollWidth - list.clientWidth,
        // Each day's rows are named by their heading, so a screen reader says which day.
        named: [...document.querySelectorAll(`${panel} ul.panel-day-rows`)].every((rows) => {
          const by = rows.getAttribute("aria-labelledby");
          return by !== null && document.getElementById(by)?.tagName === "H3";
        }),
      };
    }, TODOS);
    expect(drawn.role).toBe("group");
    expect(drawn.sideways).toBeGreaterThanOrEqual(0);
    expect(drawn.sideways).toBeLessThanOrEqual(1);
    expect(drawn.named).toBe(true);
  });
});
