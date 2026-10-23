// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { $, browser, expect } from "@wdio/globals";
import {
  closeTheViewTabsOpenedSince,
  focusWorkspace,
  projectInFront,
  runFromThePalette,
  tabNames,
} from "../reach.js";

/**
 * **What a task changed, in its own Changes tab, and the person's merge or discard of its
 * branch** (#1511, #1534), in the built app against the real core and real git.
 *
 * A task on its own branch needs a persona chat and a core that dispatched it, which the
 * scenario harness cannot make. What the Changes tab and its Merge and Discard act on is not
 * the chat, though: it is the dispatch's record and the branch purlis cut for it, both on the
 * disk. So this spec cuts two such branches off the fixture's `svc` clone exactly where and as
 * purlis cuts them (`.worktrees/svc/<task>-<the id's tail>`, the base recorded as
 * `branch.<piece>.charterBase`), commits one file on each, and writes the two ended tasks'
 * records into the project's dispatch store. From there everything is the app's: the
 * Dispatches tab lists them, Review changes opens the task's own Changes tab, which lists that
 * branch's file and no other's, Merge fast-forwards `main` to exactly the commit shown, and
 * Discard removes the other task's folder.
 *
 * **It leaves the project as it found it**: both view tabs are closed, `main` is put back where
 * it was, both branches and folders are removed, and both records are taken out of the store.
 */

const MERGED = {
  id: "01K7E2E0000000000000MERGE1",
  task: "merge me",
  piece: "merge-me-00merge1",
  file: "task-changes-merged.txt",
};
const DISCARDED = {
  id: "01K7E2E0000000000000DSCRD1",
  task: "drop me",
  piece: "drop-me-00dscrd1",
  file: "task-changes-dropped.txt",
};
type Task = typeof MERGED;

const DIALOG = '[role="alertdialog"]';

/** git, for this spec's own setup and checks: never the operator's configuration. */
function git(cwd: string, args: string[]): string {
  return execFileSync(
    "git",
    ["-c", "user.name=purlis scenario", "-c", "user.email=scenario@example.invalid", ...args],
    {
      cwd,
      stdio: "pipe",
      env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_SYSTEM: "/dev/null" },
    },
  )
    .toString()
    .trim();
}

/** git, where a failure means only that there was nothing to undo. */
function tidy(cwd: string, args: string[]): void {
  try {
    git(cwd, args);
  } catch {
    // Nothing there to put back.
  }
}

/** The project's state folder, by the spelling it has on the disk (`names::state_name`). */
function stateOf(plane: string): string {
  if (existsSync(join(plane, ".charter"))) return join(plane, ".charter");
  if (existsSync(join(plane, ".purlis"))) return join(plane, ".purlis");
  return join(plane, ".charter");
}

/** The dispatch record of an ended task on its own branch of `svc` in `alpha`. */
function recordOf(task: Task): string {
  return JSON.stringify({
    v: 1,
    id: task.id,
    mode: "task",
    asker: { chat: 9201, name: "its session", workspace: "alpha" },
    persona: "devops",
    worker: { chat: 9202, name: `devops ${task.task}`, persona: "devops" },
    task: task.task,
    place: {
      workspace: "alpha",
      worktree: { repo: "svc", piece: task.piece, branch: task.piece },
    },
    brief: `Write ${task.file}.`,
    started: "2026-10-12T08:00:00+00:00",
    ended: "2026-10-12T08:05:00+00:00",
    report: { outcome: "done", text: `Wrote ${task.file}.`, changed: {} },
  });
}

/** Presses the element `selector` names, by its own click from the page. */
async function press(selector: string): Promise<void> {
  await $(selector).waitForExist({ timeout: 20_000 });
  await browser.execute((one: string) => {
    document.querySelector<HTMLElement>(one)?.click();
  }, selector);
}

/** Presses the open question's button that says `says`. */
async function answer(says: string): Promise<void> {
  await $(DIALOG).waitForDisplayed({ timeout: 20_000 });
  await browser.waitUntil(
    async () =>
      browser.execute(
        (dialog: string, words: string) => {
          const button = [...document.querySelectorAll<HTMLButtonElement>(`${dialog} button`)].find(
            (one) =>
              one.textContent?.trim() === words &&
              !one.disabled &&
              one.getAttribute("aria-disabled") !== "true",
          );
          button?.click();
          return button !== undefined;
        },
        DIALOG,
        says,
      ),
    { timeout: 10_000, interval: 100, timeoutMsg: `the question never offered ${says}` },
  );
  await $(DIALOG).waitForDisplayed({ reverse: true, timeout: 20_000 });
}

