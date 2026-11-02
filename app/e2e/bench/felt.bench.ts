// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cpSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { browser } from "@wdio/globals";
import { built, declareAProfile } from "../harness.js";
import { ROWS, frameTime, heldTo, lateAnswers } from "../feltRows.js";
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
  watchFrames,
} from "./window.js";

/** Agents streaming behind the chat in front while frames are timed (L11). */
const STREAMING_FOR_FRAMES = 20;
/** Busy chats while switching (L12, L13): the chat in front and forty-nine streaming. */
const BUSY = 50;
/** Samples per timed row. */
const SAMPLES = 20;
/** Keystrokes typed the moment each switch has painted: all inside its first second (L13). */
const KEYS_AFTER_A_SWITCH = 3;
/** Personas in the project the dialog is opened in: the symptom grew with them (L14). */
const PERSONAS = 200;

/**
 * **The five rows for what the operator feels** (#4744; L11 to L15 of the spec's budgets of
 * record), each measured under the load the operator felt it under, with the fake harness.
 *
 * - **L11**, frames while twenty agents stream in tabs behind an idle chat;
 * - **L12**, switching back to the chat in front with fifty chats busy;
 * - **L13**, the keystrokes typed the moment each of those switches has painted;
 * - **L14**, the new-chat dialog's open, in a project with two hundred personas, the fifty
 *   still streaming in the first project;
 * - **L15**, no command answered late (1 s or longer) all the while, read off the window's own
 *   slow log (`slow.jsonl`, #4725).
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
    const since = Date.now();
    await browser.setWindowSize(1600, 1000);

    // L11: twenty agents streaming behind an idle chat.
    nextLoad(["--corpus", corpus, "--loops", "1000000", "--interval-ms", "4"]);
    const streaming: string[] = [];
    for (let n = 0; n < STREAMING_FOR_FRAMES; n++) streaming.push((await openTab()).tab);
    nextLoad(["--synthetic", "4096", "--sentinel", "FELT-HERE", "--interactive"]);
    const front = await openTab();
    await frame(front.session);
    await sleep(2000);
    await watchFrames();
    await sleep(10_000);
    results["L11 frames, 20 streaming behind"] = frameTime((await frames()).gapsMs);

    // L12 and L13: fifty busy, switching back to the chat in front and typing at once.
    nextLoad(["--corpus", corpus, "--loops", "1000000", "--interval-ms", "4"]);
    while (streaming.length < BUSY - 1) streaming.push((await openTab()).tab);
    // Something to paint on the way back: an echo the chat in front holds.
    await job({ kind: "select tab", tab: front.tab });
    await job({ kind: "keystrokes", count: 1 });
    const switches: number[] = [];
    const echoes: number[] = [];
    for (let n = 0; n < SAMPLES; n++) {
      await job({ kind: "select tab", tab: streaming[n % streaming.length] });
      const back = await job<{ ms: number; session: number }>({
        kind: "switch",
        tab: front.tab,
        sentinel: "you said:",
      });
      switches.push(back.ms);
      const keys = await job<{ samples_ms: number[] }>({
        kind: "keystrokes",
        count: KEYS_AFTER_A_SWITCH,
        session: back.session,
      });
      echoes.push(...keys.samples_ms);
    }
    results["L12 switch at 50 busy"] = heldTo(ROWS.switchAtFiftyBusy, switches);
    results["L13 echo after a switch"] = heldTo(ROWS.echoAfterASwitch, echoes);

    // L14: the new-chat dialog, in a project of many personas, the fifty still streaming.
    const many = aProjectOfItsOwn(100, (plane) => {
      declareAProfile(plane, built("fake-harness"));
      manyPersonas(plane, PERSONAS);
    });
    await openFromTheStrip(many);
    const opens: number[] = [];
    for (let n = 0; n < SAMPLES; n++) {
      opens.push((await job<{ ms: number }>({ kind: "dialog open", press: "New tab" })).ms);
      await closeTheDialog();
    }
    results["L14 new-chat dialog open"] = heldTo(ROWS.dialogOpen, opens);
    await closeTheProjectsOfItsOwn();

    // L15: every answer, read off the window's slow log.
    const log = join(LOGS, "slow.jsonl");
    results["L15 no late answers"] = lateAnswers(
      existsSync(log) ? readFileSync(log, "utf8") : undefined,
      since,
    );

    await closeEverything();
    const missed = Object.entries(results)
      .filter(([, row]) => !(row as { met: boolean }).met)
      .map(([name]) => name);
    if (missed.length > 0) throw new Error(`missed: ${missed.join(", ")}`);
  });
});

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
