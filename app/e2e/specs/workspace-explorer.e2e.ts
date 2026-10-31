// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { chmodSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { browser, expect, $, $$ } from "@wdio/globals";
import { menuNames, pressMenuPath, ROW } from "../menus.js";
import { answeringAsClaudeCode, built, READY, singleQuoted } from "../harness.js";
import { harnessRowsDrawn, pressAndStart, pressOnly, showView } from "../opening.js";
import { textOfEach } from "../reading.js";
import { ask } from "../switching.js";
import { chatsRunning, endTheChatsStartedSince, type Running } from "../chatsLeft.js";
import { focusWorkspace, tabNames } from "../reach.js";
import { workspacesListed } from "../titleBarWorkspaces.js";
import { runningIn } from "../running.js";
import { endChatById, paneInFront, startAChat } from "../started.js";

/**
 * The left region — the repo and worktree **explorer** (ADR 0038) — against the real
 * app started in a copy of the `daily` fixture plane, with real clones in it and one real
 * piece cut off `svc`. Nothing here is stubbed: the app reads the files and runs git.
 *
 * This file replaces `sidebar.e2e.ts`. What that spec asserted was the old left sidebar
 * listing every workspace with its vision text, which is the duplication ADR 0038 removed —
 * the strip above is the axis, and the workspace assertions that still matter moved to the
 * strip's own queries below.
 *
 * **Its name is why it runs last, and that is deliberate.** WebdriverIO's Tauri service keeps
 * ONE app process for the whole run, so a spec that starts chats leaves them for every spec
 * after it — and `lifecycle.e2e.ts` counts what a relaunch puts back. `sidebar.e2e.ts` sorted
 * after every other spec in `wdio.conf.ts`'s glob, which is the only reason its two chats were
 * never counted by anything; calling this file `explorer.e2e.ts` sorted it FIRST and turned
 * lifecycle's "2 sessions" into 4, measured on both platforms. The name keeps the position.
 */

/** The workspaces the strip is listing, in order (ADR 0036).
 *
 *  The names alone: a strip tab also says how many chats are in a workspace that is not on
 *  screen, and WebdriverIO's Tauri service keeps ONE app process for the whole run, so a
 *  chat another spec started would otherwise land in this list. */
async function listed(): Promise<string[]> {
  // The title bar's ⌄ lists them (#2916), each read in one pass (`titleBarWorkspaces.ts`).
  return (await workspacesListed()).map((one) => one.name).filter((name) => name !== "Plane root");
}

/** Waits until the strip has read the plane, and says what it found if it never does. */
async function untilListed(expected: string[]): Promise<void> {
  let last: string[] = [];
  try {
    await browser.waitUntil(
      async () => {
        last = await listed();
        return last.join(",") === expected.join(",");
      },
      { timeout: 20_000, interval: 250 },
    );
  } catch {
    throw new Error(
      `the strip never listed ${JSON.stringify(expected)}; it listed ${JSON.stringify(last)}`,
    );
  }
}

/**
 * Puts the window on `alpha`, which is the workspace most of the assertions are about.
 *
 * Focused rather than assumed, for the reason in this file's own docstring: one app process
 * serves the whole run, so what this spec finds depends on what ran before it.
 */
/** The header of the fixture branch's cockpit, and nothing else that names the branch. */
const COCKPIT_HEAD = 'section.cockpit-head[aria-label="Branch fix-login"]';

async function onAlpha(): Promise<void> {
  await untilListed(["alpha", "beta"]);
  await focus("alpha");
  // On the workspace itself, as it starts: a branch an earlier spec picked to see its files
  // (#1677) would be where the next chat starts, and its files what Files draws.
  await browser.execute(() => {
    // The Repos view, in the left half of the Panel since #1737.
    const root = document.querySelector<HTMLElement>('[data-testid="repos-view"] .spot-root');
    if (root !== null && root.getAttribute("aria-current") !== "true") root.click();
  });
}

/**
 * Shows a branch's files in Explorer's *Files* section (#1677): picks the branch's row, which is
 * what the section draws the files of, and waits for the section to be of it. {@link onAlpha}
 * picks the workspace itself again.
 */
async function showFilesOf(piece: string): Promise<void> {
  const row = await $(inExplorer(`.spot[data-testid="piece-svc-${piece}"]`));
  await row.waitForExist({ timeout: 20_000 });
  await row.click();
  await $(inExplorer(`[role="treegrid"][aria-label^="Files of ${piece}"]`)).waitForExist({
    timeout: 20_000,
    timeoutMsg: `Files never showed the files of ${piece}`,
  });
}

/** Focuses a workspace from the title bar's ⌄, by its row's own name (#2916). */
async function focus(workspace: string): Promise<void> {
  await focusWorkspace(workspace);
}

/**
 * Sends the `contextmenu` a right-click sends, to the first element matching `selector`.
 *
 * Answers whether it found one, so a selector that stopped matching fails as itself rather
 * than as a menu that never opened. The coordinates are the element's own centre, because
 * Radix anchors the menu to the point the event carries. The same helper
 * `workspace-lifecycle.e2e.ts` uses, and its docstring has the measurement behind it.
 */
async function sendContextMenu(selector: string): Promise<boolean> {
  return browser.execute((css: string) => {
    const el = document.querySelector(css);
    if (!(el instanceof HTMLElement)) return false;
    const box = el.getBoundingClientRect();
    el.dispatchEvent(
      new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        clientX: Math.round(box.left + box.width / 2),
        clientY: Math.round(box.top + box.height / 2),
      }),
    );
    return true;
  }, selector);
}