describe("a task's own Changes tab, merged and discarded", function () {
  this.timeout(240_000);

  let wereAlreadyOpen: string[] = [];
  let plane = "";
  let svc = "";
  let mainWas = "";
  let tip = "";
  const store = () => join(stateOf(plane), "app", "dispatches");
  const folderOf = (task: Task) =>
    join(plane, "workspaces", "alpha", ".worktrees", "svc", task.piece);
  const changesTab = `[data-testid="view-pane-charter-task-changes-${MERGED.id}"]`;

  before(async () => {
    // The strip of the workspace the tabs below are filed on, read once that one is focused.
    await focusWorkspace("alpha");
    wereAlreadyOpen = await tabNames();
    plane = await projectInFront();
    svc = join(plane, "workspaces", "alpha", "svc");
    if (!existsSync(join(svc, ".git"))) throw new Error(`${svc} is not the run's clone`);
    mainWas = git(svc, ["rev-parse", "main"]);

    for (const task of [MERGED, DISCARDED]) {
      const folder = folderOf(task);
      git(svc, ["worktree", "add", "-q", "-b", task.piece, folder, "main"]);
      git(svc, ["config", "--replace-all", `branch.${task.piece}.charterBase`, "main"]);
      writeFileSync(join(folder, task.file), `written for ${task.task}\n`);
      git(folder, ["add", task.file]);
      git(folder, ["commit", "-q", "-m", `${task.task}: ${task.file}`]);
    }
    tip = git(folderOf(MERGED), ["rev-parse", "HEAD"]);

    mkdirSync(store(), { recursive: true });
    for (const task of [MERGED, DISCARDED])
      writeFileSync(join(store(), `${task.id}.json`), recordOf(task));

    await runFromThePalette("Open dispatches", "dispatches.show");
    await $(`[data-testid="dispatch-${MERGED.id}"]`).waitForExist({
      timeout: 30_000,
      timeoutMsg: "the Dispatches tab never listed the task's record",
    });
  });

  after(async () => {
    await closeTheViewTabsOpenedSince(wereAlreadyOpen);
    for (const task of [MERGED, DISCARDED])
      rmSync(join(store(), `${task.id}.json`), { force: true });
    if (svc === "") return;
    // `main` back where it was, with the folder as it was: the clone has `main` out.
    if (mainWas !== "") tidy(svc, ["reset", "-q", "--hard", mainWas]);
    for (const task of [MERGED, DISCARDED]) {
      tidy(svc, ["worktree", "remove", "--force", folderOf(task)]);
      rmSync(folderOf(task), { recursive: true, force: true });
    }
    tidy(svc, ["worktree", "prune"]);
    for (const task of [MERGED, DISCARDED]) {
      tidy(svc, ["branch", "-D", task.piece]);
      tidy(svc, ["config", "--unset-all", `branch.${task.piece}.charterBase`]);
    }
  });

  it("lists each task's own branch on its row, with Review changes", async () => {
    for (const task of [MERGED, DISCARDED]) {
      const row = `[data-testid="dispatch-${task.id}"]`;
      await expect($(`${row} .dispatch-worktree`)).toHaveText(
        expect.stringContaining("own branch, folder kept"),
      );
      await expect($(`${row} button[aria-label="Review changes of ${task.task}"]`)).toExist();
    }
  });

  it("opens the task's own Changes tab, listing its branch's file and no other task's", async () => {
    await press(`button[aria-label="Review changes of ${MERGED.task}"]`);
    await browser.waitUntil(async () => (await tabNames()).includes(`Changes · ${MERGED.task}`), {
      timeout: 20_000,
      timeoutMsg: "Review changes opened no tab",
    });
    const tab = await $(`${changesTab} [data-testid="task-changes"]`);
    await tab.waitForExist({ timeout: 30_000, timeoutMsg: "the task's Changes tab never read" });

    const files = await browser.execute(
      (pane: string) =>
        [...document.querySelectorAll(`${pane} .task-changes-files li`)].map(
          (one) => one.textContent ?? "",
        ),
      changesTab,
    );
    expect(files).toHaveLength(1);
    expect(files[0]).toContain(MERGED.file);
    expect(files.join(" ")).not.toContain(DISCARDED.file);
    await expect(tab).toHaveText(
      expect.stringContaining(`It worked on its own branch, ${MERGED.piece} in svc.`),
    );
  });

  it("merges the branch it was shown into main, by a fast-forward, once asked", async () => {
    const pressed = await browser.execute((pane: string) => {
      const merge = [
        ...document.querySelectorAll<HTMLElement>(`${pane} .task-changes-acts button`),
      ].find((one) => one.textContent?.trim() === "Merge…");
      merge?.click();
      return merge !== undefined;
    }, changesTab);
    expect(pressed).toBe(true);

    await $(DIALOG).waitForDisplayed({ timeout: 20_000 });
    await expect($(DIALOG)).toHaveText(expect.stringContaining("Merge this task's branch?"));
    await expect($(DIALOG)).toHaveText(
      expect.stringContaining(`This lands 1 commit of ${MERGED.piece}`),
    );
    await answer("Merge branch");

    await browser.waitUntil(async () => git(svc, ["rev-parse", "main"]) === tip, {
      timeout: 20_000,
      interval: 250,
      timeoutMsg: "main never reached the task's commit",
    });
    // Said where it was pressed.
    await expect($(changesTab)).toHaveText(expect.stringContaining(`${MERGED.piece} is merged`));
  });

  it("discards the other task's folder from its Dispatches row, once asked", async () => {
    // The Dispatches tab is still on the strip, behind the Changes tab.
    await browser.execute(() => {
      const tab = [
        ...document.querySelectorAll<HTMLElement>('[data-strip="Tabs"] [role="tab"]'),
      ].find((one) => one.querySelector(".tab-name")?.textContent === "Dispatches");
      tab?.click();
    });
    await press(`button[aria-label="Discard the branch folder of ${DISCARDED.task}"]`);
    await $(DIALOG).waitForDisplayed({ timeout: 20_000 });
    await expect($(DIALOG)).toHaveText(expect.stringContaining("Discard this branch's folder?"));
    await answer("Discard folder");

    await browser.waitUntil(async () => !existsSync(folderOf(DISCARDED)), {
      timeout: 20_000,
      interval: 250,
      timeoutMsg: "the discarded folder is still on the disk",
    });
    // Nothing of it reached main.
    expect(git(svc, ["rev-parse", "main"])).toBe(tip);
    await expect($(`[data-testid="dispatch-${DISCARDED.id}"] .dispatch-worktree`)).toHaveText(
      expect.stringContaining("own branch, folder discarded"),
    );
  });
});
