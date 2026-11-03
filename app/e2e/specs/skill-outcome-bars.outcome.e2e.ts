// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { appendFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { $, $$, browser, expect } from "@wdio/globals";
import { projectRoot } from "../acting.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";
import {
  OPENINGS,
  SKILLS_AT_THE_BAR,
  type Effort,
  harnessUnderTest,
  openVerdict,
  reasonProblem,
  useVerdict,
} from "../outcomeBars.js";
import {
  closeTheViewTabsOpenedSince,
  focusWorkspace,
  runFromThePalette,
  tabNames,
} from "../reach.js";
import { runningIn } from "../running.js";
import { paneInFront, startAChat } from "../started.js";

/**
 * **The Skill studio's outcome bars** (SK-23, #2786; #4343), in the built app against the real
 * core, once per harness on its stand-in (`wdio.outcome.conf.ts`). Whether a real harness behaves
 * as its stand-in does is the operator's run before the flip.
 *
 * - **Under 2 minutes from wanting a skill to using it in a running chat**, with no files or
 *   terminal: the test touches the disk only after its clock has stopped, and everything it does
 *   is a press or a typed character, each counted for what a person would spend on it.
 * - **No skill state without a one-line reason**: a skill in each state a plane's files can put it
 *   in, in the Skills view and the studio's Library. That every reason the core has reads as one
 *   line, the two a persona's chat alone meets among them, is `purlis_skills`'s test
 *   (`every_reason_reads_as_one_line`).
 * - **The studio opens in under 300 ms with 200 skills**: from the press of the palette's row to
 *   the frame after the Library's first row is drawn (row L10 of the budgets of record).
 *
 * The same file runs on every harness: only the line a skill is typed as differs.
 */

const harness = harnessUnderTest(process.env.PURLIS_OUTCOME_HARNESS);
const MADE = "e2e-bars-wanted";
/** The studio of the workspace the chats are in: the root's has no skill turned off or ungranted. */
const STUDIO_ROW = "skills.studio:alpha";
/** That row, as the palette draws it. */
const STUDIO_OF_ALPHA = `palette-row-${STUDIO_ROW}`;
/** The first of the two hundred, by name, so the first row the Library draws. */
const crowd = (n: number) => `e2e-bars-crowd-${String(n).padStart(3, "0")}`;

/** One skill in each state, with the file beside it that puts it there. */
const NOT_GRANTED = "e2e-state-not-granted";
const TURNED_OFF = "e2e-state-turned-off";
const ARCHIVED = "e2e-state-archived";
const NARROWER = "e2e-state-narrower-copy";
const NO_DESCRIPTION = "e2e-state-no-description";
const UNSUPPORTED = "e2e-state-unsupported";
const STATES = [NOT_GRANTED, TURNED_OFF, ARCHIVED, NARROWER, NO_DESCRIPTION, UNSUPPORTED];
/** Where the Library draws each state's left-out row: the first copy of a name has the plain
 *  testid, so a narrower copy's wider one is the second (`SkillStudioTab`). */
const rowOf = (state: string) => (state === NARROWER ? `${state}-hidden-1` : state);

/** What the skills above say they do, which is no reason for the state they are in. */
const WHAT_IT_DOES = [
  "Granted elsewhere.",
  "Turned off here.",
  "Archived.",
  "The wider copy.",
  "Widens.",
];

/** Says a bar's verdict on the run's page as well as its log, whichever way it went. */
function report(said: string): void {
  console.log(`outcome bar (${harness.kind}): ${said}`);
  const summary = process.env.GITHUB_STEP_SUMMARY;
  if (summary) appendFileSync(summary, `- **${harness.kind}**: ${said}\n`);
}

/** Puts a skill in the project's `skills/`, with `beside` in the file its level keeps beside it. */
function aProjectSkill(root: string, name: string, front: string, beside = 'grant = "all"\n') {
  const folder = join(root, "skills", name);
  mkdirSync(folder, { recursive: true });
  writeFileSync(join(folder, "SKILL.md"), `---\nname: ${name}\n${front}---\n\nSay hello.\n`);
  writeFileSync(join(root, "skills", `${name}.toml`), beside);
}

/**
 * Runs the palette's row `id` ({@link runFromThePalette}), counting what a person does: the key
 * that opens it, what is typed and the press. A chat's row ends with that chat's number (#4753);
 * a workspace's, such as its studio's, has none.
 */