describe("the explorer", () => {
  // The chats below start in alpha and in beta (the strip's, the shell's and the raw prompt's),
  // and are ended again through the window, each by its number, in whichever workspace it is:
  // one app process serves the whole run, and a spec file leaves no chat running (#1796,
  // `chatsLeft.ts`).
  let wereOpen: Running = {};
  // The left side opens on Chats (#1673): these are about the Explorer view's rows.
  before(async () => {
    wereOpen = await chatsRunning();
    await showView("Explorer");
  });
  after(async () => {
    // Step by step, each on its own: a chat that would not end still leaves the view put back.
    let trouble: unknown;
    try {
      await endTheChatsStartedSince(wereOpen);
    } catch (error) {
      trouble = error;
    }
    try {
      await showView("Chats");
    } catch (error) {
      trouble ??= error;
    }
    if (trouble !== undefined) throw trouble;
  });

  it("lists the focused workspace's clones", async () => {
    await onAlpha();

    await $('[data-testid="clone-svc"]').waitForExist({ timeout: 20_000 });
    await expect(await $('[data-testid="clone-tool"]')).toBeExisting();
  });

  it("lists the worktrees cut off a clone, with the branch each is on", async () => {
    await onAlpha();

    const cut = await $('[data-testid="piece-svc-fix-login"]');
    await cut.waitForExist({ timeout: 20_000 });
    await expect(cut).toHaveText(expect.stringContaining("fix-login"));
  });

  it("says a worktree carries no charter layer before a chat is started in it", async () => {
    // The fixture's piece is cut with plain git, which is exactly the tree a chat would run
    // in with none of the plane's ask/deny rules, no persona agents and no $PURLIS_HARNESS.
    await onAlpha();

    const cut = await $('[data-testid="piece-svc-fix-login"]');
    await cut.waitForExist({ timeout: 20_000 });
    await expect(cut).toHaveText(expect.stringContaining("unwired"));
  });

  it("answers a right-click on a piece with the catalogue's rows for that piece", async () => {
    // charter-app#174, against the real core: the rows in this menu exist because the plane
    // on disk has `fix-login` cut off `svc` and `worktree_list` said so — nothing here is
    // stubbed, and a catalogue built from a plane that had not been read would draw no menu
    // at all. What the rows SAY is `src/Menus.test.tsx`'s and `src/actions.test.ts`'s.
    //
    // **The event is dispatched rather than right-clicked**, for the reason
    // `workspace-lifecycle.e2e.ts` measures at length: a WebDriver right-click is a
    // synthesised pointer sequence and raises no `contextmenu` on either engine, so what is
    // sent here is the event the platform would have sent.
    await onAlpha();
    await $('[data-testid="piece-svc-fix-login"]').waitForExist({ timeout: 20_000 });

    const sent = await sendContextMenu('.spot[data-testid="piece-svc-fix-login"]');
    expect(sent).toBe(true);

    const menu = await $('[role="menu"]');
    await menu.waitForDisplayed({ timeout: 20_000 });
    // The rows by their names: the menu is about the branch, so a row names only the repo
    // it merges into, which is another thing.
    const rows = (await menuNames())[0];
    expect(rows).toContain("Merge into svc");
    expect(rows).toContain(ROW.removeFolder);
    // Closed again, because one app process serves the whole run and a menu left up is over
    // every row the specs after this one reach for.
    await browser.keys(["Escape"]);
    await menu.waitForDisplayed({ timeout: 20_000, reverse: true });
  });

  it("says a clone has no branches cut rather than drawing nothing under it", async () => {
    await onAlpha();

    // A line of the tree under the clone's row (the tree family, #2911).
    const tool = await $('[data-clone="tool"].none');
    await tool.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(async () => (await tool.getText()).includes("No branches cut"), {
      timeout: 20_000,
      timeoutMsg: "the explorer never said whether `tool` has branches cut",
    });
  });

  /**
   * **A branch expands into its files** (FM-1, #1104), against the real core: `branch_tree`
   * lists the fixture branch's folder with git, and the file opens in its file tab through
   * `piece_file`. Nothing is stubbed.
   */
  it("expands the fixture branch into its files and opens one in its file tab", async () => {
    await onAlpha();
    await showFilesOf("fix-login");
    const readme = await fileRow("fix-login", "README.md");
    await readme.click();

    await browser.waitUntil(async () => (await tabInFront()) === "README.md · fix-login", {
      timeout: 20_000,
      timeoutMsg: `the file tab never came forward; in front is ${await tabInFront()}`,
    });
    // Markdown, so it is drawn rendered (FM-2); `# svc` is its heading.
    const rendered = await $('[data-testid="piece-markdown"]');
    await rendered.waitForDisplayed({ timeout: 20_000 });
    await expect(rendered).toHaveText(expect.stringContaining("svc"));
  });

  /**
   * **⌘P finds a file by fuzzy name** (FM-7, #1110), against the real core: `find_files` lists
   * the picked branch with git and ranks its paths, and the file opens in its file tab. The
   * key is sent as the window's own event, with the platform's modifiers, because a WebDriver
   * chord does not carry them reliably on both engines (#176).
   */
  it("finds a file of the picked branch by fuzzy name on ⌘P and opens it in its file tab", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    // In a folder of this test's own, which it removes again: the fixture's own files stay.
    mkdirSync(join(branch, "found-by-name", "auth"), { recursive: true });
    writeFileSync(
      join(branch, "found-by-name", "auth", "login-form.ts"),
      "export const form = 1;\n",
    );
    // **It leaves the window as it found it**, whatever happens in between: one app process
    // serves the whole run, and a branch left picked here is where the next spec's "New tab"
    // starts its chat — `files a chat under the workspace it was started in` needs nothing
    // picked, and a palette left up would be a dialog over every row after this.
    try {
      const spot = await $('.spot[data-testid="piece-svc-fix-login"]');
      await spot.waitForExist({ timeout: 20_000 });
      await spot.click();

      await browser.execute(() => {
        const mac = navigator.platform.startsWith("Mac");
        window.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "p",
            metaKey: mac,
            ctrlKey: !mac,
            shiftKey: !mac,
            bubbles: true,
            cancelable: true,
          }),
        );
      });
      const scope = await $('[role="status"][aria-label="Where files are found"]');
      await scope.waitForDisplayed({ timeout: 20_000 });
      await expect(scope).toHaveText(expect.stringContaining("Files in branch fix-login"));
      await browser.keys("lgnform");
      const hit = await $('[role="listbox"][aria-label="Files"] [role="option"]');
      await hit.waitForExist({ timeout: 20_000 });
      await expect(hit).toHaveText(expect.stringContaining("login-form.ts"));
      await expect(hit).toHaveText(expect.stringContaining("found-by-name/auth · fix-login"));

      // Tab widens the scope to the project and says so, and the file still leads.
      // The rung past it, every open project, is there only while another project is open,
      // which depends on the specs before this one, so it is `Palette.files.test.tsx`'s.
      await browser.keys(["Tab"]);
      await expect(scope).toHaveText(expect.stringContaining("Files in project"));
      await expect($('[role="listbox"][aria-label="Files"] [role="option"]')).toHaveText(
        expect.stringContaining("found-by-name/auth · fix-login"),
      );

      await browser.keys(["Enter"]);

      await browser.waitUntil(async () => (await tabInFront()) === "login-form.ts · fix-login", {
        timeout: 20_000,
        timeoutMsg: `the file tab never came forward; in front is ${await tabInFront()}`,
      });
      const editor = await $('[data-testid="light-editor"]');
      await editor.waitForDisplayed({ timeout: 20_000 });
      await expect(editor).toHaveText(expect.stringContaining("export const form"));

      // The palette closed on Enter.
      await expect($('[role="dialog"]')).not.toBeDisplayed();
    } finally {
      if (await $('[role="dialog"]').isDisplayed()) await browser.keys(["Escape"]);
      const root = await $('[data-testid="repos-view"] .spot-root');
      await root.click();
      await expect(root).toHaveAttribute("aria-current", "true");
      rmSync(join(branch, "found-by-name"), { recursive: true, force: true });
    }
  });

  /**
   * **⌘⇧F searches the content of the files and opens a hit at its line** (FM-8, #1111), against
   * the real core, in the left side's Search view (#1676; a Search tab before that): `search_files` walks the picked branch in-process and streams its hits, and
   * Enter on the one stepped to opens the branch's file tab with the preview on that line. The
   * key is sent as the window's own event, with the platform's modifiers, for ⌘P's reason
   * (#176).
   */
  it("finds a fixture string on ⌘⇧F and opens the hit at its line in the file tab", async () => {
    // One app serves the whole run: a palette a spec before this one left open is closed first.
    if (await $('[role="dialog"]').isExisting()) await browser.keys(["Escape"]);
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    // In a folder of this test's own, which it removes again: the fixture's own files stay.
    mkdirSync(join(branch, "found-by-content"), { recursive: true });
    writeFileSync(
      join(branch, "found-by-content", "notes.md"),
      "# Notes\n\nnothing here\n\nthe quokka-sentinel lives on line five\n",
    );
    // **It leaves the window as it found it**, for ⌘P's reason above: nothing picked in the
    // explorer, and no dialog up.
    try {
      const spot = await $('.spot[data-testid="piece-svc-fix-login"]');
      await spot.waitForExist({ timeout: 20_000 });
      await spot.click();

      await browser.execute(() => {
        const mac = navigator.platform.startsWith("Mac");
        window.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "F",
            metaKey: mac,
            ctrlKey: !mac,
            shiftKey: true,
            bubbles: true,
            cancelable: true,
          }),
        );
      });
      // The Search view, on the left, and not a tab.
      const view = await $('[role="tabpanel"][data-view="search"]');
      await view.waitForDisplayed({ timeout: 20_000 });
      const box = await view.$('input[type="search"][aria-label="Search the files"]');
      await box.waitForDisplayed({ timeout: 20_000 });
      await expect(
        $('[data-scope="select"][data-part="root"][aria-label="Where to search"]'),
      ).toHaveAttribute("data-value", "branch");
      await box.click();
      await browser.keys("quokka-sentinel");

      const hit = await $('[role="listbox"][aria-label="Search results"] [role="option"]');
      await hit.waitForExist({ timeout: 20_000 });
      await expect(hit).toHaveText(expect.stringContaining("the quokka-sentinel lives"));
      await expect(
        $('[role="listbox"][aria-label="Search results"] [role="group"]'),
      ).toHaveAttribute("aria-label", expect.stringContaining("notes.md, found-by-content"));

      // Into the hits from the box, and the one stepped to opens.
      await browser.keys(["ArrowDown"]);
      await browser.keys(["Enter"]);

      await browser.waitUntil(async () => (await tabInFront()) === "Files · fix-login", {
        timeout: 20_000,
        timeoutMsg: `the file tab never came forward; in front is ${await tabInFront()}`,
      });
      const editor = await $('[data-testid="light-editor"]');
      await editor.waitForDisplayed({ timeout: 20_000 });
      await expect(editor).toHaveText(expect.stringContaining("quokka-sentinel"));
      // The cursor is on the hit's line, which the gutter marks.
      const marked = await $('[data-testid="light-editor"] .cm-activeLineGutter');
      await expect(marked).toHaveText("5");
    } finally {
      if (await $('[role="dialog"]').isDisplayed()) await browser.keys(["Escape"]);
      // Back on the Explorer view, which this describe holds the side on.
      await showView("Explorer");
      const root = await $('[data-testid="repos-view"] .spot-root');
      await root.click();
      await expect(root).toHaveAttribute("aria-current", "true");
      rmSync(join(branch, "found-by-content"), { recursive: true, force: true });
    }
  });

  it("shows a file an agent creates in an expanded folder without a refresh", async () => {
    await onAlpha();
    await showFilesOf("fix-login");
    await fileRow("fix-login", "README.md");

    // What an agent writing in the branch does: a file appears in its folder on disk.
    const plane = (await ask<string[]>("open_planes"))[0];
    writeFileSync(
      join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login", "agent-wrote.md"),
      "written by an agent\n",
    );

    // FSEvents gives no bound on when it delivers on a busy Mac (#577), so the wait is long.
    await fileRow("fix-login", "agent-wrote.md", 60_000);

    // And one it removes goes.
    rmSync(join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login", "agent-wrote.md"));
    await $('[data-row="file:svc/fix-login:agent-wrote.md"]').waitForExist({
      timeout: 60_000,
      reverse: true,
      timeoutMsg: "the explorer still draws a file the agent removed",
    });
  });

  it("shows a file an agent creates in an expanded folder inside the branch", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    mkdirSync(join(branch, "notes"), { recursive: true });
    writeFileSync(join(branch, "notes", "first.md"), "one\n");
    await showFilesOf("fix-login");
    const notes = await fileRow("fix-login", "notes", 60_000);
    await notes.click();
    await fileRow("fix-login", "notes/first.md");

    writeFileSync(join(branch, "notes", "second.md"), "two\n");

    await fileRow("fix-login", "notes/second.md", 60_000);
  });

  /**
   * **What a branch changed, marked as an agent writes** (FM-4, #1107), against the real core:
   * `branch_status` runs git in the fixture branch, and the folder watch hears the write.
   */
  it("marks a file an agent adds or changes without a refresh", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    await showFilesOf("fix-login");
    await explorerRow("fix-login", "README.md");
    const readme = readFileSync(join(branch, "README.md"), "utf8");

    try {
      writeFileSync(join(branch, "agent-marked.md"), "written by an agent\n");
      writeFileSync(join(branch, "README.md"), `${readme}changed by an agent\n`);

      await marked("fix-login", "agent-marked.md", "added");
      await marked("fix-login", "README.md", "changed");
    } finally {
      // One app process serves the whole run: the branch is left as it was found.
      writeFileSync(join(branch, "README.md"), readme);
      rmSync(join(branch, "agent-marked.md"), { force: true });
    }
    await $(inExplorer('[data-row="file:svc/fix-login:README.md"][data-mark]')).waitForExist({
      timeout: 60_000,
      reverse: true,
      timeoutMsg: "README.md is still marked after the agent put it back",
    });
  });

  it("marks an agent's first write into a folder nobody opened", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    await showFilesOf("fix-login");
    // `.claude` holds a file git tracks, is closed, and the branch has changed nothing in it.
    const folder = await explorerRow("fix-login", ".claude");
    await expect(folder).toHaveAttribute("aria-expanded", "false");
    await $(inExplorer('[data-row="file:svc/fix-login:.claude"][data-mark]')).waitForExist({
      timeout: 20_000,
      reverse: true,
      timeoutMsg: ".claude is marked before anything was written in it",
    });

    try {
      writeFileSync(join(branch, ".claude", "agent-first.md"), "the agent's first write here\n");

      await marked("fix-login", ".claude", "added");
    } finally {
      rmSync(join(branch, ".claude", "agent-first.md"), { force: true });
    }
  });

  it("collapses to what the branch changed, and filters the tree by name until Esc", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    mkdirSync(join(branch, "changed-dir"), { recursive: true });
    writeFileSync(join(branch, "changed-dir", "deep.md"), "changed\n");
    await showFilesOf("fix-login");
    await explorerRow("fix-login", "README.md");
    // By role and name: a toggle button, and the search box beside the tree. The button's
    // words are its label part (#2904), so it is the button whose words they are, not the
    // element `aria/` finds them in.
    const changedOnly = await $('//button[normalize-space()="Changed only"]');
    await expect(changedOnly).toHaveAttribute("aria-pressed", "false");

    try {
      await changedOnly.click();

      // Every folder of it open, with nothing clicked; and what the branch did not change gone.
      await explorerRow("fix-login", "changed-dir/deep.md", 60_000);
      await $(inExplorer('[data-row="file:svc/fix-login:README.md"]')).waitForExist({
        timeout: 20_000,
        reverse: true,
        timeoutMsg: "Changed only still draws README.md, which the branch did not change",
      });
      await changedOnly.click();
      await explorerRow("fix-login", "README.md");

      const filter = await $("aria/Filter files");
      await expect(filter).toHaveAttribute("type", "search");
      await filter.setValue("readme");
      await $(inExplorer('[data-row="file:svc/fix-login:changed-dir"]')).waitForExist({
        timeout: 20_000,
        reverse: true,
        timeoutMsg: "the filter still draws a folder whose name and rows do not match",
      });
      await expect(await explorerRow("fix-login", "README.md")).toBeExisting();

      await browser.keys(["Escape"]);

      await expect(filter).toHaveValue("");
      await explorerRow("fix-login", "changed-dir");
    } finally {
      if ((await changedOnly.getAttribute("aria-pressed")) === "true") await changedOnly.click();
      rmSync(join(branch, "changed-dir"), { recursive: true, force: true });
    }
  });

  /**
   * **The branch cockpit** (FM-5, #1108), against the real core: the fixture branch's row menu
   * focuses the explorer on it, `branch_ahead_behind` counts it against the base recorded for it
   * in the reader's child, and its files are drawn under its state. Esc steps back out.
   */
  it("focuses the fixture branch into its cockpit, with its state and files, and leaves on Esc", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const clone = join(plane, "workspaces", "alpha", "svc");
    // The fixture is cut by plain git, which records no base; charter records `main` for a
    // branch it cuts, and the header counts against it.
    const base = ["config", "branch.fix-login.charterBase"];
    execFileSync("git", [...base, "main"], { cwd: clone, stdio: "pipe" });
    await $('[data-testid="piece-svc-fix-login"]').waitForExist({ timeout: 20_000 });

    try {
      expect(await sendContextMenu('.spot[data-testid="piece-svc-fix-login"]')).toBe(true);
      const menu = await $('[role="menu"]');
      await menu.waitForDisplayed({ timeout: 20_000 });
      await pressMenuPath(ROW.focus);

      // By role and name: the header region, the breadcrumb, and the cockpit's own tree.
      // The cockpit's own header: the Search view's scope list names the branch too (#1676).
      const head = await $(COCKPIT_HEAD);
      await head.waitForDisplayed({ timeout: 20_000 });
      await browser.waitUntil(
        async () => (await head.getText()).includes("0 ahead, 0 behind main"),
        {
          timeout: 60_000,
          timeoutMsg: `the header never counted the branch against main: ${await head.getText()}`,
        },
      );
      await expect(head).toHaveText(expect.stringMatching(/No changes|\d+ changes?/));
      await expect(await $("aria/Breadcrumb")).toBeDisplayed();
      // Explorer's own: the Repos view keeps its tree, in the Panel (#1737).
      await expect(
        await $('[data-testid="explorer"] [role="treegrid"][aria-label="Repos and branches"]'),
      ).not.toBeExisting();
      const files = await $(inExplorer('[data-row="file:svc/fix-login:"]'));
      await expect(files).toHaveAttribute("aria-expanded", "true");
      await explorerRow("fix-login", "README.md");

      await browser.execute(() => {
        document
          .querySelector<HTMLElement>('[data-testid="explorer"] [data-row="file:svc/fix-login:"]')
          ?.focus();
      });
      await browser.keys(["Escape"]);

      // Out of the cockpit, Explorer's Files section is back.
      await $('[data-testid="explorer"] [data-testid="files"]').waitForExist({
        timeout: 20_000,
        timeoutMsg: "Esc did not step back out of the cockpit",
      });
      await expect(await $(COCKPIT_HEAD)).not.toBeExisting();
    } finally {
      // One app process serves the whole run: the explorer and the clone are left as found.
      const crumb = await $("aria/Breadcrumb");
      if (await crumb.isExisting()) await crumb.$("button").click();
      execFileSync("git", ["config", "--unset-all", "branch.fix-login.charterBase"], {
        cwd: clone,
        stdio: "pipe",
      });
    }
  });

  it("expands a repo into the repo's own files", async () => {
    await onAlpha();
    const files = await $('.file-node[data-testid="files-tool"]');
    await files.waitForExist({ timeout: 20_000 });

    await files.click();

    // `scratch.txt` is in the clone and not committed: the repo's own folder, as it is.
    await expect(await fileRow(null, "scratch.txt", 20_000, "tool")).toBeExisting();
  });

  /**
   * **The file tab is the branch's tree beside a resizable preview** (FM-2, #1105), against the
   * real core: the tree is `branch_tree`'s, the preview `piece_file`'s, and the divider's place
   * is written to the plane's record the next launch reads. Nothing is stubbed.
   */
  it("opens the fixture branch's file tab as a tree, drags its divider and keeps it", async () => {
    await onAlpha();
    await browseTheFiles();
    const tree = await $('[data-testid="piece-files-tree"]');
    await tree.waitForExist({ timeout: 20_000 });

    // A file opens in the preview beside the tree.
    const readme = await tree.$('[data-row="file:svc/fix-login:README.md"]');
    await readme.waitForExist({ timeout: 20_000 });
    await readme.click();
    const preview = await $('section.piece-files-shown[aria-label="README.md"]');
    await browser.waitUntil(async () => (await preview.getText()).includes("svc"), {
      timeout: 20_000,
      timeoutMsg: "the preview never drew README.md",
    });
    await expect(readme).toHaveAttribute("aria-selected", "true");

    // The divider is dragged with the pointer…
    const divider = await $('[role="separator"][aria-label="Resize the file tree"]').getElement();
    await divider.waitForDisplayed({ timeout: 20_000 });
    const before = await valueOf(divider);
    expect(await dragTheDivider(120)).toBe(true);
    await browser.waitUntil(async () => (await valueOf(divider)) > before, {
      timeout: 20_000,
      timeoutMsg: `dragging the divider never moved it from ${before}%`,
    });
    // …and moved from the keyboard, saying where it is.
    const dragged = await valueOf(divider);
    await divider.click();
    await browser.keys(["ArrowLeft"]);
    await browser.waitUntil(async () => (await valueOf(divider)) < dragged, {
      timeout: 20_000,
      timeoutMsg: "the arrow key never moved the divider",
    });
    const left = await valueOf(divider);

    // The record the next launch reads holds the tab and where its divider was.
    const record = join((await ask<string[]>("open_planes"))[0], ".charter", "app", "reopen.json");
    await browser.waitUntil(
      async () =>
        filesTabsIn(record).some(
          (one) => one.key === "alpha/svc/fix-login" && Math.abs((one.split ?? 0) - left) <= 1,
        ),
      { timeout: 20_000, timeoutMsg: "the record never kept the file tab's divider" },
    );

    // Opening the same branch's files again keeps one tab…
    await browseTheFiles();
    expect(filesTabsIn(record).filter((one) => one.key === "alpha/svc/fix-login")).toHaveLength(1);
    expect((await tabNames()).filter((name) => name === "Files · fix-login")).toHaveLength(1);

    // …and closing it and opening it again brings the divider back where it was.
    await $('[data-strip="Tabs"] button[aria-label="Close Files · fix-login"]').click();
    await browser.waitUntil(async () => !(await tabNames()).includes("Files · fix-login"), {
      timeout: 20_000,
      timeoutMsg: "the file tab did not close",
    });
    await browseTheFiles();
    const again = await $('[role="separator"][aria-label="Resize the file tree"]').getElement();
    await again.waitForDisplayed({ timeout: 20_000 });
    await browser.waitUntil(async () => Math.abs((await valueOf(again)) - left) <= 1, {
      timeout: 20_000,
      timeoutMsg: `the divider came back at ${await valueOf(again)}%, not the ${left}% it was left at`,
    });

    // Put away, so the specs after this one find the explorer's rows and no tab's.
    await $('[data-strip="Tabs"] button[aria-label="Close Files · fix-login"]').click();
    await browser.waitUntil(async () => !(await tabNames()).includes("Files · fix-login"), {
      timeout: 20_000,
      timeoutMsg: "the file tab did not close",
    });
  });

  /**
   * **What a changed file changed, from the file tab** (FM-11, #1189), against the real core:
   * `branch_status` marks the file changed, its preview offers *Show what changed*, and the
   * comparison tab draws git's hunks in the merge view. The fixture branch was cut by plain git
   * and records no base, so the file is compared with its last commit. Nothing is stubbed.
   */
  it("shows what a changed file changed, from the file tab, in the merge view", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    const readme = readFileSync(join(branch, "README.md"), "utf8");
    const added = "a line the comparison shows";

    try {
      writeFileSync(join(branch, "README.md"), `${readme}${added}\n`);
      await browseTheFiles();
      const tree = await $('[data-testid="piece-files-tree"]');
      await tree.waitForExist({ timeout: 20_000 });
      const row = await tree.$('[data-row="file:svc/fix-login:README.md"]');
      await row.waitForExist({ timeout: 20_000 });
      await row.click();

      // Offered once the branch's status has said the file changed.
      const show = await $("button=Show what changed");
      await show.waitForExist({
        timeout: 60_000,
        timeoutMsg: "the file tab never offered Show what changed for README.md",
      });
      await show.click();

      await browser.waitUntil(
        async () => (await tabInFront()) === "What changed · README.md · fix-login",
        {
          timeout: 20_000,
          timeoutMsg: `the comparison never came forward; in front is ${await tabInFront()}`,
        },
      );
      const merge = await $('[data-testid="merge-viewer"]');
      await merge.waitForExist({ timeout: 20_000 });
      expect(await merge.getAttribute("aria-label")).toContain("What changed in README.md against");
      await browser.waitUntil(
        async () =>
          (await textOfEach('[data-testid="merge-viewer"] .cm-changedLine')).some((line) =>
            line.includes(added),
          ),
        { timeout: 20_000, timeoutMsg: "the merge view never drew the added line as changed" },
      );
    } finally {
      // One app process serves the whole run: the branch and the strip are left as found.
      writeFileSync(join(branch, "README.md"), readme);
      for (const name of ["What changed · README.md · fix-login", "Files · fix-login"]) {
        const close = await $(`[data-strip="Tabs"] button[aria-label="Close ${name}"]`);
        if (await close.isExisting()) await close.click();
      }
    }
  });

  /**
   * **The explorer's rows draw the icon theme** (FM-3, #1106; #1145), in the real app: the
   * built-in theme draws README.md as `readme`, by its file name, and a folder it has no name
   * for, `.claude`, as its plain folder. Vitest holds the mapping and the tones; this holds that
   * the real tree draws through it.
   */
  it("draws a file's and a folder's icon from the icon theme", async () => {
    await onAlpha();
    await showFilesOf("fix-login");

    const readme = await explorerRow("fix-login", "README.md");
    await expect(readme.$("svg.file-icon")).toHaveAttribute("data-icon", "readme");
    const folder = await explorerRow("fix-login", ".claude");
    const open = (await folder.getAttribute("aria-expanded")) === "true";
    await expect(folder.$("svg.file-icon")).toHaveAttribute(
      "data-icon",
      open ? "folder-open" : "folder",
    );
  });

  it("lists no workspaces, and nothing the old sidebar drew of them", async () => {
    // ADR 0038, and the reason this region was rewritten: the old sidebar drew every
    // workspace with its vision text under the strip that had just been made the axis. #1677
    // listed them by name again, and that section is gone too (qw358): the strip selects.
    await onAlpha();
    await $('[data-testid="clone-svc"]').waitForExist({ timeout: 20_000 });

    const explorer = await $('[data-testid="explorer"]');
    await expect(explorer).not.toHaveText(expect.stringContaining("Retire the old importer"));
    await expect(explorer).not.toHaveText(expect.stringContaining("Ship the widget"));
    const sections = await browser.execute(() =>
      [...document.querySelectorAll('[data-testid="explorer"] [data-section]')].map((one) =>
        one.getAttribute("data-section"),
      ),
    );
    expect(sections).toEqual(["files"]);
    await expect($('[data-testid="explorer"] [data-row^="workspace:"]')).not.toBeExisting();
  });

  it("is the files of the workspace the strip is on, and names it in its tree", async () => {
    /** What the files' tree says it is of: *Files* is no fold since #3343, and the view's title
     *  is its region's. Read from the document, shown or not. */
    const filesOf = () =>
      browser.execute(
        () =>
          document
            .querySelector('[data-testid="explorer"] [data-section="files"] [role="treegrid"]')
            ?.getAttribute("aria-label")
            ?.replace(/^Files of /, "") ?? "",
      );
    const untilFilesOf = (name: string) =>
      browser.waitUntil(async () => (await filesOf()) === name, {
        timeout: 20_000,
        timeoutMsg: `Files never said it was of ${name}`,
      });
    await onAlpha();
    await untilFilesOf("alpha");
    await $('[data-testid="files-svc"]').waitForExist({ timeout: 20_000 });

    await focus("beta");

    // `beta` holds no repos: none of alpha's are drawn under its name.
    await untilFilesOf("beta");
    await expect($('[data-testid="explorer"] [data-testid="files-svc"]')).not.toBeExisting();

    // The plane root is not a workspace: the heading says where the strip is, and the tree is
    // the project's own files (issue 1916), or the sentence that says why they are not listed.
    // Never a workspace's repo.
    await focusWorkspace("Plane root");
    await untilFilesOf("Plane root");
    await $(
      '[data-testid="explorer"] [role="treegrid"][aria-label="Files of Plane root"]',
    ).waitForExist({ timeout: 20_000 });
    await expect($('[data-testid="explorer"] [data-testid="files-svc"]')).not.toBeExisting();

    await focus("alpha");
    await untilFilesOf("alpha");
    await $('[data-testid="files-svc"]').waitForExist({ timeout: 20_000 });
  });

  it("has no two things called Workspaces in the window", async () => {
    // Two `[aria-label="Workspaces"]` broke a spec when the second appeared. The old left
    // sidebar was that second one, and the workspace strip the first until its workspaces
    // moved to the title bar (#2916).
    await untilListed(["alpha", "beta"]);

    const named = await $$('[aria-label="Workspaces"]').getElements();
    expect(named.length).toBeLessThanOrEqual(1);
  });

  it("follows the focus to another workspace", async () => {
    await onAlpha();

    await focus("beta");

    // `beta` holds no repos at all, which the Repos view says (#1737).
    const repos = await $('[data-testid="repos-view"]');
    await browser.waitUntil(async () => (await repos.getText()).includes("No repos"), {
      timeout: 20_000,
      timeoutMsg: "the Repos view never followed the focus to beta",
    });

    await focus("alpha");
    await $('[data-testid="clone-svc"]').waitForExist({ timeout: 20_000 });
  });

  it("lists a chat started in the workspace in the Chats view, and not in the explorer", async () => {
    await onAlpha();
    await $('[data-testid="clone-svc"]').waitForExist({ timeout: 20_000 });
    await showView("Chats");
    const rows = async () =>
      (await $$('[data-testid="chats-section"] [role="treeitem"]').getElements()).length;
    const before = await rows();

    await pressAndStart("New tab");

    // The Chats view is the one list of chats (#1673): the new chat is a row there, and the
    // explorer, which drew it under the workspace row until then, draws no chat.
    await browser.waitUntil(async () => (await rows()) > before, {
      timeout: 20_000,
      timeoutMsg: "the chat never appeared in the Chats view",
    });
    await showView("Explorer");
    expect(await $$('[data-testid="explorer"] [data-row^="chat:"]').getElements()).toHaveLength(0);
  });

  /**
   * **A chat's row is a sidebar view's row** (#3342, the prototype's Q1 A): the control's height
   * and a step, 30px at 14px, measured here in the real engine within a pixel, never exactly. A
   * file's row is still the shared tree row (the Explorer moves onto the frame on its own
   * ticket). The row's words are the roles' sizes, as shares of the window's text: a name a
   * body (13 of 14), and the view's title a strong body (13 of 14).
   */
  it("draws a chat's row as a view's row, its name and its title at the body's size (#1920, #3342)", async () => {
    await onAlpha();
    await showView("Chats");
    const rows = '[data-testid="chats-section"] [role="treeitem"]';
    // The chat the case above started; one is started where a run begins here.
    if ((await $$(rows).getElements()).length === 0) await pressAndStart("New tab");
    await $(rows).waitForExist({ timeout: 20_000 });
    const chat = await browser.execute((selector: string) => {
      const row = document.querySelector<HTMLElement>(selector);
      const name = row?.querySelector(".session");
      const title = document.querySelector("#chats-title");
      if (!row || !name || !title) return null;
      const probe = document.createElement("div");
      probe.style.height = "calc(var(--size-control) + var(--space-1))";
      document.body.append(probe);
      const viewRow = probe.getBoundingClientRect().height;
      probe.remove();
      return {
        viewRow,
        height: row.getBoundingClientRect().height,
        root: Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
        name: Number.parseFloat(getComputedStyle(name).fontSize),
        title: Number.parseFloat(getComputedStyle(title).fontSize),
      };
    }, rows);
    if (chat === null) throw new Error("no chat's row was drawn to measure");

    await showView("Explorer");
    const file = await $('.file-node[data-testid="files-tool"]');
    await file.waitForExist({ timeout: 20_000 });
    const fileHeight = await browser.execute(
      () =>
        document.querySelector('.file-node[data-testid="files-tool"]')?.getBoundingClientRect()
          .height ?? 0,
    );

    const seen = { chat, fileHeight };
    if (!(chat.height > 0 && fileHeight > 0))
      throw new Error(`a row has no height: ${JSON.stringify(seen)}`);
    if (Math.abs(chat.height - chat.viewRow) > 1)
      throw new Error(`a chat's row is not a view's row's height: ${JSON.stringify(seen)}`);
    // Ranges, never exact pixels: within half a pixel of the role's share of the window's text.
    if (Math.abs(chat.name - (chat.root * 13) / 14) > 0.5)
      throw new Error(`a chat's name is not the body's size: ${JSON.stringify(seen)}`);
    if (Math.abs(chat.title - (chat.root * 13) / 14) > 0.5)
      throw new Error(`the Chats title is not a view's title's size: ${JSON.stringify(seen)}`);
  });

  it("shows one workspace's chats on the strip, and keeps the others running", async () => {
    // The axis the tmux frame had and the port lost (ADR 0036): the chat strip shows
    // the focused workspace's chats — and a glance at another workspace ends nothing, which
    // is the same guarantee a project behind another one has (#125).
    await untilListed(["alpha", "beta"]);
    await focus("beta");
    // By the number the core gave it (#1796): another chat may carry the same name.
    const started = await startAChat();

    await focus("alpha");

    let inFront = -1;
    await browser.waitUntil(async () => (inFront = await paneInFront()) !== started.session, {
      timeout: 20_000,
      timeoutMsg: `chat ${started.session} was still in front once alpha was focused`,
    });
    // Looked away from, and still running: a glance at another workspace ends nothing.
    expect(await runningIn(started.plane)).toContain(started.session);
    expect(inFront).not.toBe(started.session);
    // And not on alpha's strip. A tab carries a name, not a number, so this is asked only while
    // no other chat in the project goes by the same name: a name another chat shares would say
    // nothing about this one. A tab may put a persona before the name the core keeps
    // (`steward 29` for `29`), so the tab's name is that name or ends with it.
    const named = await ask<{ name: string }[]>("opened_chats", { plane: started.plane });
    const sharing = named.filter(
      (one) => started.name === one.name || started.name.endsWith(` ${one.name}`),
    );
    if (sharing.length === 1) expect(await tabNames()).not.toContain(started.name);

    await focus("beta");

    // Still there, still running: nothing was torn down by looking away.
    await browser.waitUntil(async () => (await paneInFront()) === started.session, {
      timeout: 20_000,
      timeoutMsg: `chat ${started.session} did not come back when its workspace was focused again`,
    });
  });

  /**
   * **A shell tab in a folder of a branch, from the folder's menu** (FM-10, #1113), against the
   * real core, and last, so the shell it leaves running is counted by no scenario here:
   * `open_shell_in_branch` resolves the folder inside the branch and starts the operator's shell
   * there. The window names the branch and the folder, never a directory, and the chat the core
   * opened works in that folder. The event is dispatched, for the piece menu's reason above.
   */
  it("opens a shell tab in a folder of the branch from the folder's menu", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    mkdirSync(join(branch, "shell-here"), { recursive: true });
    writeFileSync(join(branch, "shell-here", "keep.md"), "kept\n");
    await showFilesOf("fix-login");
    await fileRow("fix-login", "shell-here", 60_000);

    const sent = await sendContextMenu('[data-row="file:svc/fix-login:shell-here"]');
    expect(sent).toBe(true);
    const menu = await $('[role="menu"]');
    await menu.waitForDisplayed({ timeout: 20_000 });
    expect((await menuNames())[0]).toContain(ROW.copyAbsolutePath);
    await pressMenuPath(ROW.shellHere);
    await menu.waitForDisplayed({ timeout: 20_000, reverse: true });

    await browser.waitUntil(async () => (await tabInFront()).startsWith("shell"), {
      timeout: 20_000,
      timeoutMsg: `no shell tab came forward; in front is ${await tabInFront()}`,
    });
    const chats = await ask<{ cwd: string | null }[]>("opened_chats", { plane });
    expect(
      chats.some((one) => one.cwd?.split("\\").join("/").endsWith("/fix-login/shell-here")),
    ).toBe(true);
  });

  /**
   * **A file dropped on a chat is typed into it as a reference, and nothing is sent** (FM-9,
   * #1112), against the real core. The chat is the fake harness at a raw prompt — it echoes each
   * byte typed and answers a line only on Enter — on a profile of Claude Code's kind that reports
   * its `SessionStart`, so the core types into it once it is waiting and its terminal is raw. The drag is dispatched as the window's own events, with a real `DataTransfer`,
   * for the reason the piece menu's right-click is: a WebDriver drag does not carry one. Last of
   * the chats this file starts, so the profile it adds picks nothing for a spec before it.
   */
  it("types a dropped file into a chat as a reference, and sends nothing", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    await startARawPromptChat(plane);
    const rows = paneRows;

    await showFilesOf("fix-login");
    await fileRow("fix-login", "README.md");
    // Quiet for the second the core waits on a harness of this kind before it types.
    await browser.pause(1_500);

    const dropped = await browser.execute(() => {
      const from = document.querySelector(
        '[data-testid="explorer"] [data-row="file:svc/fix-login:README.md"]',
      );
      const to = document.querySelector('[data-strip="Tabs"] [role="tab"][aria-selected="true"]');
      if (!(from instanceof HTMLElement) || !(to instanceof HTMLElement)) return false;
      const dataTransfer = new DataTransfer();
      for (const [at, type] of [
        [from, "dragstart"],
        [to, "dragover"],
        [to, "drop"],
      ] as const)
        at.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer }));
      return true;
    });
    expect(dropped).toBe(true);

    // What the action answered: its notice on the card that pops from the status bar's Notices
    // item (#3653), or at the top of the opener's page with no project in front (#3344).
    const said = await $(
      '[data-chip="status/bar/notices"] > .action-result [role="status"], .window-work > .action-result [role="status"]',
    );
    await said.waitForExist({ timeout: 20_000 });
    await expect(said).toHaveText(expect.stringContaining("Nothing was sent."));
    await browser.waitUntil(
      async () => (await rows()).some((text) => text.includes("fix-login/README.md")),
      { timeout: 20_000, timeoutMsg: `the reference was never typed: ${(await rows()).join("|")}` },
    );
    // Typed and unsent: the harness answers a line only on Enter, and it answered none.
    await browser.pause(1_000);
    expect((await rows()).some((text) => text.includes("you said:"))).toBe(false);
  });

  /**
   * **Several file rows picked, and handed to a chat as one paste** (#1151's third line): two
   * rows of the fixture branch picked with the platform's Cmd or Ctrl and a click, *Add to a
   * chat's context* under the tree, and a chat on the raw-prompt profile picked in the dialog.
   * The core types both paths into it at once, in the order the rows are drawn, and sends
   * nothing.
   *
   * The chat is the newest one open on that profile — the drop's, in a whole run — or one it
   * starts where none is, so it never rests on the case before it having passed. The modified
   * clicks are dispatched from the page, for the reason the drop is: a WebDriver click carries no
   * modifier the same way on both engines. The two files it writes into the branch go again.
   */
  it("hands several picked file rows to a chat as one paste, and sends nothing", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const branch = join(plane, "workspaces", "alpha", ".worktrees", "svc", "fix-login");
    const files = ["picked-a.md", "picked-b.md"];
    // The Explorer view's *Files*, the one tree whose rows are picked: the Repos view draws no
    // picks, and a modified click there would open the file instead.
    const PICKING = '[data-testid="explorer"] [role="treegrid"][aria-multiselectable="true"]';
    const rows = paneRows;
    // By the number the core gave it, which the dialog's row carries (#1796).
    const chat = await aRawPromptChat(plane);
    for (const name of files) writeFileSync(join(branch, name), `${name}\n`);
    try {
      await showFilesOf("fix-login");
      for (const name of files) await explorerRow("fix-login", name, 60_000);

      // One modified click at a time, each waited for: the second is read against the picks the
      // first made, and two dispatched in one script both land before the window has drawn.
      const pickRow = (css: string, mac: boolean) =>
        browser.execute(
          (selector: string, onAMac: boolean) => {
            const row = document.querySelector(selector);
            if (!(row instanceof HTMLElement)) return false;
            row.dispatchEvent(
              new MouseEvent("click", {
                bubbles: true,
                cancelable: true,
                metaKey: onAMac,
                ctrlKey: !onAMac,
              }),
            );
            return true;
          },
          css,
          mac,
        );
      const pickedNow = () =>
        browser.execute(
          () =>
            document.querySelector('[role="group"][aria-label="Picked rows"]')?.textContent ?? "",
        );
      for (const [at, name] of files.entries()) {
        expect(
          await pickRow(
            `${PICKING} [data-row="file:svc/fix-login:${name}"]`,
            process.platform === "darwin",
          ),
        ).toBe(true);
        await browser.waitUntil(async () => (await pickedNow()).includes(`${at + 1} picked`), {
          timeout: 20_000,
          timeoutMsg: `after picking ${name}, the window said ${JSON.stringify(await pickedNow())}`,
        });
      }

      const bar = await $('[role="group"][aria-label="Picked rows"]');
      await bar.waitForExist({ timeout: 20_000, timeoutMsg: "no line said what was picked" });
      await expect(bar).toHaveText(expect.stringContaining("2 picked"));
      for (const name of files)
        await expect(
          $(`${PICKING} [data-row="file:svc/fix-login:${name}"][aria-selected="true"]`),
        ).toExist();

      await browser.execute(() =>
        document
          .querySelector<HTMLButtonElement>(
            `[role="group"][aria-label="Picked rows"] button[aria-label="Add 2 picked to a chat's context"]`,
          )
          ?.click(),
      );
      const dialog = await $('[role="dialog"]');
      await dialog.waitForDisplayed({ timeout: 20_000 });
      await expect(dialog).toHaveText(
        expect.stringContaining("Add 2 picked rows to a chat's context"),
      );
      // The dialog's row for the chat, by the number it carries (`data-session`,
      // references.tsx): two chats may share a name, and a name may carry a persona before it.
      const listed = await browser.execute((session: number) => {
        const items = [
          // The chats are a list to choose from (#2911).
          ...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] [role="option"]'),
        ];
        const picked = items.find((item) => item.getAttribute("data-session") === String(session));
        picked?.click();
        return {
          all: items.map((item) => `${item.getAttribute("data-session")}: ${item.textContent}`),
          picked: picked !== undefined,
        };
      }, chat);
      if (!listed.picked)
        throw new Error(`the dialog listed no chat ${chat}: ${JSON.stringify(listed.all)}`);
      await dialog.waitForDisplayed({ timeout: 20_000, reverse: true });

      // One paste: one line says both were typed, in the order the rows are drawn.
      let said: string[] = [];
      await browser
        .waitUntil(
          async () => {
            said = await textOfEach('[role="status"]');
            return said.some((one) => one.includes(files[0]) && one.includes(files[1]));
          },
          { timeout: 20_000 },
        )
        .catch(() => {
          throw new Error(`no line said both were typed; the window said ${JSON.stringify(said)}`);
        });
      const line = said.find((one) => one.includes(files[0]) && one.includes(files[1])) ?? "";
      expect(line).toContain("Nothing was sent.");
      expect(line.indexOf(files[0])).toBeLessThan(line.indexOf(files[1]));
      await browser.waitUntil(
        async () =>
          (await rows()).some(
            (text) =>
              text.includes(`fix-login/${files[0]}`) && text.includes(`fix-login/${files[1]}`),
          ),
        { timeout: 20_000, timeoutMsg: `the paste was never typed: ${(await rows()).join("|")}` },
      );
      // Typed and unsent: the harness answers a line only on Enter, and it answered none.
      await browser.pause(1_000);
      expect((await rows()).some((text) => text.includes("you said:"))).toBe(false);
    } finally {
      // The picks go, then the files: the rows they were of are gone after.
      await browser.execute(() =>
        document
          .querySelector<HTMLButtonElement>(
            '[role="group"][aria-label="Picked rows"] button[aria-label="Clear the picked rows"]',
          )
          ?.click(),
      );
      for (const name of files) rmSync(join(branch, name), { force: true });
    }
  });

  /**
   * **A file a chat's tool touches is marked live, and the mark fades** (FM-6, #1109; #1154's
   * third line), against the real core: the chat's harness runs the real `charter hook
   * pretooluse-read` with a `Read` of the fixture branch's README, inside its own session, so
   * the line reaches the app's socket with the chat's own token. The core confines the path to
   * the chat's folder and rates it; the window draws a dot on the file's row and lets it go once
   * the chat has been quiet on it for `touching.ts`'s `FADE_MS`.
   *
   * The hook waits for a file this spec writes, so the Read happens once the row is on screen
   * and not while the chat is still starting. The profile it adds is taken out again: one app
   * process serves the whole run, and the specs after this file draw the picker too.
   */
  it("marks a file a chat's tool is reading, and lets the mark fade", async () => {
    await onAlpha();
    const plane = (await ask<string[]>("open_planes"))[0];
    const local = join(plane, "charter.local.toml");
    const declared = readFileSync(local, "utf8");
    const payload = join(plane, ".charter", "scenario-touch.json");
    // Beside the project, not in it: a sandboxed start, the floor included, runs no program the
    // chat could have written (#2071).
    const program = join(`${plane}.programs`, "touching-harness");
    mkdirSync(`${plane}.programs`, { recursive: true });
    rmSync(payload, { force: true });
    // Waits for the payload (two minutes at most), then hands it to the hook a real Claude Code
    // runs before a Read. `CHARTER_HOOK_BINARY` is the binary the app armed the chat with.
    const touch = [
      `n=0; while [ ! -e ${singleQuoted(payload)} ] && [ $n -lt 600 ]; do sleep 0.2; n=$((n+1)); done;`,
      `"\${CHARTER_HOOK_BINARY:-charter}" hook pretooluse-read < ${singleQuoted(payload)} >/dev/null`,
    ].join(" ");
    writeFileSync(
      program,
      answeringAsClaudeCode([
        "#!/bin/sh",
        // A Claude Code chat is started with purlis's own flags, which the fake harness has none of.
        "while [ $# -gt 0 ]; do shift; done",
        `exec ${singleQuoted(built("fake-harness"))} --sentinel ${singleQuoted(READY)} \\`,
        `  --hook ${singleQuoted(touch)} --interactive`,
        "",
      ]).join("\n"),
    );
    chmodSync(program, 0o755);
    writeFileSync(
      local,
      `${declared}\n[harness.touching]\nkind = "claude"\ncommand = [${JSON.stringify(program)}]\n`,
    );

    // The chat this starts, by the number the core gave it: the one `running_sessions` lists
    // after the start and did not before (#1796).
    let started: number | undefined;
    const runningBefore = new Set(await runningIn(plane));
    try {
      await pressOnly("New tab");
      await harnessRowsDrawn();
      // The harness's option card (#2907): its radio, clicked as the driver's pointerless click
      // reaches it (React Aria takes a virtual press on the input, not on its label).
      const label = await $("label*=touching");
      await browser.execute(
        (el: HTMLElement) => el.querySelector("input")?.click(),
        label as unknown as HTMLElement,
      );
      await $("button=Approve and start").waitForExist({ timeout: 20_000 });
      await browser.execute(() =>
        [...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')]
          .find((button) => button.textContent === "Approve and start")
          ?.click(),
      );
      await $('[role="dialog"][aria-labelledby="start-chat"]').waitForExist({
        timeout: 20_000,
        reverse: true,
      });
      await browser.waitUntil(
        async () =>
          (started = (await runningIn(plane)).find((one) => !runningBefore.has(one))) !== undefined,
        { timeout: 30_000, interval: 250, timeoutMsg: "the touching profile started no chat" },
      );

      await showFilesOf("fix-login");
      const readme = inExplorer('[data-row="file:svc/fix-login:README.md"] .touch-mark');
      await explorerRow("fix-login", "README.md");
      await $(readme).waitForExist({
        timeout: 5_000,
        reverse: true,
        timeoutMsg: "README.md was marked before any chat touched it",
      });

      // What Claude Code pipes to the hook before a Read: the tool and the path it was given,
      // from the chat's folder, which is the workspace's own and holds the fixture branch.
      const tool = {
        hook_event_name: "PreToolUse",
        tool_name: "Read",
        tool_use_id: "toolu_scenario_touch",
        tool_input: { file_path: ".worktrees/svc/fix-login/README.md" },
      };
      writeFileSync(`${payload}.part`, JSON.stringify(tool));
      renameSync(`${payload}.part`, payload);

      const mark = await $(readme);
      await mark.waitForExist({
        timeout: 20_000,
        timeoutMsg: "the explorer never marked README.md while the chat was reading it",
      });
      expect(await mark.getAttribute("aria-label")).toMatch(/ is working here now$/);
      // And it fades, with nothing else said: the window keeps a touch for a few seconds only.
      await mark.waitForExist({
        timeout: 20_000,
        reverse: true,
        timeoutMsg: "the mark on README.md never faded",
      });
    } finally {
      writeFileSync(local, declared);
      rmSync(payload, { force: true });
      rmSync(program, { force: true });
      // Ended here so the profile's chat goes with the profile; where it would not end, the
      // block's `after` ends it with every other chat this file started, and says so.
      if (started !== undefined) await endChatById(started, plane).catch(() => undefined);
    }
  });
});

