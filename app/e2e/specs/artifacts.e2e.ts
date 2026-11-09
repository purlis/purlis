// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { copyFixturePlane } from "../harness.js";
import { focusWorkspace } from "../reach.js";
import { openThroughTheTrustQuestion } from "../opening.js";

/**
 * **A pinned artifact, opened in its private tab** (#5952, ADR 0120 §2–3), in the built app
 * against the real core: its icon in a group of its own on the left bar, a press that opens its
 * tab (or brings it forward), its page drawn in a native webview of its own laid over a frame
 * named by it, and a tab a project's record puts back that runs nothing until pressed.
 *
 * **What it cannot reach**: the page's own webview. WebDriver drives the window's webview, so a
 * key pressed inside the page, and what the page draws, are proved below the window: the
 * scheme's answers and the keys' hand-back in `artifacts.rs`, the frame's Esc and F6 in
 * `ArtifactTab.test.tsx`. Here the webview is seen as the core lists it, by its label.
 *
 * **It leaves the window as it found it**: the tab it opens it closes, the files it writes it
 * removes, and the project it opens it lets go of.
 */

const TABS = '[data-strip="Tabs"]';
const PROJECTS = '[data-strip="Projects"]';
const GROUP = '[role="toolbar"][aria-label="Artifacts"]';
const NAME = "Backlog state";
const PAGE = `<!doctype html><title>${NAME}</title><h1>Open issues: 12</h1>`;
const PINS = `version = 1
pin = [
  { id = "e2e000000001", folder = "backlog-state", name = "${NAME}", icon = "chart", order = 1 },
]
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

/** The labels of the artifact webviews the app holds now. */
async function artifactWebviews(): Promise<string[]> {
  const all = await ask<{ label: string }[]>("plugin:webview|get_all_webviews");
  return all.map((one) => one.label).filter((label) => /^artifact-\d+$/.test(label));
}

/** The names on the chat strip, left to right. */
async function tabNames(): Promise<string[]> {
  return browser.execute(() =>
    [
      ...(document.querySelector('[data-strip="Tabs"]')?.querySelectorAll('[role="tab"]') ?? []),
    ].map((tab) => tab.textContent ?? ""),
  );
}

/** The name of the tab in front. */
async function inFront(): Promise<string | null> {
  return browser.execute(
    () =>
      document.querySelector('[data-strip="Tabs"] [role="tab"][aria-selected="true"]')
        ?.textContent ?? null,
  );
}

/** The view tabs a plane's record names, read off the disk. */
function viewsIn(record: string): { view: string; key: string; active?: boolean }[] {
  try {
    return (JSON.parse(readFileSync(record, "utf8")) as { views?: [] }).views ?? [];
  } catch {
    return [];
  }
}

/** Writes artifact `backlog-state` into workspace alpha of `plane`, pinned. */
function pinned(plane: string): string {
  const at = join(plane, "workspaces", "alpha", "artifacts");
  mkdirSync(join(at, "backlog-state"), { recursive: true });
  writeFileSync(join(at, "backlog-state", "index.html"), PAGE);
  writeFileSync(join(at, "pins.toml"), PINS);
  return at;
}

describe("a pinned artifact", function () {
  this.timeout(180_000);

  let artifacts = "";

  before(async () => {
    const plane = (await ask<string[]>("open_planes"))[0];
    artifacts = pinned(plane);
    await focusWorkspace("alpha");
  });

  after(async () => {
    if ((await tabNames()).includes(NAME)) {
      await $(`${TABS} button[aria-label="Close ${NAME}"]`).click();
      await browser.waitUntil(async () => !(await tabNames()).includes(NAME), {
        timeout: 20_000,
        timeoutMsg: "the artifact's tab did not close",
      });
    }
    rmSync(artifacts, { recursive: true, force: true });
  });

  it("has an icon in a group of its own, after purlis's own icons, which do not move", async () => {
    const icon = await $(`${GROUP} button[aria-label="${NAME}"]`);
    await icon.waitForExist({ timeout: 30_000 });
    // In document order, the left column's own tab lists come first, then the divider, then
    // the group: nothing of purlis's is after it in its column.
    const order: string[] = await browser.execute(() => {
      const column = document.querySelector('.activity-column[data-side="left"]');
      return [
        ...(column?.querySelectorAll(
          '[role="tablist"], [data-scope="separator"], [role="toolbar"]',
        ) ?? []),
      ].map((one) => one.getAttribute("role") ?? "separator");
    });
    const group = order.indexOf("toolbar");
    expect(group).toBeGreaterThan(0);
    expect(order[group - 1]).toBe("separator");
    expect(order.slice(0, group - 1).every((one) => one === "tablist")).toBe(true);
  });

  it("opens its tab on a press, its page laid over a frame named by it", async () => {
    expect(await artifactWebviews()).toEqual([]);
    await $(`${GROUP} button[aria-label="${NAME}"]`).click();

    await browser.waitUntil(async () => (await inFront()) === NAME, {
      timeout: 20_000,
      timeoutMsg: "pressing the artifact did not open its tab in front",
    });
    const frame = await $(`[role="region"][aria-label="${NAME}"]`);
    await frame.waitForExist({ timeout: 20_000 });
    expect(await $('[data-testid="artifact-bar"]').getText()).toContain(NAME);
    await browser.waitUntil(async () => (await artifactWebviews()).length === 1, {
      timeout: 20_000,
      timeoutMsg: "no webview of its own was laid over the frame",
    });
    // Its icon says it is the one in front.
    expect(await $(`${GROUP} button[aria-label="${NAME}"]`).getAttribute("aria-current")).toBe(
      "page",
    );
  });

  it("is brought forward, not opened twice, when its icon is pressed again", async () => {
    const plane = (await ask<string[]>("open_planes"))[0];
    const record = join(plane, ".charter", "app", "reopen.json");
    await $(`${GROUP} button[aria-label="${NAME}"]`).click();
    await browser.waitUntil(
      async () =>
        viewsIn(record).find((one) => one.view === "artifact")?.key === "alpha/backlog-state",
      { timeout: 20_000, timeoutMsg: "the record never named the artifact's tab" },
    );
    expect(viewsIn(record).filter((one) => one.view === "artifact")).toHaveLength(1);
    expect((await tabNames()).filter((one) => one === NAME)).toHaveLength(1);
    expect(await artifactWebviews()).toHaveLength(1);
  });

  it("is a Tab stop named by the artifact", async () => {
    const frame = await $(`[role="region"][aria-label="${NAME}"]`);
    expect(await frame.getAttribute("tabindex")).toBe("0");
    await browser.execute(() =>
      (document.querySelector('[data-testid="artifact-frame"]') as HTMLElement | null)?.focus(),
    );
    expect(
      await browser.execute(() => document.activeElement?.getAttribute("data-testid") ?? null),
    ).toBe("artifact-frame");
  });

  it("lets its webview go when its tab closes", async () => {
    await $(`${TABS} button[aria-label="Close ${NAME}"]`).click();
    await browser.waitUntil(async () => (await artifactWebviews()).length === 0, {
      timeout: 20_000,
      timeoutMsg: "the artifact's webview outlived its tab",
    });
  });

  describe("put back by a project's record", () => {
    /** A project of this spec's own, whose record names the artifact's tab and no chat. */
    const other = (() => {
      const copied = copyFixturePlane();
      const renamed = join(dirname(copied), "artifact-back");
      renameSync(copied, renamed);
      pinned(renamed);
      mkdirSync(join(renamed, ".charter", "app"), { recursive: true });
      writeFileSync(
        join(renamed, ".charter", "app", "reopen.json"),
        `${JSON.stringify({
          version: 1,
          at: 0,
          chats: [],
          views: [
            {
              from: "",
              view: "artifact",
              key: "alpha/backlog-state",
              title: NAME,
              workspace: "alpha",
              at: 0,
              active: true,
            },
          ],
        })}\n`,
      );
      return realpathSync(renamed);
    })();

    after(async () => {
      if (!(await ask<string[]>("open_planes")).includes(other)) return;
      await ask("close_plane", { plane: other });
      await browser.waitUntil(async () => !(await ask<string[]>("open_planes")).includes(other), {
        timeout: 20_000,
        timeoutMsg: "the artifact-back project was not let go of",
      });
      await browser.waitUntil(
        async () => !(await $(`${PROJECTS} [role="tab"][aria-label="artifact-back"]`).isExisting()),
        { timeout: 20_000, timeoutMsg: "the artifact-back project's tab stayed on the strip" },
      );
    });

    it("comes back in front and runs nothing until Open is pressed", async () => {
      await $(`${PROJECTS} button[aria-label="Open a project…"]`).click();
      const box = await $('[data-setting="open-by-path"] input');
      await box.waitForDisplayed({ timeout: 20_000 });
      await box.addValue(other);
      await $('form.by-path button[type="submit"]').click();
      await openThroughTheTrustQuestion();

      const open = await $(`button=Open ${NAME}`);
      await open.waitForExist({ timeout: 30_000 });
      expect(await inFront()).toBe(NAME);
      expect(await $(`[role="region"][aria-label="${NAME}"]`).isExisting()).toBe(false);
      expect(await artifactWebviews()).toEqual([]);

      await open.click();
      await $(`[role="region"][aria-label="${NAME}"]`).waitForExist({ timeout: 20_000 });
      await browser.waitUntil(async () => (await artifactWebviews()).length === 1, {
        timeout: 20_000,
        timeoutMsg: "pressing Open laid no webview over the frame",
      });
    });
  });
});

/**
 * **A pinned artifact with script** (#5959, ADR 0120 §4): shown without its script until the
 * person approves it in purlis's bar, never in the page; an approval that holds through new data
 * and is asked again for any change to the template. What the page's script does once it runs is
 * the webview's, which WebDriver cannot reach: the CSP that lets it run only when approved is
 * proved in `artifacts.rs`, and the record in `purlis artifact approve-status`'s tests.
 */
describe("a pinned artifact with script", function () {
  this.timeout(180_000);

  const LIVE = "Live board";
  const SCRIPTED = `<!doctype html><title>${LIVE}</title><h1 id=n>0</h1><script src=app.js></script>`;
  let folder = "";

  /** What purlis's bar over the page says. */
  const bar = async () => $('[data-testid="artifact-bar"]').getText();
  /** What the frame's edge says of the page's script. */
  const edge = async () => $('[data-testid="artifact-edge"]').getAttribute("data-standing");

  /** Closes the tab, if it is open, and opens it again from its icon: a fresh read. */
  async function openAgain() {
    if ((await tabNames()).includes(LIVE)) {
      await $(`${TABS} button[aria-label="Close ${LIVE}"]`).click();
      await browser.waitUntil(async () => (await artifactWebviews()).length === 0, {
        timeout: 20_000,
        timeoutMsg: "the artifact's webview outlived its tab",
      });
    }
    await $(`${GROUP} button[aria-label="${LIVE}"]`).click();
    await browser.waitUntil(async () => (await inFront()) === LIVE, {
      timeout: 20_000,
      timeoutMsg: "pressing the artifact did not open its tab in front",
    });
  }

  before(async () => {
    const plane = (await ask<string[]>("open_planes"))[0];
    const at = join(plane, "workspaces", "alpha", "artifacts");
    folder = join(at, "live-board");
    mkdirSync(folder, { recursive: true });
    writeFileSync(join(folder, "index.html"), SCRIPTED);
    writeFileSync(join(folder, "app.js"), "document.getElementById('n').textContent = '12';");
    writeFileSync(
      join(at, "pins.toml"),
      `version = 1
