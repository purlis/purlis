// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";
import { PROMPTS } from "../src/interruptBudget.ts";
import { recordFailure } from "./failures.js";
import { specOf } from "./chatsLeft.js";

/**
 * **Every scenario flow counts its interruptions, and fails above its ceiling** (ADR 0121
 * decision 18, #6021; takes over #1079): each test of every spec file is a flow, and while it
 * runs the window counts what stopped the person, three ways:
 *
 * - **prompts**: every dialog, alert dialog and question shown, as W10's interrupt budget counts
 *   them (`src/interruptBudget.ts`, whose selector this shares);
 * - **asks**: each time the title bar's ✋ rose, a chat newly waiting on the person;
 * - **notifications**: each system notification purlis sent (`asknotify::NOTIFIED`, told to the
 *   window holding the project).
 *
 * A flow may cost at most its {@link Ceiling}: {@link DEFAULT_CEILING}, W10's three prompts (DS-9,
 * #631) and #1079's budget for every way back to a chat, unless {@link CEILINGS} names its file
 * with a reason. The root hooks (`leftBehind.ts`) count each test; a file whose tests went over
 * fails in its `after all`, naming each test and what it cost, so one costly flow fails its
 * own file and no test after it.
 *
 * **What it does not see**: a page reloaded during a flow loses the window's tally, and that flow
 * goes uncounted (said in the run's output, never a failure); a native dialog, which is not on the
 * page; and a notification about a project no window holds.
 */

/** What a flow may cost. */
export type Ceiling = { prompts: number; asks: number; notifications: number };

/** What one flow cost. */
export type Counted = Ceiling;

/** **Every flow's ceiling** where its file names none: W10's three prompts (#631, #1079). */
export const DEFAULT_CEILING: Ceiling = { prompts: 3, asks: 3, notifications: 1 };

/**
 * **The files whose flows may cost more**, each with why. A ceiling is raised here only with a
 * reason a reader can check against the spec; a flow over its ceiling is otherwise a new
 * interruption the change brought, and it is the change that is wrong.
 */
export const CEILINGS: Readonly<Record<string, { ceiling: Partial<Ceiling>; why: string }>> = {};

/** The ceiling of every flow in spec file `spec`. */
export function ceilingOf(spec: string): Ceiling {
  const name = spec.slice(spec.lastIndexOf("/") + 1);
  return { ...DEFAULT_CEILING, ...CEILINGS[name]?.ceiling };
}

/** What `counted` costs past `ceiling`, by name; empty where it is within it. */
export function overCeiling(counted: Counted, ceiling: Ceiling): string[] {
  return (["prompts", "asks", "notifications"] as const)
    .filter((kind) => counted[kind] > ceiling[kind])
    .map((kind) => `${counted[kind]} ${kind} (ceiling ${ceiling[kind]})`);
}

/** What the guard says of a file whose flows went over their ceilings. */
export function overSaid(spec: string, over: readonly { flow: string; past: string[] }[]): string {
  const shown = spec.includes("specs/") ? spec.slice(spec.lastIndexOf("specs/")) : spec;
  const flows = over.map((one) => `"${one.flow}": ${one.past.join(", ")}`).join("; ");
  return (
    `${shown}: ${over.length === 1 ? "a flow interrupts" : `${over.length} flows interrupt`} ` +
    `more than its ceiling allows: ${flows}. Take the interruption out, or raise the file's ` +
    `ceiling in e2e/interrupts.ts with the reason (#6021).`
  );
}

/** The window's tally, under this name on `window`. */
const TALLY = "__PURLIS_E2E_INTERRUPTS__";

/** The event a notification sent is told to the window by (`asknotify::NOTIFIED`). */
const NOTIFIED = "asks-notified";

/** The title bar's ✋: its words are how many chats wait on the person. */
const HAND = '[data-testid="title-bar"] [data-testid="needs-you-button"]';