/**
 * **The tree's rows, measured — because jsdom lays nothing out.**
 *
 * Every assertion in this block is about a used value: a height in pixels, a `scrollWidth`, the
 * position a guide was actually painted at. `getComputedStyle` in jsdom answers from the
 * cascade it managed to build and gives every box a size of zero, so a vitest assertion that a
 * row "does not wrap" or that the region "scrolls" checks nothing at all. These need a window
 * that really lays out, which is why they are here.
 *
 * They also catch the trap `docs/design-system.md` names: a class that does not exist emits no
 * CSS and nothing goes red. A rule that never took would show up here as a row two lines tall.
 */
describe("the explorer's rows, in a region too narrow for them", () => {
  before(async () => await showView("Explorer"));
  after(async () => await showView("Chats"));

  /**
   * Narrows the explorer, measures, and puts it back.
   *
   * **The region's own box is narrowed, not the separator dragged.** The question is whether
   * this scroll container folds its rows or scrolls them when it is narrower than they are, and
   * the container is `.explorer` itself — so its width is the input, whatever produced it. A
   * pointer gesture on the separator would ask the same question through a drag whose pixels
   * differ per platform, and `react-resizable-panels`' own inline style is a shape this spec
   * would then be pinning. Everything is read back inside one script, before React can render
   * again and take the style away.
   */
  async function narrowed(): Promise<{
    scrollWidth: number;
    clientWidth: number;
    scrolledTo: number;
    rows: { what: string; height: number; limit: number; overflow: number; wraps: string }[];
  }> {
    return browser.execute(() => {
      // The repos' and branches' rows, the Repos view's since #1737: Explorer's own tree, drawn
      // in the Panel.
      const explorer = document.querySelector<HTMLElement>('[data-testid="repos-view"]');
      if (!explorer) throw new Error("no Repos view to narrow");
      const was = explorer.getAttribute("style") ?? "";
      explorer.style.width = "120px";

      /** What one line of THIS row would measure, from its own font and its own padding.
       *
       *  The slack is half a line, which is the gap between the two answers rather than a
       *  guess: a row that wrapped is a WHOLE line taller, and a row that did not can still
       *  be a few pixels over its own line box because a chip inside it carries a border and
       *  its own padding. Measuring to the pixel would make this a test of `.label`'s border
       *  width; every row still clears this limit by more than three pixels on both platforms,
       *  and every wrapped one exceeds it by more than three. */
      const oneLine = (el: Element, what: string) => {
        const css = getComputedStyle(el);
        const line = Number.parseFloat(css.lineHeight);
        const height = Number.isFinite(line) ? line : Number.parseFloat(css.fontSize) * 1.5;
        const padding = Number.parseFloat(css.paddingTop) + Number.parseFloat(css.paddingBottom);
        const border =
          Number.parseFloat(css.borderTopWidth) + Number.parseFloat(css.borderBottomWidth);
        return {
          what,
          height: el.getBoundingClientRect().height,
          limit: height * 1.5 + padding + border + 4,
          // **What `min-width: max-content` is for, and the only thing that holds it.** A row
          // told only `white-space: nowrap` also stays on one line — its text simply overflows
          // its own box — and then the band behind a hovered or current row stops at the
          // region's edge while the name runs on past it. Zero here is the row's box having
          // grown to hold its content.
          overflow: el.scrollWidth - el.clientWidth,
          wraps: css.whiteSpace,
        };
      };

      const rows = [
        ...[...explorer.querySelectorAll(".spot")].map((el) => oneLine(el, "a spot")),
        ...[...explorer.querySelectorAll(".clone")].map((el) => oneLine(el, "a clone")),
        ...[...explorer.querySelectorAll(".chat")].map((el) => oneLine(el, "a chat")),
        ...[...explorer.querySelectorAll(".worktree")].map((el) => oneLine(el, "a worktree")),
      ];
      const measured = {
        scrollWidth: explorer.scrollWidth,
        clientWidth: explorer.clientWidth,
        scrolledTo: 0,
        rows,
      };
      // It really scrolls, rather than merely having something to scroll: a region whose
      // overflow is hidden clips the row and answers 0 here.
      explorer.scrollLeft = 10_000;
      measured.scrolledTo = explorer.scrollLeft;
      explorer.scrollLeft = 0;

      explorer.setAttribute("style", was);
      return measured;
    });
  }

  it("keeps every row on one line, and scrolls sideways instead of folding it", async () => {
    // The operator's own report, and his screenshot: `ai-assistant` and `the workspace itself`
    // on two lines in a narrow explorer.
    await onAlpha();
    await $('[data-testid="clone-svc"]').waitForExist({ timeout: 20_000 });

    const measured = await narrowed();

    // A spot, a clone and a worktree at least. No chat row is counted on: the spec file ends
    // the chats it started before this describe runs (#1796), so none is left to draw.
    expect(measured.rows.length).toBeGreaterThanOrEqual(3);
    // Reported as a list rather than asserted one at a time, so a failure names every row that
    // folded and what it measured instead of stopping at the first.
    const wrapped = measured.rows
      .filter((row) => row.height > row.limit)
      .map((row) => `${row.what}: ${row.height}px, and one line of it is ${row.limit}px`);
    expect(wrapped).toEqual([]);
    // A row's own box holds its own name, so the band behind a hovered or current row reaches
    // the end of it rather than stopping at the region's edge.
    const clipped = measured.rows
      .filter((row) => row.overflow > 1)
      .map((row) => `${row.what}: ${row.overflow}px of it is outside its own box`);
    expect(clipped).toEqual([]);
    // And the rule that says a name is never broken, as the engine resolved it. A rule that
    // emitted no CSS at all — the trap `docs/design-system.md` names — reads `normal` here.
    expect([...new Set(measured.rows.map((row) => row.wraps))]).toEqual(["nowrap"]);
    // Nothing to scroll means the rows were folded to fit instead.
    expect(measured.scrollWidth).toBeGreaterThan(measured.clientWidth);
    // And it really scrolls: a region whose overflow is hidden answers 0 here.
    expect(measured.scrolledTo).toBeGreaterThan(0);
  });

  /**
   * **Each level a step in, with a guide per level** (#1672, the shared tree row since DS-12).
   *
   * The explorer's rows are the tree family's (#2911): a row stands one `--tree-step` further in
   * per `aria-level`, and each level above it draws a `tree.guide` hairline on the row's own
   * `::before`, VS Code's straight guides. What is held here is what only a real WebView can
   * say: a deeper row's name really starts further in, by about one step per level (a range,
   * never a pixel), and a row under another really paints its guides.
   */
  it("steps each level in, under the row it hangs from, with its guides", async () => {
    await onAlpha();
    await $('[data-testid="piece-svc-fix-login"]').waitForExist({ timeout: 20_000 });

    const rows = await browser.execute(() => {
      const tree = document.querySelector<HTMLElement>(
        '[data-testid="repos-view"] [role="treegrid"][aria-label="Repos and branches"]',
      );
      if (!tree) throw new Error("no explorer tree drawn by the tree family");
      // The rows that name something: a line under a clone ("Asking git…") has no name to
      // measure.
      return [...tree.querySelectorAll<HTMLElement>('[role="row"]:not([data-line])')].map((row) => {
        const name = row.querySelector(".spot-name, .repo") ?? row;
        return {
          what: row.textContent?.trim().slice(0, 40) ?? "",
          level: Number(row.getAttribute("aria-level")),
          nameAt: name.getBoundingClientRect().left - tree.getBoundingClientRect().left,
          // The tree family draws a row's guides on its `::before` (`ui/tree/tree.css`).
          guides: getComputedStyle(row, "::before").backgroundImage,
        };
      });
    });

    // The workspace, a clone and a branch at least: a run with one level proved nothing.
    const levels = [...new Set(rows.map((row) => row.level))].sort();
    expect(levels.length).toBeGreaterThanOrEqual(3);
    // Where each level's names start, on average: one step (12px at the default text size)
    // further in per level, within a range that a wrong step or no step at all leaves.
    const at = (level: number) => {
      const of = rows.filter((row) => row.level === level);
      return of.reduce((sum, row) => sum + row.nameAt, 0) / of.length;
    };
    const steps = levels.slice(1).map((level, i) => at(level) - at(levels[i]));
    const crooked = steps.filter((step) => step < 8 || step > 20);
    expect(crooked).toEqual([]);
    // Every row under another paints its guides; a top-level row has none to paint.
    const unguided = rows
      .filter((row) => row.level > 1 && !row.guides.includes("gradient"))
      .map((row) => `${row.what}: ${row.guides}`);
    expect(unguided).toEqual([]);
  });

  /**
   * **A row's chevron is in the middle of its row** (qw358). The operator's screenshots: every
   * folder's `>` stood above its name, closed or open, at every depth. jsdom lays nothing out, so
   * only here can the chevron's own box be compared with its row's: their centres within a pixel
   * (a range, never a pixel position), for a closed row and an open one, in Files and in Repos.
   */
  it("draws a folding row's chevron in the middle of the row, closed or open", async () => {
    await onAlpha();
    const tool = await $('.file-node[data-testid="files-tool"]');
    await tool.waitForExist({ timeout: 20_000 });
    // One repo's own folder open and the other closed, whatever an earlier spec left.
    const wanted: [string, string][] = [
      ["files-tool", "true"],
      ["files-svc", "false"],
    ];
    for (const [testid, expanded] of wanted) {
      const row = await $(`.file-node[data-testid="${testid}"]`);
      await row.waitForExist({ timeout: 20_000 });
      if ((await row.getAttribute("aria-expanded")) !== expanded) await row.click();
      await browser.waitUntil(async () => (await row.getAttribute("aria-expanded")) === expanded, {
        timeout: 20_000,
        timeoutMsg: `${testid} never became aria-expanded=${expanded}`,
      });
    }
    // Past the twist's own turn (`motion.duration.quick`): a box half turned is not measured.
    await browser.pause(400);

    const twists = await browser.execute(() =>
      [
        ...document.querySelectorAll<HTMLElement>(
          ':is([data-testid="explorer"], [data-testid="repos-view"]) :is(.ui-tree-row, [data-scope="tree"][data-part="root"])',
        ),
      ].flatMap((row) => {
        // The Repos tree's twist (`.ui-tree-twist`), or the tree family's (`[data-part="twist"]`).
        const icon = row.querySelector<SVGElement>(
          ':scope > .ui-tree-twist > svg, [data-part="twist"] > svg',
        );
        const box = row.getBoundingClientRect();
        // A row not drawn (its view is not the one shown) has no box to measure.
        if (icon === null || box.height === 0) return [];
        const at = icon.getBoundingClientRect();
        if (at.height === 0) return [];
        const holder = row.closest("[aria-expanded]");
        return [
          {
            what: row.textContent?.trim().slice(0, 40) ?? "",
            open: holder?.getAttribute("aria-expanded") === "true",
            level: Number(
              row.getAttribute("aria-level") ?? holder?.getAttribute("aria-level") ?? "0",
            ),
            off: (at.top + at.bottom) / 2 - (box.top + box.bottom) / 2,
            // The chevron is in the step before the name, inside the row.
            inside: at.left >= box.left - 1 && at.right <= box.right + 1,
            size: at.height,
          },
        ];
      }),
    );

    // A closed row and an open one at least, or this measured nothing.
    expect(twists.some((one) => one.open)).toBe(true);
    expect(twists.some((one) => !one.open)).toBe(true);
    const off = twists
      .filter((one) => Math.abs(one.off) > 1 || !one.inside)
      .map(
        (one) => `${one.what} (level ${one.level}, ${one.open ? "open" : "closed"}): ${one.off}px`,
      );
    expect(off).toEqual([]);
    // And it is a chevron of the tree's step, about 12px at the default text size, not a dot.
    const odd = twists.filter((one) => one.size < 8 || one.size > 20).map((one) => one.size);
    expect(odd).toEqual([]);
  });
});

