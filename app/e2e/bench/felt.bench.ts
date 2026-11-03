// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { browser } from "@wdio/globals";
import { built, declareAProfile } from "../harness.js";
import {
  P95_SAMPLES,
  type Phase,
  ROWS,
  burstOf,
  frameTime,
  heldTo,
  lateAnswers,
  switchPlan,
} from "../feltRows.js";
import { nextLoad } from "../load.js";
import { LOGS } from "../processes.js";
import { aProjectOfItsOwn, closeTheProjectsOfItsOwn, openFromTheStrip } from "../switching.js";
import {
  closeEverything,
  corpus,
  frame,
  frames,
  job,
  openTab,
  ready,
  record,
  sleep,
  type,
  watchFrames,
} from "./window.js";

/** Agents streaming behind the chat in front while frames are timed (L11). */
const STREAMING_FOR_FRAMES = 20;
/** Chats open while switching (L12, L13): the chat in front, the targets, the rest streaming. */
const BUSY = 50;
/**
 * The chats L12 switches to in turn: more than the window keeps (`KEPT`), so none is kept when
 * switched back to, and even, so their bursts alternate 2 MB and 13 MB (`switchPlan`).
 */
const TARGETS = 6;
/** Samples per timed row: enough for a p95 that is not the worst sample. */
const SAMPLES = P95_SAMPLES;
/** Keystrokes typed the moment each switch has painted: all inside its first second (L13). */
const KEYS_AFTER_A_SWITCH = 3;
/** Personas in the project the dialog is opened in: the symptom grew with them (L14). */
const PERSONAS = 200;

/**
 * **The five rows for what the operator feels** (#4744; L11 to L15 of the spec's budgets of
 * record), each measured under the load the operator felt it under, with the fake harness.
 *
 * - **L11**, frames while twenty agents stream in tabs behind an idle chat;
 * - **L12**, switching to a chat that has just taken a 2 MB or 13 MB burst while hidden and is not
 *   among the last three shown (so not kept), six such in turn, with forty-three streaming;
 * - **L13**, the keystrokes typed the moment each of those switches has painted;
 * - **L14**, the new-chat dialog's open, in a project with two hundred personas, the chats still
 *   streaming in the first project;
 * - **L15**, no command answered late (1 s or longer) while any of those was measured, read off
 *   the window's own slow log (`slow.jsonl`, #4725). The bench's own setup (fifty chats started,
 *   a project opened) is left out, and a window that never wrote its log does not meet it.
 *
 * Each row is recorded with `met` and the spec fails after recording them when one is missed,
 * as `projects.bench.ts` does: `tools/bench.mjs` prints each `met` or `MISSED`
 * (`tools/window-verdicts.mjs`) and keeps its exit status (ADR 0086: a release absolute miss is
 * a bug filed before the release notes).
 */
