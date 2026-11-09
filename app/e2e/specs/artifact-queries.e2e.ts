// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { focusWorkspace } from "../reach.js";
import { closeTabOf, iconFor, iconOf } from "../artifactTabs.js";

/**
 * **A pinned artifact whose page calls purlis's queries** (#5969, ADR 0122 §7), in the built app
 * against the real core: purlis's bar lists the queries the page declared and the parameters that
 * widen what they read, before and after the person approves its script.
 *
 * **What it cannot reach**: the page's own webview, so the calls themselves. That a declared
 * call is answered as data and an undeclared one `not-approved`, gated again at each ask, is
 * proved in `artifacts/calls.rs` and `purlis_core::artifact::calls`.
 */

const NAME = "Todo board";
const DECLARED = `{"api":1,"queries":{"purlis.todo_list@1":["done"],"purlis.change_status@1":[]}}`;
const PAGE = `<!doctype html><title>${NAME}</title><script type="application/json" id="purlis">${DECLARED}</script><ul id=todos></ul><script src=app.js></script>`;

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

describe("a pinned artifact that calls purlis's queries", function () {
  this.timeout(180_000);

  let at = "";
  const bar = async () => $('[data-testid="artifact-bar"]').getText();

  before(async () => {
    const plane = (await ask<string[]>("open_planes"))[0];
    at = join(plane, "workspaces", "alpha", "artifacts");
    const folder = join(at, "todo-board");
    mkdirSync(folder, { recursive: true });
    writeFileSync(join(folder, "index.html"), PAGE);
    writeFileSync(
      join(folder, "app.js"),
      "purlis.port.postMessage({ id: 1, query: 'purlis.todo_list@1' });",
    );
    writeFileSync(
      join(at, "pins.toml"),
      `version = 1
pin = [
  { id = "e2e000000003", folder = "todo-board", name = "${NAME}", icon = "chart", order = 1 },
]
`,
    );
    await focusWorkspace("alpha");
    await iconFor(NAME);
  });

  after(async () => {
    await closeTabOf(NAME);
    rmSync(at, { recursive: true, force: true });
  });

  it("lists in purlis's bar the queries its page declared, as its approval covers them", async () => {
    await $(iconOf(NAME)).click();
    await browser.waitUntil(async () => (await inFront()) === NAME, {
      timeout: 20_000,
      timeoutMsg: "pressing the artifact did not open its tab in front",
    });
    await browser.waitUntil(async () => (await bar()).includes("Reads purlis.todo_list@1"), {
      timeout: 20_000,
      timeoutMsg: "the bar did not list the queries the page declared",
    });
    const said = await bar();
    expect(said).toContain("purlis.todo_list@1 (done: the done todos, not only the open ones)");
    expect(said).toContain("purlis.change_status@1");
    expect(await $("aria/Approve script").isExisting()).toBe(true);
    // Every query it declares is of its workspace: nothing project-wide is asked.
    expect(await $("aria/Allow project-wide reads").isExisting()).toBe(false);
  });

  it("still lists them once its script is approved", async () => {
    await $('[data-testid="artifact-bar"]').$("aria/Approve script").click();
    await browser.waitUntil(async () => (await bar()).includes("Script approved on this machine"), {
      timeout: 20_000,
      timeoutMsg: "the approval did not reach the tab",
    });
    expect(await bar()).toContain("Reads purlis.todo_list@1");
  });
});