/** A file's row in a branch's folder — or, with no piece, in the repo's own — once it is drawn. */
async function fileRow(
  piece: string | null,
  path: string,
  timeout = 20_000,
  repo = "svc",
): Promise<WebdriverIO.Element> {
  const row = await $(`[data-row="file:${repo}/${piece ?? ""}:${path}"]`).getElement();
  await row.waitForExist({
    timeout,
    timeoutMsg: `the explorer never drew ${path} under ${piece ?? repo}`,
  });
  return row;
}

/**
 * `selector` inside the explorer: the Explorer view or the Repos view, which is Explorer's
 * *Repos and branches* in the Panel since #1737. The file tab (FM-2) draws a branch's tree with
 * the same row ids, and a tab opened by an earlier spec is still there: a row the explorer
 * stopped drawing can still be in the file tab.
 */
function inExplorer(selector: string): string {
  return `:is([data-testid="explorer"], [data-testid="repos-view"]) ${selector}`;
}

/** A file's row in a branch's folder in the explorer, once it is drawn there. */
async function explorerRow(
  piece: string,
  path: string,
  timeout = 20_000,
): Promise<WebdriverIO.Element> {
  const row = await $(inExplorer(`[data-row="file:svc/${piece}:${path}"]`)).getElement();
  await row.waitForExist({
    timeout,
    timeoutMsg: `the explorer never drew ${path} under ${piece}`,
  });
  return row;
}