async function fromThePalette(typed: string, id: string, effort: Effort): Promise<void> {
  await runFromThePalette(typed, id);
  effort.presses += 2;
  effort.characters += typed.length;
}

/** Presses the button that says `said` in `scope`, once it can be pressed. */
async function press(said: string, effort: Effort, scope = "body"): Promise<void> {
  const button = await $(scope).$(`button=${said}`);
  await button.waitForEnabled({ timeout: 20_000 });
  await button.click();
  effort.presses += 1;
}

async function typeInto(label: string, text: string, effort: Effort): Promise<void> {
  const box = await $(`aria/${label}`);
  await box.waitForDisplayed({ timeout: 10_000 });
  await box.addValue(text);
  effort.characters += text.length;
  effort.presses += 1;
}

/** The chat in front's Skills view, its Left out unfolded. */
async function theSkillsViewUnfolded(): Promise<void> {
  const chip = await $('[data-testid="pane-skills"]');
  await chip.waitForDisplayed({ timeout: 20_000 });
  await chip.click();
  await $('[data-testid="skills-view"]').waitForDisplayed({ timeout: 20_000 });
  const fold = await $('[data-fold="left-out:fold"]');
  await fold.waitForExist({
    timeout: 20_000,
    timeoutMsg: "the Skills view has no Left out to unfold",
  });
  if ((await fold.getAttribute("data-expanded")) === null) await fold.click();
  await browser.waitUntil(async () => (await fold.getAttribute("data-expanded")) !== null, {
    timeout: 10_000,
    timeoutMsg: "Left out never unfolded",
  });
}

/** **Opens the studio from the palette `OPENINGS` times**, each a new tab that reads the project
 *  afresh, and answers how long each took to draw its first row. */
