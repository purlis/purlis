// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, realpathSync, renameSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { $, $$, browser, expect } from "@wdio/globals";
import { THE_RUNS_TREE, anEmptyRecord, copyFixturePlane } from "../harness.js";
import { closeProject, noDialogIsLeftUp, openThroughTheTrustQuestion } from "../opening.js";
import { attributesOfEach } from "../reading.js";
import { projectInFront, runningIn } from "../running.js";
import { endChatById, startAChat } from "../started.js";

/**
 * A window holding more than one project, in the built app (ADR 0033, decision 23).
 *
 * **What this is for.** The operator asked for Zed's project tabs by name, and the reason he
 * wanted one window per project before that was the same one: fifty chats in project A must
 * not be torn down because he glanced at project B. So the claim under test is not "a second
 * tab appears" — it is that the first project's sessions are *still the same sessions* after
 * the window has been somewhere else and come back. `running_sessions` is asked before and
 * after, because that is the only answer a tab strip cannot fake.
 *
 * It also pins the half of decision 28 a scenario run can reach. Restoring at a COLD launch
 * needs a second launch, and a run shares one app process — so what is proved here is the
 * other end of that record: the window writes down what it holds, into the real machine
 * store, and `planes_to_restore` reads it back. Whether the next launch opens what comes back
 * is `App`'s, in `src/Projects.test.tsx`; the store's own round trip is `machine.rs`'s.
 *
 * **It leaves the window as it found it.** Specs share one app process and run in name order:
 * the second project is closed at the end, the chat this spec opened is closed with it, and
 * the project the launch opened is never touched.
 */

const PROJECTS = '[data-strip="Projects"]';
const TABS = '[data-strip="Tabs"]';

/**
 * A project of this spec's own, under a name of its own, spelled the way charter will.
 *
 * **Renamed**, because `copyFixturePlane` names its copy after the fixture and the launch's
 * project is a copy of the same one: two tabs both reading `daily`, and a `Close project
 * daily` button that could be either. The path tells them apart on screen; the name has to
 * tell them apart for the driver.
 *
 * **Resolved**, because `Planes::open` canonicalises every root it takes and a `PlaneId` IS
 * that spelling — so on macOS, where `tmpdir()` is `/var/folders/…` and `/var` is a link into
 * `/private`, the path this spec would otherwise compare against is not the one the app is
 * holding. That is the same defect `PlaneId`'s own docstring is about, arrived at from a test.
 */
const second = (() => {
  const copied = copyFixturePlane();
  const renamed = join(dirname(copied), "second");
  renameSync(copied, renamed);
  return realpathSync(renamed);
})();

/**
 * Where the strip's *New project…* button is asked to write one — a directory that is not
 * there yet, which is the case that dialog is for.
 *
 * **Inside `THE_RUNS_TREE` and nowhere else.** That tree is `$CHARTER_PLANE_FENCE` for every
 * charter process this run starts, and the app under test is built with the fence compiled in
 * (charter-app#129). A plane scaffolded outside it does not quietly pollute somebody's real
 * control plane; the app dies naming it, in the job that did it — which is the arrangement
 * #132 bought after a run wrote 49 chats into the operator's own plane.
 */
const made = join(THE_RUNS_TREE, "made-by-the-window");