/** Waits until a file's row in a branch's folder in the explorer carries `mark`. */
async function marked(piece: string, path: string, mark: string): Promise<void> {
  await $(inExplorer(`[data-row="file:svc/${piece}:${path}"][data-mark="${mark}"]`)).waitForExist({
    // FSEvents gives no bound on when it delivers on a busy Mac (#577), so the wait is long.
    timeout: 60_000,
    timeoutMsg: `the explorer never marked ${path} as ${mark}`,
  });
}

/**
 * Opens the fixture branch's file tab from its row's menu — *Browse files* —
 * and waits for it to come forward.
 */
async function browseTheFiles(): Promise<void> {
  await $('[data-testid="piece-svc-fix-login"]').waitForExist({ timeout: 20_000 });
  expect(await sendContextMenu('.spot[data-testid="piece-svc-fix-login"]')).toBe(true);
  const menu = await $('[role="menu"]');
  await menu.waitForDisplayed({ timeout: 20_000 });
  await pressMenuPath(ROW.browseFiles);
  await browser.waitUntil(async () => (await tabInFront()) === "Files · fix-login", {
    timeout: 20_000,
    timeoutMsg: `the file tab never came forward; in front is ${await tabInFront()}`,
  });
}

/**
 * Drags the file tab's divider `by` pixels to the right, as the pointer events a drag sends.
 *
 * **Dispatched rather than performed**, for the reason `sendContextMenu` gives: a WebDriver
 * pointer action on the embedded driver raises no pointer events the page sees, measured on
 * macOS. What is sent is the sequence the platform would send — down on the divider's centre,
 * a move, and up — to the document `react-resizable-panels` listens on.
 */