pin = [
  { id = "e2e000000002", folder = "live-board", name = "${LIVE}", icon = "chart", order = 1 },
]
`,
    );
    await focusWorkspace("alpha");
    await $(`${GROUP} button[aria-label="${LIVE}"]`).waitForExist({ timeout: 30_000 });
  });

  after(async () => {
    if ((await tabNames()).includes(LIVE)) {
      await $(`${TABS} button[aria-label="Close ${LIVE}"]`).click();
    }
    rmSync(dirname(folder), { recursive: true, force: true });
  });

  it("is shown without its script, and asks only in purlis's bar", async () => {
    await openAgain();
    await browser.waitUntil(async () => (await bar()).includes("Has script, shown without it"), {
      timeout: 20_000,
      timeoutMsg: "the bar did not ask about the page's script",
    });
    expect(await edge()).toBe("asks");
    expect(await $("aria/Approve script").isExisting()).toBe(true);
  });

  it("runs once approved there, and its page is opened again under the approval", async () => {
    await $('[data-testid="artifact-bar"]').$("aria/Approve script").click();
    await browser.waitUntil(async () => (await bar()).includes("Script approved on this machine"), {
      timeout: 20_000,
      timeoutMsg: "the approval did not reach the tab",
    });
    expect(await edge()).toBe("runs");
    expect(await artifactWebviews()).toHaveLength(1);
  });

  it("stays approved through new data", async () => {
    writeFileSync(join(folder, "data.json"), '{"open": 12}');
    await openAgain();
    await browser.waitUntil(async () => (await bar()).includes("Script approved on this machine"), {
      timeout: 20_000,
      timeoutMsg: "new data asked again",
    });
  });

  it("asks again when its template changes, until approved anew", async () => {
    writeFileSync(join(folder, "app.js"), "document.getElementById('n').textContent = '13';");
    await openAgain();
    await browser.waitUntil(
      async () => (await bar()).includes("Its template changed since you approved it"),
      { timeout: 20_000, timeoutMsg: "a changed template ran under the old approval" },
    );
    expect(await edge()).toBe("asks");
    await $('[data-testid="artifact-bar"]').$("aria/Approve script").click();
    await browser.waitUntil(async () => (await bar()).includes("Script approved on this machine"), {
      timeout: 20_000,
      timeoutMsg: "the new approval did not reach the tab",
    });
  });
});
