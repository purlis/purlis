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
 * **The calls themselves, end to end** (#5978's review): WebDriver cannot reach the page's own
 * webview, so the page says what it was answered the one way it can reach purlis's window, by
 * what it offers (#5972). It asks `purlis.todo_list@1`, which it declared, and offers Mark done
 * for the todo of this spec's own it finds in the answer: a button in purlis's bar is that real
 * data came back. It asks `purlis.session_record_list@1`, which it did not declare, and offers
 * a row named by the error it was answered: the bar's refusal of that row names the kind.
 *
 * **Not here: an ungranted plane query.** No query of purlis's own reads the whole plane; only
 * an extension's can, so refusing one without its grant needs an extension installed and
 * approved on the machine. That is proved below the window, in `artifacts/calls.rs`,
 * `purlis_core::artifact::calls` and `purlis_core::extension::queries`.
 */

for (const ROUND of [1, 2, 3, 4, 5, 6, 7, 8]) {
// [DEBUG-6261] repeated, each round its own page, pin and todo.
const NAME = `Todo board ${ROUND}`;
const DECLARED = `{"api":1,"queries":{"purlis.todo_list@1":["done"],"purlis.change_status@1":[]}}`;
const PAGE = `<!doctype html><title>${NAME}</title><script type="application/json" id="purlis">${DECLARED}</script><ul id=todos></ul><script src=app.js></script>`;
const OFFERS = `[role="group"][aria-label="Offered by ${NAME}"]`;
/** The todo of this spec's own the page finds in its answer. */
const TODO = `Queried by the board ${ROUND}`;
/**
 * The page's script: its two calls, and each answer said as an offer (see above). The offers
 * answer on the port too, under id 99, which it passes over.
 */
const SCRIPT = `const said = {};
purlis.port.onmessage = (event) => {
  const answer = event.data;
  if (answer.id === 1) {
    const mine = ((answer.data && answer.data.todos) || []).find((one) => one.title === ${JSON.stringify(TODO)});
    said[1] = { does: "todo.done", subject: mine ? mine.slug : "declared-" + (answer.error ? answer.error.kind : "no-todo") };
  } else if (answer.id === 2) {
    said[2] = { does: "todo.done", subject: "undeclared-" + (answer.error ? answer.error.kind : "answered") };
  } else return;
  purlis.port.postMessage({ id: 99, offers: Object.values(said) });
};
purlis.port.postMessage({ id: 1, query: "purlis.todo_list@1" });
purlis.port.postMessage({ id: 2, query: "purlis.session_record_list@1" });
// round ${ROUND}
`;

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

describe(`a pinned artifact that calls purlis's queries, round ${ROUND}`, function () {
  this.timeout(180_000);

  let at = "";
  const bar = async () => $('[data-testid="artifact-bar"]').getText();

  before(async () => {
    const plane = (await ask<string[]>("open_planes"))[0];
    at = join(plane, "workspaces", "alpha", "artifacts");
    const folder = join(at, `todo-board-${ROUND}`);
    mkdirSync(folder, { recursive: true });
    writeFileSync(join(folder, "index.html"), PAGE);
    writeFileSync(join(folder, "app.js"), SCRIPT);
    await ask("todo_add", { plane, workspace: "alpha", text: TODO });
    writeFileSync(
      join(at, "pins.toml"),
      `version = 1
pin = [
  { id = "e2e00000003${ROUND}", folder = "todo-board-${ROUND}", name = "${NAME}", icon = "chart", order = 1 },
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

  it("answers a call its page declared with the workspace's own data", async () => {
    await $(OFFERS).$(`aria/Mark done: ${TODO}`).waitForExist({
      timeout: 20_000,
      timeoutMsg: "the page was not answered its declared purlis.todo_list@1 with the todo",
    });
  });

  it("refuses a call its page did not declare, as not-approved", async () => {
    await browser.waitUntil(
      async () =>
        (
          await browser.execute(() =>
            [...document.querySelectorAll('[data-testid="artifact-offer-refused"]')].map(
              (one) => one.textContent ?? "",
            ),
          )
        ).some((said) => said.includes("undeclared-not-approved")),
      {
        timeout: 20_000,
        timeoutMsg:
          "the page's undeclared purlis.session_record_list@1 was not refused not-approved",
      },
    );
  });
});
}