async function dragTheDivider(by: number): Promise<boolean> {
  return browser.execute(async (dx: number) => {
    const handle = document.querySelector('[role="separator"][aria-label="Resize the file tree"]');
    if (!(handle instanceof HTMLElement)) return false;
    const box = handle.getBoundingClientRect();
    const x = box.left + box.width / 2;
    const y = box.top + box.height / 2;
    const at = (type: string, clientX: number, target: EventTarget) =>
      target.dispatchEvent(
        new PointerEvent(type, {
          bubbles: true,
          cancelable: true,
          pointerId: 1,
          pointerType: "mouse",
          isPrimary: true,
          button: 0,
          buttons: type === "pointerup" ? 0 : 1,
          clientX,
          clientY: y,
        }),
      );
    const frame = () => new Promise((done) => requestAnimationFrame(done));
    at("pointerdown", x, handle);
    await frame();
    for (let step = 1; step <= 6; step++) {
      at("pointermove", x + (dx * step) / 6, document);
      await frame();
    }
    at("pointerup", x + dx, document);
    return true;
  }, by);
}

/** Where a divider says it is, in percent. */
async function valueOf(divider: WebdriverIO.Element): Promise<number> {
  return Number(await divider.getAttribute("aria-valuenow"));
}

/** The file tabs a plane's record names, read off the disk. */
function filesTabsIn(record: string): { key: string; split?: number | null }[] {
  try {
    const views =
      (
        JSON.parse(readFileSync(record, "utf8")) as {
          views?: { view: string; key: string; split?: number | null }[];
        }
      ).views ?? [];
    return views.filter((one) => one.view === "piece-files");
  } catch {
    return [];
  }
}