async function theOpenings(wereOpen: readonly string[]): Promise<number[]> {
  const samples: number[] = [];
  for (let n = 0; n < OPENINGS; n++) {
    await closeTheViewTabsOpenedSince(wereOpen);
    const already = await $('[data-testid="skill-studio"]').isExisting();
    if (already) throw new Error("the studio's tab is still open, so this would time no opening");
    // [DEBUG-4351] A/B: 1 drops the tabs layout's :has() rule, 2 keeps #root from going inert, 3 both.
    const variant = n % 4; // [DEBUG-4351]
    await browser.execute((v: number) => {
      const g = globalThis as {
        __variant4351?: number;
        __inertPatched4351?: boolean;
        __dropped4351?: { sheet: CSSStyleSheet | CSSGroupingRule; at: number; text: string }[];
      };
      g.__variant4351 = v;
      if (!g.__inertPatched4351) {
        g.__inertPatched4351 = true;
        const d = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "inert");
        if (d?.set !== undefined) {
          const set = d.set;
          Object.defineProperty(HTMLElement.prototype, "inert", {
            ...d,
            set(this: HTMLElement, on: boolean) {
              const w = (globalThis as { __variant4351?: number }).__variant4351 ?? 0;
              if (this.id === "root" && w >= 2) return;
              set.call(this, on);
            },
          });
        }
      }
      for (const one of (g.__dropped4351 ?? []).reverse()) one.sheet.insertRule(one.text, one.at);
      g.__dropped4351 = [];
      if (v % 2 === 1) {
        const walk = (list: CSSRuleList, owner: CSSStyleSheet | CSSGroupingRule) => {
          for (let i = list.length - 1; i >= 0; i--) {
            const r = list[i];
            if (r instanceof CSSStyleRule && r.selectorText.includes('[data-part="layout"]:has(')) {
              g.__dropped4351?.push({ sheet: owner, at: i, text: r.cssText });
              owner.deleteRule(i);
            } else if ("cssRules" in r) walk((r as CSSGroupingRule).cssRules, r as CSSGroupingRule);
          }
        };
        for (const sheet of Array.from(document.styleSheets)) {
          try {
            walk(sheet.cssRules, sheet);
          } catch {
            // a sheet from elsewhere
          }
        }
      }
      return (g.__dropped4351 ?? []).length;
    }, variant); // [DEBUG-4351]
    await browser.keys(["F2"]);
    const box = await $("#palette-query");
    await box.waitForDisplayed({ timeout: 10_000 });
    await box.addValue("Open Skill studio");
    await $(`[id="${STUDIO_OF_ALPHA}"]`).waitForExist({ timeout: 20_000 });
    // The clock is the window's own, started in the same task as the press, and stopped by the
    // frame after the row is drawn: so WebDriver's round trips are in neither end of it.
    const armed = await browser.execute(
      (id: string, css: string, snapThis: boolean) => {
        const row = document.getElementById(id);
        if (row === null) return false;
        const holder = globalThis as { __openedInMs?: number };
        holder.__openedInMs = undefined;
        // [DEBUG-4351] a timeline of this opening: each core call and the DOM's milestones.
        const tl: string[] = []; // [DEBUG-4351]
        (globalThis as { __tl4351?: string[] }).__tl4351 = tl; // [DEBUG-4351]
        const internals = (
          globalThis as {
            __TAURI_INTERNALS__?: {
              invoke: (...a: unknown[]) => Promise<unknown>;
              __orig4351?: unknown;
            };
          }
        ).__TAURI_INTERNALS__; // [DEBUG-4351]
        if (internals !== undefined) {
          // [DEBUG-4351]
          const orig = (internals.__orig4351 ?? internals.invoke) as (
            ...a: unknown[]
          ) => Promise<unknown>; // [DEBUG-4351]
          try {
            // [DEBUG-4351]
            internals.__orig4351 = orig; // [DEBUG-4351]
            internals.invoke = (...a: unknown[]) => {
              // [DEBUG-4351]
              const at = performance.now(); // [DEBUG-4351]
              const name = String(a[0]); // [DEBUG-4351]
              const t = (globalThis as { __tl4351?: string[] }).__tl4351; // [DEBUG-4351]
              t?.push(`${(at - started).toFixed(0)} send ${name}`); // [DEBUG-4351]
              const back = orig.apply(internals, a); // [DEBUG-4351]
              void back.then(
                () =>
                  t?.push(
                    `${(performance.now() - started).toFixed(0)} back ${name} (${(performance.now() - at).toFixed(0)} ms)`,
                  ),
                () => t?.push(`${(performance.now() - started).toFixed(0)} fail ${name}`),
              ); // [DEBUG-4351]
              return back; // [DEBUG-4351]
            }; // [DEBUG-4351]
            tl.push(internals.invoke === orig ? "invoke not patched" : "invoke patched"); // [DEBUG-4351]
          } catch (e) {
            tl.push(`invoke patch failed ${String(e)}`);
          } // [DEBUG-4351]
        } // [DEBUG-4351]
        const seen = new Set<string>(); // [DEBUG-4351]
        const watch = new MutationObserver(() => {
          // [DEBUG-4351]
          for (const [label, q] of [
            ["tab", '[data-testid="skill-studio"]'],
            ["palette gone", "#palette-query"],
            ["row", css],
          ] as const) {
            // [DEBUG-4351]
            const here = document.querySelector(q) !== null; // [DEBUG-4351]
            const key = `${label}:${here}`; // [DEBUG-4351]
            if (!seen.has(key) && (label === "palette gone" ? !here : here)) {
              seen.add(key);
              tl.push(`${(performance.now() - started).toFixed(0)} dom ${label}`);
            } // [DEBUG-4351]
          } // [DEBUG-4351]
        }); // [DEBUG-4351]
        watch.observe(document.body, { childList: true, subtree: true }); // [DEBUG-4351]
        let frames = 0; // [DEBUG-4351]
        let last = performance.now(); // [DEBUG-4351]
        const frame = () => {
          const now = performance.now();
          if (now - last > 50)
            tl.push(`${(now - started).toFixed(0)} frame gap ${(now - last).toFixed(0)} ms`);
          last = now;
          if (++frames < 120 && holder.__openedInMs === undefined) requestAnimationFrame(frame);
          else watch.disconnect();
        }; // [DEBUG-4351]
        requestAnimationFrame(frame); // [DEBUG-4351]
        // [DEBUG-4351] where the thread goes: a task kept running through a MessageChannel, and each
        // frame callback; a pause over 30 ms says what ran before it (task: script; frame: then the
        // browser's style, layout and paint) and what ran after.
        let lastAt = performance.now(); // [DEBUG-4351]
        let lastKind = "start"; // [DEBUG-4351]
        const mark = (kind: string) => {
          const now = performance.now();
          if (now - lastAt > 30)
            tl.push(
              `${(now - started).toFixed(0)} pause ${(now - lastAt).toFixed(0)} ms ${lastKind}->${kind}`,
            );
          lastAt = now;
          lastKind = kind;
        }; // [DEBUG-4351]
        const channel = new MessageChannel(); // [DEBUG-4351]
        let ticks = 0; // [DEBUG-4351]
        channel.port1.onmessage = () => {
          mark("task");
          if (++ticks < 4000 && holder.__openedInMs === undefined) channel.port2.postMessage(0);
        }; // [DEBUG-4351]
        channel.port2.postMessage(0); // [DEBUG-4351]
        const rafMark = () => {
          mark("frame");
          if (holder.__openedInMs === undefined) requestAnimationFrame(rafMark);
        }; // [DEBUG-4351]
        requestAnimationFrame(rafMark); // [DEBUG-4351]
        // [DEBUG-4351] every read that forces a layout, timed, with who asked: patched once.
        const w = globalThis as { __reflowPatched?: boolean }; // [DEBUG-4351]
        if (!w.__reflowPatched) {
          // [DEBUG-4351]
          w.__reflowPatched = true; // [DEBUG-4351]
          const styleOf = window.getComputedStyle.bind(window); // [DEBUG-4351]
          let inside = false; // [DEBUG-4351]
          const labelOf = (el: Element) => {
            const d = (el as HTMLElement).dataset ?? {};
            const cls = typeof el.className === "string" ? el.className.split(" ")[0] : "";
            const aria = (el.getAttribute("aria-label") ?? "").slice(0, 24);
            return `${el.tagName.toLowerCase()}${el.id ? "#" + el.id : ""}${cls ? "." + cls : ""}${d.scope ? "[" + d.scope + "/" + (d.part ?? "") + "]" : ""}${aria ? '"' + aria + '"' : ""}`;
          }; // [DEBUG-4351]
          const census = () => {
            const all = document.body.getElementsByTagName("*").length;
            const heavy: string[] = [];
            const walk = (el: Element, depth: number) => {
              const n = el.getElementsByTagName("*").length;
              if (n < 150 || depth > 14) return;
              const kids = Array.from(el.children);
              const big = kids.find((k) => k.getElementsByTagName("*").length >= n * 0.6);
              if (big) return walk(big, depth + 1);
              const shown =
                (el as Element & { checkVisibility?: () => boolean }).checkVisibility?.() ?? true;
              heavy.push(`${labelOf(el)}=${n}${shown ? "" : " hidden"}`);
              for (const k of kids) walk(k, depth + 1);
            };
            walk(document.body, 0);
            return `census ${all} elements, sheets ${document.styleSheets.length}: ${heavy.slice(0, 14).join(", ")}`;
          }; // [DEBUG-4351]
          const styleFirst = () => {
            if (inside) return 0;
            inside = true;
            const g = globalThis as {
              __snapOn4351?: boolean;
              __snaps4351?: { name: string; html: string }[];
              __frame4351?: string;
              __t04351?: number;
            }; // [DEBUG-4351]
            const pre = g.__snapOn4351 ? document.documentElement.outerHTML : ""; // [DEBUG-4351]
            const t0 = performance.now();
            void styleOf(document.documentElement).color;
            const ms = performance.now() - t0;
            if (g.__snapOn4351 && ms >= 40 && g.__snaps4351 !== undefined) {
              const at = (t0 - (g.__t04351 ?? t0)).toFixed(0);
              g.__snaps4351.push({
                name: `${at}-${ms.toFixed(0)}ms-lastframe`,
                html: g.__frame4351 ?? "",
              });
              g.__snaps4351.push({ name: `${at}-${ms.toFixed(0)}ms-pre`, html: pre });
            } // [DEBUG-4351]
            inside = false;
            return ms;
          }; // [DEBUG-4351]
          const note = (name: string, ms: number, styleMs = 0, self?: unknown) => {
            const t = globalThis as {
              __tl4351?: string[];
              __t04351?: number;
              __census4351?: boolean;
            };
            if (ms + styleMs < 3 || t.__tl4351 === undefined || t.__t04351 === undefined) return;
            const at = self instanceof Element ? ` on ${labelOf(self)}` : ""; // [DEBUG-4351]
            if (ms + styleMs >= 40 && !t.__census4351) {
              t.__census4351 = true;
              t.__tl4351.push(census());
            } // [DEBUG-4351]
            const stack = (new Error().stack ?? "")
              .split("\n")
              .slice(2, 7)
              .map((one) =>
                one
                  .trim()
                  .replace(/@?(https?|tauri):\/\/[^ ]*\//, "@")
                  .slice(0, 60),
              )
              .join(" < ");
            t.__tl4351.push(
              `${(performance.now() - t.__t04351).toFixed(0)} reflow ${name} style ${styleMs.toFixed(0)} layout ${ms.toFixed(0)} ms${at}: ${stack}`,
            );
          }; // [DEBUG-4351]
          const wrapFn = (proto: object, name: string) => {
            const was = (proto as Record<string, unknown>)[name] as (...a: unknown[]) => unknown;
            if (typeof was !== "function") return;
            (proto as Record<string, unknown>)[name] = function (this: unknown, ...a: unknown[]) {
              const styleMs = styleFirst(); // [DEBUG-4351]
              const t0 = performance.now();
              const r = was.apply(this, a);
              note(name, performance.now() - t0, styleMs, this);
              return r;
            };
          }; // [DEBUG-4351]
          const wrapGet = (proto: object, name: string) => {
            const d = Object.getOwnPropertyDescriptor(proto, name);
            if (d?.get === undefined) return;
            const get = d.get;
            Object.defineProperty(proto, name, {
              ...d,
              get(this: unknown) {
                const styleMs = styleFirst(); // [DEBUG-4351]
                const t0 = performance.now();
                const r = get.call(this);
                note(name, performance.now() - t0, styleMs, this);
                return r;
              },
            });
          }; // [DEBUG-4351]
          for (const name of ["getBoundingClientRect", "getClientRects", "scrollIntoView", "focus"])
            wrapFn(Element.prototype, name); // [DEBUG-4351]
          wrapFn(HTMLElement.prototype, "focus"); // [DEBUG-4351]
          wrapFn(window as unknown as object, "getComputedStyle"); // [DEBUG-4351]
          for (const name of [
            "offsetWidth",
            "offsetHeight",
            "offsetTop",
            "offsetLeft",
            "offsetParent",
          ])
            wrapGet(HTMLElement.prototype, name); // [DEBUG-4351]
          for (const name of [
            "clientWidth",
            "clientHeight",
            "scrollWidth",
            "scrollHeight",
            "scrollTop",
          ])
            wrapGet(Element.prototype, name); // [DEBUG-4351]
        } // [DEBUG-4351]
        const started = performance.now();
        (globalThis as { __t04351?: number }).__t04351 = started; // [DEBUG-4351]
        (globalThis as { __census4351?: boolean }).__census4351 = false; // [DEBUG-4351]
        const snap = globalThis as {
          __snapOn4351?: boolean;
          __snaps4351?: { name: string; html: string }[];
          __frame4351?: string;
          __css4351?: string;
        }; // [DEBUG-4351]
        snap.__snapOn4351 = snapThis; // [DEBUG-4351]
        if (snapThis) {
          snap.__css4351 = Array.from(document.styleSheets)
            .map((sheet) => {
              try {
                return Array.from(sheet.cssRules)
                  .map((r) => r.cssText)
                  .join("\n");
              } catch {
                return "";
              }
            })
            .join("\n");
          snap.__frame4351 = document.documentElement.outerHTML;
          snap.__snaps4351 = [{ name: "0-before-click", html: snap.__frame4351 }];
          const keep = () => {
            if (!snap.__snapOn4351) return;
            snap.__frame4351 = document.documentElement.outerHTML;
            requestAnimationFrame(keep);
          };
          requestAnimationFrame(keep);
        } // [DEBUG-4351]
        row.click();
        tl.push(`${(performance.now() - started).toFixed(0)} click returned`); // [DEBUG-4351]
        const look = () => {
          if (document.querySelector(css) === null) requestAnimationFrame(look);
          else
            requestAnimationFrame(() => {
              holder.__openedInMs = performance.now() - started;
            });
        };
        requestAnimationFrame(look);
        return true;
      },
      STUDIO_OF_ALPHA,
      `[data-testid="skill-${crowd(0)}"]`,
      false, // [DEBUG-4351] no snapshots this run
    );
    expect(armed).toBe(true);
    let took: number | undefined;
    await browser.waitUntil(
      async () => {
        took = await browser.execute(() => (globalThis as { __openedInMs?: number }).__openedInMs);
        return took !== undefined && took !== null;
      },
      { timeout: 30_000, interval: 100, timeoutMsg: "the studio's Library drew no row in 30 s" },
    );
    samples.push(took as number);
    const tl = await browser.execute(() => (globalThis as { __tl4351?: string[] }).__tl4351 ?? []); // [DEBUG-4351]
    console.log(
      `[DEBUG-4351] opening ${n} variant ${variant} took ${Math.round(took as number)} ms: ${(tl as string[]).join(" | ")}`,
    ); // [DEBUG-4351]
    if (n === 2) {
      const got = (await browser.execute(() => {
        const g = globalThis as {
          __snapOn4351?: boolean;
          __snaps4351?: { name: string; html: string }[];
          __css4351?: string;
        };
        g.__snapOn4351 = false;
        return { snaps: g.__snaps4351 ?? [], css: g.__css4351 ?? "" };
      })) as { snaps: { name: string; html: string }[]; css: string }; // [DEBUG-4351]
      const dir = join(process.cwd(), "logs", `dom4351-${harness.kind}`); // [DEBUG-4351]
      mkdirSync(dir, { recursive: true }); // [DEBUG-4351]
      writeFileSync(join(dir, "window.css"), got.css); // [DEBUG-4351]
      for (const one of got.snaps) writeFileSync(join(dir, `${one.name}.html`), one.html); // [DEBUG-4351]
      console.log(`[DEBUG-4351] wrote ${got.snaps.length} snapshots to ${dir}`); // [DEBUG-4351]
    } // [DEBUG-4351]
  }
  return samples;
}