/** Starts the window's tally afresh: what is up now is not this flow's. */
async function startCounting(): Promise<void> {
  await browser.execute(
    (tally: string, prompts: string, hand: string, notified: string) => {
      type Held = Counted & { up: Set<string>; hand: number; stop: () => void };
      const host = window as unknown as Record<string, Held | undefined>;
      host[tally]?.stop();
      const promptsUp = (): Set<string> => {
        const now = new Set<string>();
        for (const one of document.querySelectorAll(prompts)) {
          const role =
            one.getAttribute("role") ??
            (one.localName === "fieldset" ? "group" : one.localName === "dialog" ? "dialog" : "");
          const name = (
            one.getAttribute("aria-label") ??
            (one.getAttribute("aria-labelledby") ?? "")
              .split(/\s+/)
              .filter(Boolean)
              .map((id) => document.getElementById(id)?.textContent ?? "")
              .join(" ")
          ).trim();
          if ((role === "group" || role === "radiogroup") && !name.endsWith("?")) continue;
          now.add(`${role}|${name}`);
        }
        return now;
      };
      const handNow = () =>
        Number.parseInt(document.querySelector(hand)?.textContent ?? "", 10) || 0;
      const held: Held = {
        prompts: 0,
        asks: 0,
        notifications: 0,
        up: promptsUp(),
        hand: handNow(),
        stop: () => {},
      };
      const look = () => {
        const now = promptsUp();
        for (const key of now) if (!held.up.has(key)) held.prompts += 1;
        held.up = now;
        const count = handNow();
        if (count > held.hand) held.asks += count - held.hand;
        held.hand = count;
      };
      const observer = new MutationObserver(look);
      observer.observe(document.body, {
        childList: true,
        subtree: true,
        characterData: true,
        attributes: true,
        attributeFilter: ["role", "aria-label", "aria-labelledby", "open"],
      });
      let unlisten: (() => void) | undefined;
      void window.__TAURI__.event
        .listen(notified, () => {
          held.notifications += 1;
        })
        .then((stop) => {
          unlisten = stop;
        });
      held.stop = () => {
        look();
        observer.disconnect();
        unlisten?.();
      };
      host[tally] = held;
    },
    TALLY,
    PROMPTS,
    HAND,
    NOTIFIED,
  );
}

/** What the window counted since {@link startCounting}; none where the page was loaded again. */
async function counted(): Promise<Counted | null> {
  return browser.execute((tally: string) => {
    const held = (
      window as unknown as Record<string, (Counted & { stop: () => void }) | undefined>
    )[tally];
    if (held === undefined) return null;
    held.stop();
    return { prompts: held.prompts, asks: held.asks, notifications: held.notifications };
  }, TALLY);
}

/** The flows of the file running now that went over their ceilings. */
let over: { flow: string; past: string[] }[] = [];

/** **The guard's root hooks**: each test counted, each file judged (`leftBehind.ts`). */
export const interruptHooks = {
  beforeAll(): void {
    over = [];
  },

  async beforeEach(): Promise<void> {
    await startCounting().catch(() => undefined);
  },

  async afterEach(this: Mocha.Context): Promise<void> {
    const test = this.currentTest;
    if (test === undefined) return;
    const cost = await counted().catch(() => null);
    const flow = test.fullTitle();
    if (cost === null) {
      console.warn(`charter-e2e: "${flow}" went uncounted: the page was loaded again (#6021).`);
      return;
    }
    const past = overCeiling(cost, ceilingOf(test.file ?? ""));
    if (past.length > 0) over.push({ flow, past });
  },

  // Async as every guard's is, so `leftBehind.ts` calls each alike.
  async afterAll(this: Mocha.Context): Promise<void> {
    if (over.length === 0) return;
    const spec = specOf(this.test?.parent);
    const error = new Error(overSaid(spec, over));
    over = [];
    recordFailure({ title: '"after all" hook: interruptions', parent: "", file: spec }, error);
    throw error;
  },
};