/** What the tab in front is called. */
async function tabInFront(): Promise<string> {
  const name = await $('[data-strip="Tabs"] [role="tab"][aria-selected="true"] .tab-name');
  await name.waitForExist({ timeout: 20_000 });
  return name.getText();
}

/** The profile of the fake harness at a raw prompt, as `charter.local.toml` declares it. */
const RAW_PROMPT = "raw-prompt";

/** What each terminal on the page draws, its spaces as one: a terminal draws a no-break space. */
function paneRows(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll(".pane-frame .xterm-rows")].map((r) =>
      (r.textContent ?? "").replace(/\s+/g, " "),
    ),
  );
}

/**
 * Declares the {@link RAW_PROMPT} profile in project `plane`, once a run: the fake harness at a
 * raw prompt (it echoes each byte typed and answers a line only on Enter), of Claude Code's
 * kind, reporting its `SessionStart` the way Claude Code says it is at its prompt, so the core
 * types into it once it is waiting and its terminal is raw. A profile can be declared only once.
 */
function declareTheRawPrompt(plane: string): void {
  const local = join(plane, "charter.local.toml");
  const declared = readFileSync(local, "utf8");
  if (declared.includes(`[harness.${RAW_PROMPT}]`)) return;
  // Beside the project, not in it, as the touching harness above (#2071).
  const program = join(`${plane}.programs`, "raw-prompt-harness");
  mkdirSync(`${plane}.programs`, { recursive: true });
  // `SessionStart`, reported by the real `charter hook` with the conversation charter chose
  // (`--session-id`) and the harness's own pid, as `writeAPluginHookingShell` reports it.
  const sessionStart = [
    `printf '{"session_id":"%s","hook_event_name":"SessionStart","source":"startup"}'`,
    '"$CLAUDE_CODE_SESSION_ID"',
    '| CLAUDE_PID=$PPID "${CHARTER_HOOK_BINARY:-charter}" hook sessionstart >/dev/null',
  ].join(" ");
  writeFileSync(
    program,
    answeringAsClaudeCode([
      "#!/bin/sh",
      'while [ $# -gt 0 ]; do [ "$1" = "--session-id" ] && CLAUDE_CODE_SESSION_ID=$2; shift; done',
      "export CLAUDE_CODE_SESSION_ID",
      `exec ${singleQuoted(built("fake-harness"))} --sentinel ${singleQuoted(READY)} \\`,
      `  --hook ${singleQuoted(sessionStart)} --interactive --raw`,
      "",
    ]).join("\n"),
  );
  chmodSync(program, 0o755);
  writeFileSync(
    local,
    `${declared}\n[harness.${RAW_PROMPT}]\nkind = "claude"\ncommand = [${JSON.stringify(program)}]\n`,
  );
}

