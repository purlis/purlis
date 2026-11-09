// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press } from "../acting.js";
import { focusWorkspace, workspaceTabs } from "../reach.js";
import { endChatsStartedHere, startAChat, type Started } from "../started.js";

/**
 * **A `purlis://` link in an artifact's page** (#5967, ADR 0120 §5 as amended), in the built app
 * against the real core: each of the four places goes where it names without asking (a
 * workspace focused, a todo's view opened, a chat shown, the chat working on an issue shown),
 * and a link naming an action, or a place that is not there, is refused with a sentence and goes
 * nowhere.
 *
 * **What it cannot reach**: the page's own webview, where the click is. The click's report is
 * made by the `e2e` build's `artifact_link_follow` as the page's script makes it, attributed to
 * the page's webview by its label (as `artifact-links.e2e.ts` does). The chat's lasting id is
 * read from the work link the spec gives it (`workspaces/alpha/work/`), which is also what the
 * issue link goes by.
 *
 * **It leaves the window as it found it**: the chat it starts it unlinks and ends, the todo it
 * adds it forgets, the tabs it opens it closes, and the artifact's files it removes.
 */

const TABS = '[data-strip="Tabs"]';
const GROUP = '[role="toolbar"][aria-label="Artifacts"]';
const DIALOG = '[role="dialog"]';
const NAME = "Release hub";
const TODO = "Linked from the hub";
const ISSUE = "github:github.com/purlis/purlis#5967";

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

/** The labels of the artifact webviews the app holds now. */
async function artifactWebviews(): Promise<string[]> {
  const all = await ask<{ label: string }[]>("plugin:webview|get_all_webviews");
  return all.map((one) => one.label).filter((label) => /^artifact-\d+$/.test(label));
}

/** The name of the tab in front. */
async function inFront(): Promise<string | null> {
  return browser.execute(
    (tabs: string) =>
      document.querySelector(`${tabs} [role="tab"][aria-selected="true"]`)?.textContent ?? null,
    TABS,
  );
}

/** Whether chat `session`'s pane is drawn now. */
async function paneShown(session: number): Promise<boolean> {
  return browser.execute(
    (n: number) =>
      [...document.querySelectorAll<HTMLElement>(`[data-testid="pane"][data-session="${n}"]`)].some(
        (one) => one.getBoundingClientRect().width > 0,
      ),
    session,
  );
}

