// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import type { Bench, Plan } from "../src/bench.ts";
import { anEmptyRecord, copyFixturePlane } from "./harness.js";
import { closeProject, openThroughTheTrustQuestion } from "./opening.js";
import { attributesOfEach } from "./reading.js";
import { LOGS } from "./processes.js";
import { escapeUntilClosed } from "./tooltips.js";
import { OPEN, summarise, type Switches } from "./switchTimes.js";

export { BUDGET_MS, OPEN, heldToBudget, type Switches } from "./switchTimes.js";

/**
 * Ten projects open in one window, and the switch between them timed (FR-27, row L9 of the
 * spec's budgets of record). Shared by the scenario run, which records it on every pull request
 * (`specs/project-switch.e2e.ts`), and the benchmark, which holds it to the budget on the
 * operator's machine (`bench/projects.bench.ts`): one copy of how a project is opened and how
 * a switch is measured, so the two cannot drift into measuring different things.
 */

export const PROJECTS = '[data-strip="Projects"]';

/** What the app answered a command with, insisting it answered at all. */
export async function ask<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const answer = await browser.executeAsync(
    (
      name: string,
      passed: Record<string, unknown>,
      done: (out: { ok?: unknown; trouble?: string }) => void,
    ) => {
      void window.__TAURI__.core
        .invoke(name, passed)
        .then((ok) => done({ ok }))
        .catch((e: unknown) => done({ trouble: String(e) }));
    },
    command,
    args,
  );
  if (answer.trouble !== undefined) throw new Error(`${command} refused: ${answer.trouble}`);
  return answer.ok as T;
}

/**
 * Runs a job in the window and answers with its result, once it has finished — the scenario
 * run's, which has no benchmark display checks to pass first (`bench/window.ts` has those).
 *
 * **Its patience is longer than any of the job's own** (#6198): a `project switch` waits 60 s
 * for its paint once the project is in front, after the switcher and the strip have had their
 * turn, and a job that gives up says what it was waiting on and what the window showed. With
 * the same 60 s here, this always ran out first, by a second or two, and said only that the job
 * never finished.
 */
export async function jobInTheWindow<T>(plan: Plan): Promise<T> {
  await browser.execute((plan) => (window.charterBench as Bench).begin(plan), plan);
  let state: { running: boolean; result?: unknown; trouble?: string } = { running: true };
  await browser.waitUntil(
    async () => {
      state = await browser.execute(() => (window.charterBench as Bench).poll());
      return !state.running;
    },
    { timeout: 90_000, interval: 50, timeoutMsg: `${plan.kind} never finished` },
  );
  if (state.trouble) throw new Error(`${plan.kind}: ${state.trouble}`);
  return state.result as T;
}

/** The project in front, by its path, as the strip says, read in one pass (`reading.ts`). */
export async function inFront(): Promise<string | null> {
  const tabs = await attributesOfEach(`${PROJECTS} [role="tab"]`, [
    "data-directory",
    "aria-selected",
  ]);
  return tabs.find((tab) => tab["aria-selected"] === "true")?.["data-directory"] ?? null;
}

/**
 * A project of the run's own, named `switch-<n>` so its tab, its `×` and its row say which it
 * is, with nothing to put back — so opening it starts nothing. Answered with the spelling the
 * app holds it by (`projects.e2e.ts` says why that is the resolved path).
 */
export function aProjectOfItsOwn(n: number, prepare: (plane: string) => void = () => {}): string {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), `switch-${n}`);
  renameSync(copied, renamed);
  anEmptyRecord(renamed);
  prepare(renamed);
  return realpathSync(renamed);
}

/** Opens `plane` from the strip's `+`, through the trust gate, and waits for it in front. */
export async function openFromTheStrip(plane: string): Promise<void> {
  await $(`${PROJECTS} button[aria-label="Open a project…"]`).click();
  const box = await $('[data-setting="open-by-path"] input');
  await box.waitForDisplayed({ timeout: 20_000 });
  await box.addValue(plane);
  // The path box's own submit, found by its form, and the trust question's act found
  // in the question: the two are both named Open project.
  await $('form.by-path button[type="submit"]').click();
  await openThroughTheTrustQuestion();
  await browser.waitUntil(async () => (await inFront()) === plane, {
    timeout: 30_000,
    timeoutMsg: `${plane} never came to the front`,
  });
}

