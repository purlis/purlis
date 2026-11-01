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
import { closeTheViewTabsOpenedSince, focusWorkspace, tabNames } from "../reach.js";
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
const STUDIO_OF_ALPHA = "palette-row-skills.studio:alpha";
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

/** Presses the palette's row `id`, found by typing `typed`, counting what a person does. */
async function fromThePalette(typed: string, id: string, effort: Effort): Promise<void> {
  await browser.keys(["F2"]);
  effort.presses += 1;
  const box = await $("#palette-query");
  await box.waitForDisplayed({ timeout: 10_000 });
  await box.addValue(typed);
  effort.characters += typed.length;
  const row = await $(`[id^="${id}"]`);
  await row.waitForExist({ timeout: 20_000, timeoutMsg: `the palette never offered ${typed}` });
  await browser.execute((one: HTMLElement) => one.click(), row);
  effort.presses += 1;
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
    await browser.keys(["F2"]);
    const box = await $("#palette-query");
    await box.waitForDisplayed({ timeout: 10_000 });
    await box.addValue("Open Skill studio");
    await $(`[id="${STUDIO_OF_ALPHA}"]`).waitForExist({ timeout: 20_000 });
    // The clock is the window's own, started in the same task as the press, and stopped by the
    // frame after the row is drawn: so WebDriver's round trips are in neither end of it.
    const armed = await browser.execute(
      (id: string, css: string) => {
        const row = document.getElementById(id);
        if (row === null) return false;
        const holder = globalThis as { __openedInMs?: number };
        holder.__openedInMs = undefined;
        const started = performance.now();
        // DEBUG-4351
        const probe = globalThis as unknown as Record<string, any>;
        const calls: string[] = [];
        probe.__calls = calls;
        const internals = probe.__TAURI_INTERNALS__;
        if (internals.__was === undefined) {
          internals.__was = internals.invoke;
          internals.invoke = (cmd: string, ...rest: unknown[]) => {
            const at = performance.now();
            const answer = internals.__was(cmd, ...rest);
            void Promise.resolve(answer).finally(() => {
              probe.__calls?.push(
                `${cmd} @${Math.round(at - probe.__t0)}+${Math.round(performance.now() - at)}`,
              );
            });
            return answer;
          };
        }
        probe.__t0 = started;
        row.click();
        calls.push(`clicked +${Math.round(performance.now() - started)}`);
        let tab = false;
        const look = () => {
          if (!tab && document.querySelector('[data-testid="skill-studio"]') !== null) {
            tab = true;
            calls.push(`tab drawn +${Math.round(performance.now() - started)}`);
          }
          if (document.querySelector(css) === null) requestAnimationFrame(look);
          else {
            calls.push(`row drawn +${Math.round(performance.now() - started)}`);
            requestAnimationFrame(() => {
              holder.__openedInMs = performance.now() - started;
            });
          }
        };
        requestAnimationFrame(look);
        return true;
      },
      STUDIO_OF_ALPHA,
      `[data-testid="skill-${crowd(0)}"]`,
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
    console.log(
      `DEBUG-4351 ${harness.kind} #${n} ${Math.round(took as number)} ms: ` +
        (await browser.execute(() => (globalThis as any).__calls.join(" | "))),
    );
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

    await fromThePalette("Open Skill studio", STUDIO_OF_ALPHA, effort);
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

    await fromThePalette("Use skill", "palette-row-tab.useskill:", effort);
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
      await fromThePalette("Open Skill studio", STUDIO_OF_ALPHA, {
        presses: 0,
        characters: 0,
      });
      await $(`[data-testid="skill-${crowd(0)}"]`).waitForExist({ timeout: 30_000 });
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