describe("what the operator feels, with fifty chats busy", () => {
  const results: Record<string, unknown> = {};

  before(ready);
  after(() => record("felt", results));

  it("holds frames, switches, echo, the new-chat dialog and every answer to their rows", async () => {
    await browser.setWindowSize(1600, 1000);
    const logWritten = await theSlowLogIsWritten();
    const phases: Phase[] = [];

    // L11: twenty agents streaming behind an idle chat.
    nextLoad(["--corpus", corpus, "--loops", "1000000", "--interval-ms", "4"]);
    const streaming: string[] = [];
    for (let n = 0; n < STREAMING_FOR_FRAMES; n++) streaming.push((await openTab()).tab);
    nextLoad(["--synthetic", "4096", "--sentinel", "FELT-HERE", "--interactive"]);
    const front = await openTab();
    await frame(front.session);
    await sleep(2000);
    let from = Date.now();
    await watchFrames();
    await sleep(10_000);
    results["L11 frames, 20 streaming behind"] = frameTime((await frames()).gapsMs);
    phases.push({ name: "L11", from, to: Date.now() });

    // L12 and L13: switching to a chat that has just taken a burst while hidden and is not kept,
    // and typing in it at once, the rest streaming.
    const targets: { tab: string; done: string }[] = [];
    for (let t = 0; t < TARGETS; t++) {
      const done = join(BURSTS_DONE, `${t}`);
      nextLoad([
        "--corpus",
        corpus,
        "--loops",
        `${burstOf(t)}`,
        "--wait-for-input",
        "--sentinel",
        `FELT-BURST-${t}`,
        "--hook",
        `touch "${done}"`,
        "--interactive",
        "--again",
        "burst",
      ]);
      targets.push({ tab: (await openTab()).tab, done });
    }
    nextLoad(["--corpus", corpus, "--loops", "1000000", "--interval-ms", "4"]);
    while (streaming.length < BUSY - TARGETS - 1) streaming.push((await openTab()).tab);
    const plan = switchPlan(SAMPLES, TARGETS, streaming.length);
    const asked = new Set<number>();
    const switches: number[] = [];
    const echoes: number[] = [];
    from = Date.now();
    for (const step of plan) {
      const target = targets[step.target];
      // Its burst, asked for while it is in front: the first line typed starts it, `burst` again.
      rmSync(target.done, { force: true });
      const shown = await job<{ session: number }>({ kind: "select tab", tab: target.tab });
      await type(shown.session, asked.has(step.target) ? "burst\r" : "\r");
      asked.add(step.target);
      for (const away of step.away) await job({ kind: "select tab", tab: streaming[away] });
      await browser.waitUntil(async () => existsSync(target.done), {
        timeout: 120_000,
        interval: 100,
        timeoutMsg: `target ${step.target}'s burst never ended`,
      });
      const back = await job<{ ms: number; session: number }>({
        kind: "switch",
        tab: target.tab,
        sentinel: `FELT-BURST-${step.target}`,
      });
      switches.push(back.ms);
      const keys = await job<{ samples_ms: number[] }>({
        kind: "keystrokes",
        count: KEYS_AFTER_A_SWITCH,
        session: back.session,
      });
      echoes.push(...keys.samples_ms);
    }
    phases.push({ name: "L12 and L13", from, to: Date.now() });
    const bursts = { burstLoops: plan.map((step) => step.loops), streaming: streaming.length };
    results["L12 switch at 50 busy"] = { ...heldTo(ROWS.switchAtFiftyBusy, switches), ...bursts };
    results["L13 echo after a switch"] = { ...heldTo(ROWS.echoAfterASwitch, echoes), ...bursts };

    // L14: the new-chat dialog, in a project of many personas, the rest still streaming.
    const many = aProjectOfItsOwn(100, (plane) => {
      declareAProfile(plane, built("fake-harness"));
      manyPersonas(plane, PERSONAS);
    });
    await openFromTheStrip(many);
    const opens: number[] = [];
    from = Date.now();
    for (let n = 0; n < SAMPLES; n++) {
      opens.push((await job<{ ms: number }>({ kind: "dialog open", press: "New tab" })).ms);
      await closeTheDialog();
    }
    phases.push({ name: "L14", from, to: Date.now() });
    results["L14 new-chat dialog open"] = heldTo(ROWS.dialogOpen, opens);
    await closeTheProjectsOfItsOwn();

    // L15: every answer while a row was measured, read off the window's slow log.
    results["L15 no late answers"] = {
      ...lateAnswers(logWritten ? readFileSync(SLOW_LOG, "utf8") : undefined, phases),
      phases,
    };

    await closeEverything();
    const missed = Object.entries(results)
      .filter(([, row]) => !(row as { met: boolean }).met)
      .map(([name]) => name);
    if (missed.length > 0) throw new Error(`missed: ${missed.join(", ")}`);
  });
});

/** Where the window writes its slow log in a run (`CHARTER_LOG_DIR`, `harness.ts`). */
const SLOW_LOG = join(LOGS, "slow.jsonl");

/** Where each target's harness says its burst has ended, by a hook: one file each. */
const BURSTS_DONE = join(tmpdir(), "charter-bench", "felt-bursts");
mkdirSync(BURSTS_DONE, { recursive: true });

/**
 * Whether the window writes its slow log where L15 reads it: the window's thread is held for
 * 400 ms, a stall the log must then hold. Without it, a log that was never written would read
 * as a run with no late answer (#4744's review).
 */
async function theSlowLogIsWritten(): Promise<boolean> {
  const from = Date.now();
  await browser.execute(() => {
    const until = performance.now() + 400;
    while (performance.now() < until) {
      // The window's thread held, as a slow command would hold it.
    }
  });
  try {
    await browser.waitUntil(
      async () =>
        existsSync(SLOW_LOG) &&
        readFileSync(SLOW_LOG, "utf8")
          .split("\n")
          .some((line) => {
            try {
              const moment = JSON.parse(line) as { kind?: unknown; at?: unknown };
              return moment.kind === "stall" && typeof moment.at === "number" && moment.at >= from;
            } catch {
              return false;
            }
          }),
      { timeout: 10_000, interval: 200 },
    );
    return true;
  } catch {
    return false;
  }
}

/** `count` personas in `plane`, each a copy of its steward under a name of its own. */
function manyPersonas(plane: string, count: number): void {
  const steward = join(plane, "personas", "steward");
  for (let n = 0; n < count; n++) {
    const name = `felt-${String(n).padStart(3, "0")}`;
    const to = join(plane, "personas", name);
    cpSync(steward, to, { recursive: true });
    const charter = join(to, "persona.md");
    writeFileSync(
      charter,
      readFileSync(charter, "utf8").replace(/^name: steward$/m, `name: ${name}`),
    );
  }
}

/** Closes the new-chat dialog by Escape, and waits until it has gone. */
async function closeTheDialog(): Promise<void> {
  await browser.execute(() => {
    (document.activeElement ?? document.body).dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    );
  });
  await browser.waitUntil(
    async () => browser.execute(() => document.querySelector('[role="dialog"]') === null),
    { timeout: 10_000, timeoutMsg: "the new-chat dialog never shut on Escape" },
  );
}