/**
 * Starts a chat on the {@link RAW_PROMPT} profile in project `plane`, declaring it first where
 * the run has not, and waits for it: open in the core, and a harness at its prompt on the page.
 */
async function startARawPromptChat(plane: string): Promise<void> {
  declareTheRawPrompt(plane);
  const before = (await rawPromptChats(plane)).length;
  await pressOnly("New tab");
  await harnessRowsDrawn();
  // The row's own option card (#2907): its radio, clicked as the driver's pointerless click
  // reaches it (React Aria takes a virtual press on the input, not on its label).
  const label = await $(`label*=${RAW_PROMPT}`);
  await browser.execute(
    (el: HTMLElement) => el.querySelector("input")?.click(),
    label as unknown as HTMLElement,
  );
  // Its first run asks to approve the command, a later one only starts. Pressed from the
  // document: the card the row draws makes the picker taller than WebDriver counts as on screen.
  await browser.waitUntil(
    () =>
      browser.execute(() => {
        const start = [...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')]
          .filter((button) => !button.disabled && button.getAttribute("aria-disabled") !== "true")
          .find((button) => ["Approve and start", "Start chat"].includes(button.textContent ?? ""));
        start?.click();
        return start !== undefined;
      }),
    { timeout: 20_000, timeoutMsg: "the picker offered no way to start the raw-prompt chat" },
  );
  await browser.waitUntil(async () => (await rawPromptChats(plane)).length > before, {
    timeout: 30_000,
    timeoutMsg: `no chat on ${RAW_PROMPT} opened`,
  });
  await browser
    .waitUntil(async () => (await paneRows()).some((text) => text.includes(READY)), {
      timeout: 30_000,
      timeoutMsg: "the raw-prompt chat never reached its prompt",
    })
    .catch(async (err: unknown) => {
      const said = await browser.execute(() =>
        [...document.querySelectorAll('[role="alert"],[role="status"],[role="dialog"]')]
          .map((el) => (el as HTMLElement).innerText)
          .join(" | "),
      );
      // What the window holds, so a red run says where the chat went: its frames, the tab in
      // front, and the core's own view of the chat.
      const frames = await browser.execute(() => ({
        frames: document.querySelectorAll(".pane-frame").length,
        terminals: document.querySelectorAll(".pane-frame .xterm").length,
        front: document.querySelector('[data-strip="Tabs"] [aria-selected="true"]')?.textContent,
      }));
      const chats = JSON.stringify(await rawPromptChats(plane));
      throw new Error(
        `${String(err)}; panes: ${(await paneRows()).join("|")}; window: ${JSON.stringify(frames)}; ` +
          `chats: ${chats}; said: ${said}`,
      );
    });
}

/**
 * The newest chat open on the {@link RAW_PROMPT} profile in project `plane`, by its number;
 * one is started where none is open.
 */
async function aRawPromptChat(plane: string): Promise<number> {
  if ((await rawPromptChats(plane)).length === 0) await startARawPromptChat(plane);
  const newest = (await rawPromptChats(plane)).at(-1);
  if (newest === undefined) throw new Error(`no chat is open on ${RAW_PROMPT} in ${plane}`);
  return newest.session;
}

/** The chats open on the {@link RAW_PROMPT} profile in project `plane`, oldest first. */
async function rawPromptChats(plane: string): Promise<{ session: number; name: string }[]> {
  type Open = { session: number; name: string; profile?: string | null };
  return (await ask<Open[]>("opened_chats", { plane }))
    .filter((chat) => chat.profile === RAW_PROMPT)
    .sort((a, b) => a.session - b.session);
}