describe(`the Skill studio's outcome bars on ${harness.kind}'s stand-in`, function () {
  this.timeout(480_000);

  let root = "";
  let wereRunning: Running = {};

  after(async () => {
    await endTheChatsStartedSince(wereRunning);
  });

  before(async () => {
    root = await projectRoot();
    // The Skills chip is left out of a narrow pane.
    await browser.setWindowSize(1440, 900);
    await focusWorkspace("alpha");
    wereRunning = await chatsRunning();
  });

  it("goes from wanting a skill to using it in a running chat in under 2 minutes", async () => {
    const chat = await startAChat();
    // A running chat at its prompt, as a person has one: the reporting stand-ins hold their first
    // turn until a line is typed, and a restart waits for a turn to end.
    if (harness.kind !== "claude") await $(".xterm-helper-textarea").addValue("\n");
    const effort: Effort = { presses: 0, characters: 0 };
    const beforeTheStudio = await tabNames();
    const started = Date.now();

    await fromThePalette("Open Skill studio", STUDIO_ROW, effort);
    await press("New skill…", effort, '[data-testid="skill-studio"]');
    await press("Write it yourself", effort, '[data-testid="skill-new"]');
    await typeInto("Name", MADE, effort);
    await typeInto("What it does, and when a chat should use it", "Says hello.", effort);
    await press("Make it and open its page", effort);
    await $(`[data-testid="skill-${MADE}"]`).waitForExist({ timeout: 20_000 });

    // Back to the chat: the studio's tab closed, as a person would.
    await closeTheViewTabsOpenedSince(beforeTheStudio);
    effort.presses += 1;
    await browser.waitUntil(async () => (await paneInFront()) === chat.session, {
      timeout: 20_000,
      timeoutMsg: "the chat never came back to the front",
    });

    // A chat keeps the skills it started with until it restarts on the same conversation (decision
    // 15), while Use skill… lists what the folders hold now: without this restart the line typed
    // below would name a skill the harness was never handed. So the Notice is required.
    const notice = await $(`[data-cause="skills-changed:${chat.session}"]`);
    await notice.waitForExist({
      timeout: 30_000,
      timeoutMsg: `the running chat was never offered a restart for its new skill`,
    });
    // A note, folded into the pane's bell while notices are minimised, the window's default
    // (#4754): a person presses the bell to see it, a press counted like any other.
    const bell = await $(
      `.pane-frame:has([data-cause="skills-changed:${chat.session}"]) [data-way="bell"]`,
    );
    const folded = (await bell.getAttribute("aria-expanded")) === "false";
    if (folded) {
      await bell.click();
      effort.presses += 1;
      await notice.waitForDisplayed({ timeout: 10_000 });
    }
    await notice.$("button=Restart").click();
    effort.presses += 1;
    await notice.waitForExist({ timeout: 40_000, reverse: true }).catch(async () => {
      // Said with what the pane says then, so a red run says why the restart did not land.
      const said = await browser.execute(
        (session: number) =>
          document
            .querySelector(`[data-testid="pane"][data-session="${session}"]`)
            ?.closest('[data-part="root"], section, main')?.textContent ?? "",
        chat.session,
      );
      const statuses = await browser.execute(() =>
        [...document.querySelectorAll('[role="status"], [role="alert"], [role="dialog"]')].map(
          (one) => one.textContent ?? "",
        ),
      );
      throw new Error(
        `the restart never landed. Statuses: ${JSON.stringify(statuses)}. Around the pane: ${said.slice(0, 500)}`,
      );
    });

    // The panes' notices minimised again, as the window's default, for the specs after this one:
    // the person's own tidying, not part of wanting a skill, so no press is counted.
    if (folded) {
      const now = await $('.pane-frame [data-way="bell"][aria-expanded="true"]');
      if (await now.isExisting()) await now.click();
    }

    // The Notice goes as the restart begins. The chat is back once the pane in front is a chat the
    // core runs, and has stayed so: a pick before that types into the program being replaced.
    let steady = 0;
    await browser.waitUntil(
      async () => {
        const front = await paneInFront();
        steady = (await runningIn(chat.plane)).includes(front) ? steady + 1 : 0;
        return steady >= 3;
      },
      { timeout: 40_000, interval: 500, timeoutMsg: "the restarted chat never came back" },
    );

    await fromThePalette("Use skill", `chat.useskill:${await paneInFront()}`, effort);
    await $('[data-testid="use-skill"]').waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(
      async () => {
        for (const row of await $$('[data-testid="use-skill-row"]')) {
          if ((await row.getText()).includes(MADE)) {
            await row.click();
            return true;
          }
        }
        return false;
      },
      { timeout: 40_000, timeoutMsg: `the running chat's Use skill… never listed ${MADE}` },
    );
    effort.presses += 1;

    // The skill takes nothing, so its pick types it; the window says the line, in this harness's
    // syntax, with the chat named.
    const line = harness.typed(MADE);
    await browser
      .waitUntil(
        async () =>
          browser.execute(
            (typed: string) =>
              [...document.querySelectorAll('[role="status"]')].some((one) =>
                (one.textContent ?? "").includes(typed),
              ),
            line,
          ),
        { timeout: 20_000, interval: 250 },
      )
      .catch(async () => {
        const said = await browser.execute(() =>
          [...document.querySelectorAll('[role="status"], [role="alert"], [role="dialog"]')]
            .map((one) => (one.textContent ?? "").slice(0, 200))
            .filter((one) => one !== ""),
        );
        throw new Error(`the window never said it typed ${line}: ${JSON.stringify(said)}`);
      });

    const verdict = useVerdict(Date.now() - started, effort);
    report(verdict.said);
    if (!verdict.met) throw new Error(`missed: ${verdict.said}`);
  });

  describe("with two hundred skills", () => {
    before(async () => {
      // Made at the level its dialog started on, the workspace's. Either way it is not one of the
      // two hundred.
      rmSync(join(root, "workspaces", "alpha", "skills", MADE), { recursive: true, force: true });
      rmSync(join(root, "skills", MADE), { recursive: true, force: true });
      rmSync(join(root, "skills", `${MADE}.toml`), { force: true });
      for (let n = 0; n < SKILLS_AT_THE_BAR - STATES.length; n++)
        aProjectSkill(root, crowd(n), "description: One of the crowd.\n");

      aProjectSkill(root, NOT_GRANTED, "description: Granted elsewhere.\n", 'grant = ["beta"]\n');
      aProjectSkill(root, TURNED_OFF, "description: Turned off here.\n");
      aProjectSkill(
        root,
        ARCHIVED,
        "description: Archived.\n",
        'grant = "all"\nlifecycle = "archived"\n',
      );
      aProjectSkill(root, NARROWER, "description: The wider copy.\n");
      const narrower = join(root, "workspaces", "alpha", "skills", NARROWER);
      mkdirSync(narrower, { recursive: true });
      writeFileSync(
        join(narrower, "SKILL.md"),
        `---\nname: ${NARROWER}\ndescription: The narrower copy.\n---\n\nSay hello.\n`,
      );
      aProjectSkill(root, NO_DESCRIPTION, "");
      aProjectSkill(root, UNSUPPORTED, "description: Widens.\nallowedTools: Bash\n");

      const manifest = join(root, "workspaces", "alpha", "workspace.json");
      const doc = JSON.parse(readFileSync(manifest, "utf8")) as {
        settings?: { skills?: { off?: string[] } };
      };
      writeFileSync(
        manifest,
        JSON.stringify({
          ...doc,
          settings: { ...doc.settings, skills: { ...doc.settings?.skills, off: [TURNED_OFF] } },
        }),
      );
    });

    it("gives a one-line reason for every state, in the Skills view", async () => {
      // The chat the first test used, whose set the view reads afresh. On a slow runner its restart
      // can still be replacing it, which a second start would count as its own chat.
      await theSkillsViewUnfolded();
      const problems: string[] = [];
      // Every row of Left out, and any marked elsewhere: a row of Left out with no reason drawn
      // reads as an empty one, which is the failure this bar is for.
      const left = await browser.execute(() =>
        [...document.querySelectorAll<HTMLElement>('[data-testid^="skills-row-"]')].flatMap(
          (row) => {
            const reason = row.querySelector('[data-part="summary"][data-tone]');
            const inLeftOut = row.closest('[aria-label="Left out"]') !== null;
            if (reason === null && !inLeftOut) return [];
            const name = row.querySelector('[data-part="label"]')?.textContent ?? "";
            return [{ name, said: reason?.textContent ?? "" }];
          },
        ),
      );
      for (const { name, said } of left) {
        const problem = reasonProblem(said);
        if (problem !== undefined) problems.push(`${name}: ${problem}`);
      }
      const named = left.map((one) => one.name);
      // Past the budget of 30 a router hides the most of them: that state has a reason too.
      if (!named.includes(crowd(0))) problems.push(`no skill of the crowd is marked hidden`);
      for (const state of STATES)
        if (!named.includes(state)) problems.push(`${state}: not marked, in ${named.length} rows`);
      report(`reasons in the Skills view: ${left.length} rows, ${problems.length} without one`);
      expect(problems).toEqual([]);
    });

    it("gives a one-line reason for every state, in the studio's Library", async () => {
      await fromThePalette("Open Skill studio", STUDIO_ROW, {
        presses: 0,
        characters: 0,
      });
      await $(`[data-testid="skill-${crowd(0)}"]`).waitForExist({ timeout: 30_000 });
      // The Library draws its first screen, then the rest in the frames after it (#4351).
      await $(`[data-testid="skill-${rowOf(UNSUPPORTED)}"]`).waitForExist({ timeout: 10_000 });
      const problems: string[] = [];
      for (const state of STATES) {
        const row = await $(`[data-testid="skill-${rowOf(state)}"]`);
        if (!(await row.isExisting())) {
          problems.push(`${state}: the Library does not list it`);
          continue;
        }
        // The line under the name: a left-out skill's line has to say why, not what it does.
        const below = await row
          .$('[data-part="below"]')
          .getText()
          .catch(() => "");
        console.log(`outcome bar (${harness.kind}): ${state}'s row says: ${below}`);
        const problem = reasonProblem(below);
        if (problem !== undefined) problems.push(`${state}: ${problem}`);
        else if (WHAT_IT_DOES.includes(below))
          problems.push(`${state}: its row says what it does, not why: "${below}"`);
      }
      report(`reasons in the Library: ${STATES.length} states, ${problems.length} without one`);
      expect(problems).toEqual([]);
    });

    it("opens the studio in under 300 ms", async () => {
      const verdict = openVerdict(
        await theOpenings((await tabNames()).filter((one) => one !== "Skill studio")),
      );
      report(verdict.said);
      if (!verdict.met) throw new Error(`missed: ${verdict.said}`);
    });
  });
});
