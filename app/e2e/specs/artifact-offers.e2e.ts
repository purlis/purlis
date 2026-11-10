// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { focusWorkspace } from "../reach.js";
import { closeTabOf, iconFor, iconOf } from "../artifactTabs.js";

/**
 * **A pinned artifact whose page offers actions** (#5972, ADR 0122 §8), in the built app against
 * the real core: the page offers, purlis draws the offer in its own bar in the catalogue's
 * words, and only the person's press there does it. A delete asks first, and an id a page may
 * not offer is refused with its sentence.
 *
 * **The page cannot perform**: its script posts its offers as soon as it runs, and nothing
 * changes on disk until the person presses. Before its script is approved, it offers nothing.
 */

const NAME = "Offering board";
const OFFERS = `[role="group"][aria-label="Offered by ${NAME}"]`;
/** The todos it adds, an open one and a done one: no word of more than three letters in common,
 *  or the core refuses the second as the same work as the first (`memstore::duplicate_of`). */
const TITLES = ["Polish the offering banner", "Retire stale gadget"];
const PAGE = `<!doctype html><title>${NAME}</title><p>Offers</p><script src=app.js></script>`;

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

/** The name of the tab in front. */
async function inFront(): Promise<string | null> {
  return browser.execute(
    () =>
      document.querySelector('[data-strip="Tabs"] [role="tab"][aria-selected="true"]')
        ?.textContent ?? null,
  );
}

/** The slug of the todo in `dir` whose file holds `title`. */
function slugOf(dir: string, title: string): string {
  const file = readdirSync(dir).find(
    (one) =>
      one.endsWith(".md") &&
      one !== "MEMORY.md" &&
      readFileSync(join(dir, one), "utf8").includes(title),
  );
  if (file === undefined) throw new Error(`no todo '${title}' in ${dir}`);
  return file.slice(0, -".md".length);
}

/** The slugs of the todos in `dir` whose file holds `title`, none when there is no `dir`. */
function slugsOf(dir: string, title: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter(
      (one) =>
        one.endsWith(".md") &&
        one !== "MEMORY.md" &&
        readFileSync(join(dir, one), "utf8").includes(title),
    )
    .map((one) => one.slice(0, -".md".length));
}

/** Forgets every todo of `alpha`'s titled one of `titles`, open or done: what an earlier
 *  attempt at this spec left, so it starts from none and leaves none. */
async function forgetTodos(plane: string, todos: string, titles: string[]): Promise<void> {
  for (const title of titles) {
    for (const slug of slugsOf(todos, title)) {
      await ask("todo_forget", { plane, workspace: "alpha", slug });
    }
    for (const slug of slugsOf(join(todos, "archive"), title)) {
      await ask("todo_forget_done", { plane, workspace: "alpha", slug });
    }
  }
}

describe("a pinned artifact whose page offers actions", function () {
  this.timeout(180_000);

  let plane = "";
  let at = "";
  let todos = "";
  let open = "";
  let done = "";

  before(async () => {
    plane = (await ask<string[]>("open_planes"))[0];
    todos = join(plane, "workspaces", "alpha", "todos");
    await forgetTodos(plane, todos, TITLES);
    await ask("todo_add", { plane, workspace: "alpha", text: "Polish the offering banner" });
    await ask("todo_add", { plane, workspace: "alpha", text: "Retire stale gadget" });
    open = slugOf(todos, "Polish the offering banner");
    done = slugOf(todos, "Retire stale gadget");
    await ask("todo_done", { plane, workspace: "alpha", slug: done });

    at = join(plane, "workspaces", "alpha", "artifacts");
    const folder = join(at, "offering-board");
    mkdirSync(folder, { recursive: true });
    writeFileSync(join(folder, "index.html"), PAGE);
    writeFileSync(
      join(folder, "app.js"),
      `purlis.port.postMessage({ id: 1, offers: [
        { does: "todo.done", subject: ${JSON.stringify(open)} },
        { does: "todo.delete", subject: ${JSON.stringify(done)} },
        { does: "workspace.remove", subject: "alpha" },
      ] });`,
    );
    writeFileSync(
      join(at, "pins.toml"),
      `version = 1
pin = [
  { id = "e2e000000005", folder = "offering-board", name = "${NAME}", icon = "chart", order = 1 },
]
`,
    );
    await focusWorkspace("alpha");
    await iconFor(NAME);
  });

  after(async () => {
    await closeTabOf(NAME);
    rmSync(at, { recursive: true, force: true });
    await forgetTodos(plane, todos, TITLES);
  });

  it("offers nothing while its script is not approved", async () => {
    await $(iconOf(NAME)).click();
    await browser.waitUntil(async () => (await inFront()) === NAME, {
      timeout: 20_000,
      timeoutMsg: "pressing the artifact did not open its tab in front",
    });
    await $('[data-testid="artifact-bar"]')
      .$("aria/Approve script")
      .waitForExist({ timeout: 20_000 });
    expect(await $(OFFERS).isExisting()).toBe(false);
  });

  it("draws the page's offers in purlis's bar once approved, and performs none of them", async () => {
    await $('[data-testid="artifact-bar"]').$("aria/Approve script").click();
    await $(OFFERS).waitForExist({
      timeout: 20_000,
      timeoutMsg: "the approved page's offers did not reach purlis's bar",
    });
    await $(OFFERS)
      .$("aria/Mark done: Polish the offering banner")
      .waitForExist({ timeout: 20_000 });
    expect(await $(OFFERS).$("aria/Delete done todo Retire stale gadget…").isExisting()).toBe(true);
    // The page offered them; nothing was done.
    expect(existsSync(join(todos, `${open}.md`))).toBe(true);
    expect(existsSync(join(todos, "archive", `${done}.md`))).toBe(true);
  });

  it("refuses an action a page may not offer, with its sentence", async () => {
    const refused = await $('[data-testid="artifact-offer-refused"]');
    await refused.waitForExist({ timeout: 20_000 });
    expect(await refused.getText()).toContain("A page cannot offer workspace.remove");
    expect(await $(OFFERS).$("aria/Delete workspace alpha…").isExisting()).toBe(false);
  });

  it("does an offer when the person presses it in the bar", async () => {
    await $(OFFERS).$("aria/Mark done: Polish the offering banner").click();
    await browser.waitUntil(() => !existsSync(join(todos, `${open}.md`)), {
      timeout: 20_000,
      timeoutMsg: "pressing the offer did not mark the todo done",
    });
    expect(existsSync(join(todos, "archive", `${open}.md`))).toBe(true);
  });

  it("asks before an offered delete, and Cancel deletes nothing", async () => {
    await $(OFFERS).$("aria/Delete done todo Retire stale gadget…").click();
    const dialog = await $('[role="dialog"]');
    await dialog.waitForExist({ timeout: 20_000 });
    expect(await dialog.getText()).toContain(`${NAME} offered this.`);
    await dialog.$("aria/Cancel").click();
    await dialog.waitForExist({ reverse: true, timeout: 20_000 });
    expect(existsSync(join(todos, "archive", `${done}.md`))).toBe(true);
  });
});