/** The workspace focused now. */
async function focused(): Promise<string | undefined> {
  return (await workspaceTabs()).find((one) => one.selected)?.name;
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

/** The lasting id of the chat the work links in `dir` link to `item`. */
function chatLinkedTo(dir: string, item: string): string | undefined {
  for (const file of readdirSync(dir).filter((one) => one.endsWith(".jsonl"))) {
    for (const line of readFileSync(join(dir, file), "utf8").split("\n")) {
      if (line.trim() === "") continue;
      const said = JSON.parse(line) as { op?: string; item?: string; chat?: string };
      if (said.op === "link" && said.item === item && said.chat !== undefined) return said.chat;
    }
  }
  return undefined;
}

/** Brings the artifact's tab to the front, by its tab on the strip. */
async function artifactInFront(): Promise<void> {
  await focusWorkspace("alpha");
  if ((await inFront()) === NAME) return;
  await $(`${GROUP} button[aria-label="${NAME}"]`).click();
  await browser.waitUntil(async () => (await inFront()) === NAME, {
    timeout: 20_000,
    timeoutMsg: "the artifact's tab did not come to the front",
  });
}

/** The dialog that says `title`, once it is up. */
async function dialogSaying(title: string) {
  const dialog = $(DIALOG);
  await browser.waitUntil(
    async () => (await dialog.isExisting()) && (await dialog.getText()).includes(title),
    { timeout: 20_000, timeoutMsg: `no dialog saying "${title}" came up` },
  );
  return dialog;
}

describe("a purlis:// link in an artifact's page", function () {
  this.timeout(240_000);

  let plane = "";
  let artifacts = "";
  let slug = "";
  let chat: Started | undefined;
  let chatId = "";
  let label = "";

  before(async () => {
    plane = (await ask<string[]>("open_planes"))[0];
    await ask("todo_add", { plane, workspace: "alpha", text: TODO });
    slug = slugOf(join(plane, "workspaces", "alpha", "todos"), TODO);

    await focusWorkspace("alpha");
    chat = await startAChat();
    await ask("chat_work_link", { plane, session: chat.session, item: ISSUE });
    const linked = chatLinkedTo(join(plane, "workspaces", "alpha", "work"), ISSUE);
    if (linked === undefined) throw new Error("the chat's work link was not written");
    chatId = linked;

    artifacts = join(plane, "workspaces", "alpha", "artifacts");
    mkdirSync(join(artifacts, "release-hub"), { recursive: true });
    writeFileSync(
      join(artifacts, "release-hub", "index.html"),
      `<!doctype html><title>${NAME}</title>
<a href="purlis://workspace/beta">Beta</a>
<a href="purlis://workspace/nowhere">Nowhere</a>
<a href="purlis://todo/alpha/${slug}">The todo</a>
<a href="purlis://todo/alpha/${slug}/done">Mark it done</a>
<a href="purlis://chat/${chatId}">The chat</a>
<a href="purlis://issue/github:github.com/purlis/purlis%235967">The issue</a>
<a href="purlis://run/regenerate">Regenerate</a>`,
    );
    writeFileSync(
      join(artifacts, "pins.toml"),
      `version = 1
pin = [
  { id = "e2e000000006", folder = "release-hub", name = "${NAME}", icon = "chart", order = 1 },
]
`,
    );
    const icon = await $(`${GROUP} button[aria-label="${NAME}"]`);
    await icon.waitForExist({ timeout: 30_000 });
    await icon.click();
    await browser.waitUntil(async () => (await artifactWebviews()).length === 1, {
      timeout: 20_000,
      timeoutMsg: "the artifact's page did not open",
    });
    label = (await artifactWebviews())[0];
  });

  after(async () => {
    await focusWorkspace("alpha");
    for (const name of [NAME, TODO]) {
      const close = await $(`${TABS} button[aria-label="Close ${name}"]`);
      if (await close.isExisting()) await close.click();
    }
    rmSync(artifacts, { recursive: true, force: true });
    if (chat !== undefined) {
      await ask("chat_work_unlink", { plane, session: chat.session }).catch(() => undefined);
      await endChatsStartedHere([chat]);
    }
    await ask("todo_forget", { plane, workspace: "alpha", slug }).catch(() => undefined);
  });

  it("focuses the workspace a workspace link names, without asking", async () => {
    await artifactInFront();
    await ask("artifact_link_follow", { label, href: "purlis://workspace/beta" });
    await browser.waitUntil(async () => (await focused()) === "beta", {
      timeout: 20_000,
      timeoutMsg: "the workspace link did not focus beta",
    });
    expect(await $(DIALOG).isExisting()).toBe(false);
  });

  it("opens the todo a todo link names, in its view", async () => {
    await artifactInFront();
    await ask("artifact_link_follow", { label, href: `purlis://todo/alpha/${slug}` });
    await browser.waitUntil(async () => (await inFront()) === TODO, {
      timeout: 20_000,
      timeoutMsg: "the todo link did not open the todo's view in front",
    });
    expect(await $(DIALOG).isExisting()).toBe(false);
  });

  it("shows the chat a chat link names", async () => {
    const session = chat?.session ?? -1;
    await artifactInFront();
    expect(await paneShown(session)).toBe(false);
    await ask("artifact_link_follow", { label, href: `purlis://chat/${chatId}` });
    await browser.waitUntil(async () => paneShown(session), {
      timeout: 20_000,
      timeoutMsg: "the chat link did not show the chat",
    });
  });

  it("shows the chat that works on the issue an issue link names", async () => {
    const session = chat?.session ?? -1;
    await artifactInFront();
    expect(await paneShown(session)).toBe(false);
    await ask("artifact_link_follow", {
      label,
      href: "purlis://issue/github:github.com/purlis/purlis%235967",
    });
    await browser.waitUntil(async () => paneShown(session), {
      timeout: 20_000,
      timeoutMsg: "the issue link did not show the chat working on it",
    });
  });

  for (const [href, said] of [
    ["purlis://run/regenerate", "`run` is none of them"],
    ["purlis://todo/alpha/SLUG/done", "names more after it (`done`)"],
    ["purlis://workspace/nowhere", "This project has no workspace nowhere"],
  ] as const) {
    it(`refuses ${href.replace("SLUG", "<todo>")} with a sentence, and goes nowhere`, async () => {
      await artifactInFront();
      await ask("artifact_link_follow", { label, href: href.replace("SLUG", slug) });
      const dialog = await dialogSaying("Link not opened");
      expect(await dialog.getText()).toContain(said);
      await press(DIALOG, "Close");
      await browser.waitUntil(async () => !(await $(DIALOG).isExisting()), {
        timeout: 20_000,
        timeoutMsg: "the dialog did not close",
      });
      expect(await inFront()).toBe(NAME);
      expect(await focused()).toBe("alpha");
    });
  }
});