/** What the app answered a command with, insisting it answered at all. */
async function ask<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
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
 * The project tabs, as the strip shows them: the path each names, and which is in front.
 *
 * Read in one pass, because the strip is polled while projects open and close on it, and a tab
 * found and then read can be gone in between (charter#506, `reading.ts`).
 */
async function strip(): Promise<{ path: string | null; front: boolean }[]> {
  const tabs = await attributesOfEach(`${PROJECTS} [role="tab"]`, [
    "data-directory",
    "aria-selected",
  ]);
  return tabs.map((tab) => ({
    // The path, not the name: the tab carries the whole path because two projects can share
    // a directory name, and that is exactly the case this spec sets up.
    path: tab["data-directory"],
    front: tab["aria-selected"] === "true",
  }));
}

/** How many chat tabs the project in front is showing. */
async function chatTabs(): Promise<number> {
  return (await $$(`${TABS} [role="tab"]`).getElements()).length;
}

/** Waits for the strip to look like `want`, so nothing here races a click. */
async function stripBecomes(want: { path: string | null; front: boolean }[]): Promise<void> {
  await browser.waitUntil(async () => JSON.stringify(await strip()) === JSON.stringify(want), {
    timeout: 30_000,
    timeoutMsg: `the project strip never became ${JSON.stringify(want)}`,
  });
}

/**
 * Waits until the store holds `want` as the window set, and answers with the whole of it.
 *
 * **Waited for rather than asked once.** The window says what it holds by an `invoke` nothing
 * awaits — it is bookkeeping, not an action — so a read taken the instant a click settles is
 * a read taken before the write. `opener.e2e.ts` waits for the same class of reason.
 */
async function remembers(
  want: string[],
  /** The one in front, where it matters: the store is written after each switch, so a read
   *  that only waited for the list could see the one before it. */
  inFront?: number,
): Promise<{ planes: string[]; active: number | null; dropped: string[] }> {
  let back = { planes: [] as string[], active: null as number | null, dropped: [] as string[] };
  await browser.waitUntil(
    async () => {
      // The main window's share: every scenario here holds its projects in one window.
      const raw = await ask<{
        windows: { planes: string[]; active: number | null }[];
        dropped: string[];
      }>("planes_to_restore");
      back = {
        planes: raw.windows[0]?.planes ?? [],
        active: raw.windows[0]?.active ?? null,
        dropped: raw.dropped,
      };
      return (
        JSON.stringify(back.planes) === JSON.stringify(want) &&
        (inFront === undefined || back.active === inFront)
      );
    },
    {
      timeout: 30_000,
      timeoutMsg: `charter never came to remember ${JSON.stringify(want)}`,
    },
  );
  return back;
}

/** Waits until the project in front shows `many` chat tabs. */
async function chatTabsBecome(many: number): Promise<void> {
  let saw = -1;
  await browser.waitUntil(
    async () => {
      saw = await chatTabs();
      return saw === many;
    },
    {
      timeout: 30_000,
      timeoutMsg: `the project in front showed ${saw} chat tabs, not ${many}`,
    },
  );
}

describe("a window holding more than one project", function () {
  // On the describe, where WebdriverIO reads it before the runnable is built — a
  // `this.timeout()` inside a test body is silently ignored (`e2e/budget.test.ts`). Opening a
  // second project reads its settings, its reopen record and this machine's store, and then
  // repaints the whole window; three minutes is mocha's own default and is plenty.
  this.timeout(180_000);

  /** The project the launch opened, which this spec never closes. */
  let first = "";
  /** What it had running before this spec touched anything. */
  let running: number[] = [];
  /** How many chat tabs it was showing then. Read rather than assumed: specs share one app
   *  process and run in name order, so whatever ran before this one decides the number. */
  let chatsBefore = 0;
  /** The chat this spec started, by the number the core gave it, which it owes a close; or
   *  none, where the launch's project already ran one. */
  let mySession: number | undefined;
  /** Whether this spec pressed for a chat at all: set BEFORE the press, so a start that throws
   *  once its session is up (its pane never drawn, say) still leaves `after` a chat to end. */
  let pressed = false;

  before(async () => {
    // No record of its own, so opening it starts nothing: this spec is about tabs, and a chat
    // appearing in the second project would be a session the next spec inherits.
    anEmptyRecord(second);
    first = (await ask<string[]>("open_planes"))[0];
    // At least one chat RUNNING in the launch's project, so "the first project kept its
    // sessions" is a claim about something rather than about an empty list.
    //
    // **Asked of the core, not counted on the strip** (#1941). This used to start a chat only
    // where the strip drew no tab, and wait for a tab — then assert the core's
    // `running_sessions`. A tab is not a running chat: the strip also holds view tabs, and the
    // tab of a chat the guard ended by id (`chatsLeft.ts`: that end takes no tab away), so an
    // earlier spec's leftovers skipped the start and the list came back empty; and a fresh tab
    // can be drawn before its session is listed. So the decision and the wait are both on the
    // list the assertion reads: `startAChat` waits (bounded) until the core lists the new chat
    // and its pane is in front.
    running = await runningIn(first);
    if (running.length === 0) {
      // `startAChat` starts in the project in front: that has to be `first`, or the chat is
      // started in one project and read and ended in another.
      expect(await projectInFront()).toBe(first);
      pressed = true;
      const started = await startAChat("New tab");
      expect(started.plane).toBe(first);
      mySession = started.session;
      running = await runningIn(first);
    }
    // Read once the strip has drawn what it will: a chat this spec started has its pane in front
    // by now (`startAChat`). A stale read could only make `chatTabsBecome(chatsBefore)` below
    // time out (bounded, and failing), never pass on a wrong count.
    chatsBefore = await chatTabs();
    expect(running.length).toBeGreaterThan(0);
  });

  after(async () => {
    // The window as it was found. Nothing of either project goes from disk.
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first)) {
      await ask("close_plane", { plane });
    }
    // Its own chat, by its number and through the window, never the strip's first `×`: that
    // can be a chat another spec left (`started.ts`). It answers the question ending asks, so
    // no modal is handed to the next spec file — one app process serves the whole run. Where
    // the start threw before naming its chat, `first` ran nothing before the press, so every
    // chat it runs now is this spec's.
    if (!pressed) return;
    const mine = mySession === undefined ? await runningIn(first) : [mySession];
    for (const session of mine) await endChatById(session, first);
  });

  it("opens a second project beside the first, through the same trust gate", async () => {
    await stripBecomes([{ path: first, front: true }]);

    // `+` on the strip, which is the catalogue's own row drawn as a button.
    //
    // **By its accessible name, not by its position.** It was `${PROJECTS} > button`, the one
    // direct-child button of the strip — which stopped matching anything the moment the
    // strip's own controls were gathered into an element of their own, and a selector that
    // matches nothing is a spec that fails with "element not displayed" and says nothing
    // about why. The `+` carries the catalogue's words in `aria-label` precisely so that it
    // can be reached by what it means, and that is what an operator gets from it too.
    await $(`${PROJECTS} button[aria-label="Open a project…"]`).click();
    const box = await $('[data-setting="open-by-path"] input');
    await box.waitForDisplayed({ timeout: 20_000 });
    await box.addValue(second);
    // The path box's own submit, found by its form, and the trust question's act found
    // in the question: the two are both named Open project.
    await $('form.by-path button[type="submit"]').click();

    // **A tab is an open, and an open goes through the gate.** A project nobody has approved
    // is described and not opened; there is no third way in (ADR 0035).
    await openThroughTheTrustQuestion();

    await stripBecomes([
      { path: first, front: false },
      { path: second, front: true },
    ]);
    // The second project is what is on screen now, and it has nothing open: its record was
    // emptied, so opening it started nothing.
    await chatTabsBecome(0);
  });

  it("leaves the first project's sessions running while the second is in front", async () => {
    // The whole reason the operator wanted one window per project, which tabs have to keep
    // rather than trade away. The tab strip cannot prove it; the core's own list can.
    expect(await runningIn(first)).toEqual(running);
  });

  it("brings the first project back with its chats, having ended none of them", async () => {
    const tabs = await $$(`${PROJECTS} [role="tab"]`).getElements();
    await tabs[0].click();

    await stripBecomes([
      { path: first, front: true },
      { path: second, front: false },
    ]);
    await chatTabsBecome(chatsBefore);
    expect(await runningIn(first)).toEqual(running);
  });

  it("writes down what it holds, so a cold launch can put it back", async () => {
    // The other end of decision 28, which is as far as one app process reaches: the window
    // said what it holds, the real machine store took it, and the core reads it back in the
    // order the tabs are in.
    const back = await remembers([first, second], 0);

    expect(back.active).toBe(0);
    expect(back.dropped).toEqual([]);
  });

  it("closes one project without disturbing the other", async () => {
    await closeProject(`${PROJECTS} button[aria-label="Close project ${basename(second)}"]`);

    await stripBecomes([{ path: first, front: true }]);
    expect(await ask<string[]>("open_planes")).toEqual([first]);
    expect(await runningIn(first)).toEqual(running);
    // And what it writes down follows, so the next launch does not put back a project the
    // operator closed.
    await remembers([first]);
  });

  /**
   * The other button on the strip: one that makes a project instead of finding one
   * (charter-app#178).
   *
   * **Last, and it leaves the strip as it found it**, because every spec above it is about the
   * two projects it set up and this one adds a third to the same window.
   *
   * **It writes for real**, which is the only reason this is a scenario test at all:
   * `NewProject.test.tsx` already proves the dialog, the command it sends and the gate it ends
   * at, all against a mocked core. What no mocked core can show is that the button an operator
   * presses reaches `scaffold::init` and that a directory appears with a plane in it. So the
   * folder is a path that does not exist yet, under `THE_RUNS_TREE` — the one tree this run
   * may write in, and the fence every charter process it starts is held to (charter-app#129,
   * #132). A plane made anywhere else would kill the app, in this job, naming it.
   */
  it("makes a project from the strip's other button, and opens it through the same gate", async () => {
    // Whatever happens below, the window is left as this file found it: a miss here used to leave
    // a dialog, or a half-made project, in front of every spec after it (#1949's scenario stall).
    // The test's own failure is the one reported: a cleanup that fails after it is only said.
    let failed: unknown;
    try {
      await $(`${PROJECTS} button[aria-label="New project…"]`).click();

      const dialog = await $('[role="dialog"]');
      await dialog.waitForDisplayed({ timeout: 20_000 });
      await expect(dialog).toHaveText("New project", { containing: true });
      // The folder is under Advanced since FR-4: the dialog's default asks for a repo, and the
      // two-directory form is a closed `<details>`, opened by its summary.
      await $('[role="dialog"] details.advanced > summary').click();
      // Typed rather than picked: Browse… opens the system's own folder dialog, which is not a
      // thing a driver inside the webview can answer. The first box under Advanced is the folder
      // — React owns the id, so the element is taken by position, as `workspace-lifecycle.e2e.ts`
      // takes the workspace name out of its sibling dialog.
      await $('[role="dialog"] details.advanced input').setValue(made);
      await $("button=Create project").click();
      // An empty folder has no repo whose remote could say which forge (#839): the dialog asks,
      // and the answer makes the project.
      await $("button=GitHub").click();

      // **A plane charter made a second ago is still one this machine has approved nothing
      // about** (ADR 0035). There is no shortcut past the ask, and this is the assertion that
      // says so from the outside. The New project dialog is still up, saying "Creating…", when
      // the press lands, and the question takes its place: waited for by its words (#2126).
      await openThroughTheTrustQuestion();

      // The operator's second ruling on #178: it opens in THIS window, selected. Pressing create
      // was the asking; a second dialog would be a third confirmation of something asked twice.
      await stripBecomes([
        { path: first, front: false },
        { path: realpathSync(made), front: true },
      ]);
      expect(existsSync(join(made, "charter.toml"))).toBe(true);

      await closeProject(`${PROJECTS} button[aria-label="Close project ${basename(made)}"]`);
      await stripBecomes([{ path: first, front: true }]);
    } catch (error) {
      failed = error;
    }
    let cleanupFailed: unknown;
    try {
      await noDialogIsLeftUp();
      const left = `${PROJECTS} button[aria-label="Close project ${basename(made)}"]`;
      if (await $(left).isExisting()) await closeProject(left);
    } catch (cleanup) {
      cleanupFailed = cleanup;
    }
    if (failed !== undefined) {
      if (cleanupFailed !== undefined)
        console.error("charter-e2e: the cleanup after this failure failed too:", cleanupFailed);
      throw failed;
    }
    if (cleanupFailed !== undefined) throw cleanupFailed;
  });
});