/**
 * **Opens `plane` and waits until its first-open baseline has taken the doctor's first report**
 * (#5648, ADR 0119): what a project holds the first time it opens on a machine counts as seen,
 * so a spec that needs a doctor's finding to be new opens its project this way, with nothing to
 * find, and then makes the finding and {@link asksTheDoctorAgain}. The core's record says when
 * the window has set that report's findings aside (`findings` is no longer null), which the
 * status line cannot: its label may still be the last project's.
 */
export async function openedWithTheDoctorHeard(plane: string): Promise<void> {
  await openFromTheStrip(plane);
  let findings: unknown = null;
  await browser
    .waitUntil(
      async () => {
        ({ findings } = await ask<{ findings: string[] | null }>("inbox_first_open", { plane }));
        return findings !== null;
      },
      { timeout: 60_000, interval: 500 },
    )
    .catch(() => {
      throw new Error(`the window never set aside the doctor's first report for ${plane}`);
    });
}

/** **Runs the doctor again for the project in front**, as the person does: its status item opens
 *  the Doctor dialog, which checks again in full; the dialog is closed once it has answered.
 *  Answers the rows it found that are not ok, for a failure to name. */
export async function asksTheDoctorAgain(): Promise<string> {
  await $('[data-testid="status-doctor"]').click();
  const dialog = $('[role="dialog"]');
  await dialog.waitForDisplayed({ timeout: 20_000 });
  await browser.waitUntil(
    async () => {
      const said = await dialog.getText();
      return said.includes("each harness profile probed") && !said.includes("Checking again");
    },
    { timeout: 90_000, interval: 250, timeoutMsg: "the full doctor never answered" },
  );
  // What it found, for a spec's failure to say: the rows that are not ok.
  const found = await browser.execute(() =>
    [...document.querySelectorAll<HTMLElement>('[role="dialog"] li.doctor-row:not(.doctor-ok)')]
      .map((row) => row.innerText.replace(/\s+/g, " ").trim())
      .join("; "),
  );
  await escapeUntilClosed(() => dialog.isDisplayed());
  return found;
}

/**
 * **Keeps what the window showed, for a failure to be read**: a picture of the window and the
 * Notices view's markup, beside the run's logs (`LOGS`, kept with the run), named by `what`.
 * Answers where they are, for the failure's message.
 */
export async function keepTheNotices(what: string): Promise<string> {
  mkdirSync(LOGS, { recursive: true });
  const picture = join(LOGS, `${what}.png`);
  const markup = join(LOGS, `${what}.html`);
  await browser.saveScreenshot(picture).catch(() => undefined);
  const html = await browser.execute(() =>
    [
      ...document.querySelectorAll<HTMLElement>(
        '[role="tabpanel"][data-view="notices"], [data-cause^="doctor-finding"]',
      ),
    ]
      .map((one) => one.outerHTML)
      .join("\n\n<!-- next -->\n\n"),
  );
  writeFileSync(markup, html);
  return `${picture}, ${markup}`;
}

/**
 * Closes every `switch-<n>` project from its rail button, answering the question a project with a chat
 * open asks. The strip collapses rather than scrolling, so a project it was hiding comes onto
 * it as the drawn ones go — `endEveryChat`'s loop one level up, and bounded the same way.
 */
export async function closeTheProjectsOfItsOwn(): Promise<void> {
  const closer = `${PROJECTS} [role="tab"][aria-label^="switch-"]`;
  for (let pressed = 0; pressed < OPEN; pressed++) {
    if (!(await $(closer).isExisting())) return;
    await closeProject(closer);
  }
  throw new Error(`${OPEN} presses did not close every project this run opened`);
}

/**
 * Brings each project to the front `rounds` times, in the order given, through the switcher,
 * and answers with how long each took. The one in front when it starts must be the LAST one
 * given, so that no switch is ever to the project already there.
 *
 * @param sentinel What the chat in front in each project has printed, for its pane's paint.
 * @param job The window's own job runner, which the benchmark and the scenario run each have.
 */
export async function measureSwitches(
  planes: readonly string[],
  rounds: number,
  sentinel: (plane: string) => string,
  job: <T>(plan: Plan) => Promise<T>,
): Promise<Switches> {
  const painted: number[] = [];
  const shown: number[] = [];
  const opening: number[] = [];
  for (let round = 0; round < rounds; round++) {
    for (const plane of planes) {
      const took = await job<{ ms: number; shownMs: number; openMs: number }>({
        kind: "project switch",
        plane,
        sentinel: sentinel(plane),
      });
      expect(await inFront()).toBe(plane);
      painted.push(took.ms);
      shown.push(took.shownMs);
      opening.push(took.openMs);
    }
  }
  return summarise(planes.length, painted, shown, opening);
}
