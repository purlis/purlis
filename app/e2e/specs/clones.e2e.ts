// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import {
  appendFileSync,
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { focusWorkspace, press, theProjectsFoldersAreAccepted, untilSays } from "../acting.js";
import {
  type FakeGitLab,
  gitConfigFor,
  OWNER,
  type Served,
  startFakeGitLab,
} from "../fakeGitLab.js";
import {
  theRunsStandIns,
  THE_RUNS_CLONE_STALL,
  THE_RUNS_FAKE_FORGE,
  THE_RUNS_GIT_CONFIG,
  THE_RUNS_TREE,
} from "../harness.js";
import { showView } from "../opening.js";
import { type Exchange, noForgeAnswers, theForgeAnswers } from "../recordedForge.js";
import {
  ask,
  aProjectOfItsOwn,
  closeTheProjectsOfItsOwn,
  inFront,
  openFromTheStrip,
} from "../switching.js";

/**
 * **A repo cloned from a self-hosted GitLab, in the built window, over the network** (#4090,
 * #4159; the report behind them is #4079's "clone failed — no access, network, or gh isn't
 * authed").
 *
 * The forge (`fakeGitLab.ts`) is a TLS server on a self-hosted host name and a port, whose git is
 * git's own smart HTTP and refuses anything but a token it took. So what reaches it is the window's
 * real `clone_repo`, a real `git clone`, and the app's binary started again by git as its
 * credential helper, reading the account's keyring item.
 *
 * **What is a seam, and why** (`src-tauri/src/e2eforge.rs`, `purlis-core/src/cloning.rs`), none of
 * it in a shipped build:
 *
 * - **Sign in with GitLab signs in to this host.** A shipped build registers gitlab.com alone, and
 *   a self-managed GitLab takes a token (ADR 0077). The flow, listener, exchange and account store
 *   are the shipped ones, and the last `it` proves the shipped token sign-in on this same host.
 * - **The person's browser is this spec**: the page the flow would open is written to a file, and
 *   the forge answers it.
 * - **The account's API calls come from the run's recording** (`recordedForge.ts`): they are not
 *   what is being proved, and git's are never recorded.
 * - **The stall limit is shortened for its one case.** Nothing offers a clock for "the old 120
 *   seconds", which is no limit in the code any more, so that case is honest: the clone takes over
 *   two minutes.
 *
 * **No forge CLI**: `gh` and `glab` on the app's `PATH` are stand-ins that note being run and fail,
 * since a runner's own `gh` cannot be taken off it, and the last `it` reads the note.
 *
 * Nothing is left behind: the project is one of this spec's own, closed at the end, and the run's
 * git config, stand-ins, accounts and recording are put back.
 */

const HOST = "gitlab.purlis-e2e.test";

/** Long enough that the old limit would have stopped it: 120 seconds. */
const SLOWLY_PAST_THE_OLD_LIMIT = 125;

/** Slow enough to press Cancel part way. */
const SLOWLY = 45;

/** A clone's stall limit in its one case, in seconds. */
const SHORT_STALL = "20";

const WHERE_SHOTS_GO = resolve("logs", "clone-screenshots");
const CLI_NOTE = join(THE_RUNS_TREE, "forge-cli-was-run");
const PAGE = join(THE_RUNS_TREE, "fake-forge-page");

const OPEN_ONE = '[data-strip="Projects"] button[aria-label="Open a project…"]';
const DOORS = 'ul[aria-label="Ways to start"]';
const WELCOME = "section.welcome";
const PALETTE = '[role="dialog"][aria-label="Command palette"]';
const SIGN_IN_DOOR = "Sign in to GitHub or GitLab";

/** The card on `workspace`'s chip, and nothing the status bar's Notices or the Inbox draw. */
const cardOf = (workspace: string) => `.chip-card:has([data-chip="${workspace}"])`;
const cardMeterOf = (workspace: string) => `${cardOf(workspace)} .action-result-meter`;

type Account = { id: string; host: string; method: string };

/** The repos the forge has, each served its own way and named for it. */
const REPOS: Record<string, Served & { files: number }> = {
  "first-repo": { way: "whole", files: 3 },
  "second-repo": { way: "whole", files: 3 },
  "third-repo": { way: "whole", files: 3 },
  "fourth-repo": { way: "whole", files: 3 },
  "token-repo": { way: "whole", files: 3 },
  "long-repo": { way: "slowly", seconds: SLOWLY_PAST_THE_OLD_LIMIT, files: 400 },
  "stopped-on-its-row": { way: "slowly", seconds: SLOWLY, files: 200 },
  "stopped-on-its-card": { way: "slowly", seconds: SLOWLY, files: 200 },
  "gone-silent": { way: "silently", files: 3 },
};

/** A project's GitLab listing of the forge's repos, in the shape the core's recording has. */
function theListing(forge: FakeGitLab): Exchange[] {
  const get = (path: string, out: unknown): Exchange => ({
    call: { endpoint: { rest: { method: null, path } }, fields: [] },
    reply: { code: 0, out: JSON.stringify(out), err: "", status: 200 },
  });
  const names = Object.keys(REPOS);
  return [
    get("user", { id: 1, username: "jdoe", name: "J Doe", state: "active" }),
    get("personal_access_tokens/self", {
      id: 4,
      name: "purlis",
      revoked: false,
      created_at: "2026-10-01T09:00:00.000Z",
      scopes: ["api"],
      user_id: 1,
      last_used_at: null,
      active: true,
      expires_at: "2027-10-01",
    }),
    get(
      "projects?membership=true&archived=false&per_page=100&page=1",
      names.map((name, id) => ({
        id: id + 100,
        name,
        path: name,
        path_with_namespace: `${OWNER}/${name}`,
        default_branch: "main",
        description: null,
        web_url: forge.webUrl(name),
        ssh_url_to_repo: `git@${HOST}:${OWNER}/${name}.git`,
        topics: [],
      })),
    ),
    ...names.map((_, id) =>
      get(`projects/${id + 100}/repository/tree?per_page=100&page=1`, [
        { id: "x0", name: "file-0.bin", type: "blob", path: "file-0.bin", mode: "100644" },
      ]),
    ),
  ];
}

/** The plane's forge and inventory: what `purlis discover` would have left for this forge. */
function theProjectKnowsTheForge(plane: string, forge: FakeGitLab): void {
  appendFileSync(
    join(plane, "charter.toml"),
    `\n[[forge]]\nkind = "gitlab"\nhost = "${forge.server}"\nowner = "${OWNER}"\n`,
  );
  mkdirSync(join(plane, "inventory"), { recursive: true });
  const repos = Object.keys(REPOS).map((name) => ({
    name,
    forge: "gitlab",
    path_with_namespace: `${OWNER}/${name}`,
    web_url: forge.webUrl(name),
    ssh_url: `git@${HOST}:${OWNER}/${name}.git`,
    default_branch: "main",
  }));
  writeFileSync(
    join(plane, "inventory", "repos.json"),
    JSON.stringify({ group: OWNER, count: repos.length, repos }, null, 2),
  );
}

/** A `gh` or `glab` that notes it was run, and fails as a CLI nobody logged in to does. */
function aCliThatNotes(name: string): string {
  const at = join(theRunsStandIns(), name);
  writeFileSync(at, `#!/bin/sh\necho "${name} $*" >> '${CLI_NOTE}'\nexit 1\n`);
  chmodSync(at, 0o755);
  return at;
}

/**
 * The stand-ins' notes that touch a clone: a credential ask, or anything naming this forge. The
 * window's own look at which CLIs are logged in (`gh auth status --hostname github.com`) runs
 * whoever is signed in, about the default hosts, and signs nothing.
 */
function cliRunsForThisForge(): string[] {
  if (!existsSync(CLI_NOTE)) return [];
  return readFileSync(CLI_NOTE, "utf8")
    .split("\n")
    .filter((line) => line.includes("git-credential") || line.includes(HOST));
}

/** The workspace `name`, naming `repos`, none of them cloned yet, focused. */
async function aWorkspaceNaming(plane: string, name: string, repos: string[]): Promise<void> {
  await ask("workspace_create", { plane, name, vision: null, live: false });
  const manifest = join(plane, "workspaces", name, "workspace.json");
  const doc = JSON.parse(readFileSync(manifest, "utf8")) as Record<string, unknown>;
  writeFileSync(
    manifest,
    JSON.stringify({ ...doc, repos: repos.map((repo) => ({ name: repo })) }, null, 2),
  );
  await focusWorkspace(name);
  await showView("Repos");
  for (const repo of repos)
    await $(rowOf(repo)).waitForExist({
      timeout: 30_000,
      timeoutMsg: `${repo} was never drawn as not cloned in ${name}`,
    });
}

/** One card stands at a time, and the status bar's Notices would stand over the chip's. */
async function theStatusBarsCardIsPutAway(): Promise<void> {
  await browser.execute(() => {
    for (const close of document.querySelectorAll<HTMLElement>(
      '[data-chip="status/bar/notices"] button[aria-label="Dismiss"]',
    ))
      close.click();
  });
}

/** The workspace `name` made in its dialog with `repos` picked: its clones run on its chip. */
async function aNewWorkspaceCloning(name: string, repos: string[]): Promise<void> {
  // Before the card comes: a press anywhere else hides it.
  await showView("Repos");
  await theStatusBarsCardIsPutAway();
  await browser.keys(["F2"]);
  await $(PALETTE).waitForDisplayed({ timeout: 20_000 });
  const box = await $("#palette-query");
  await browser.waitUntil(async () => box.isFocused(), {
    timeout: 10_000,
    timeoutMsg: "the palette did not take the keyboard when it opened",
  });
  await box.addValue("New workspace");
  await browser.waitUntil(
    async () =>
      browser.execute(
        () =>
          document.querySelector('[role="option"] .palette-title')?.textContent ===
          "New workspace…",
      ),
    { timeout: 20_000, timeoutMsg: "the palette never put New workspace… first" },
  );
  await browser.keys(["Enter"]);
  const name_ = await $('[role="dialog"] input');
  await name_.waitForDisplayed({ timeout: 20_000 });
  await name_.setValue(name);
  await untilSays('[data-testid="repo-picker"]', repos[0]);
  for (const repo of repos) {
    const picked = await browser.execute((words: string) => {
      const label = [
        ...document.querySelectorAll<HTMLElement>('[data-testid="repo-picker"] label'),
      ].find((one) => (one.textContent ?? "").trim().startsWith(words));
      // The input's to take, not its label's: the driver presses without a pointer (`footer`).
      const box =
        label?.querySelector("input") ?? document.getElementById(label?.getAttribute("for") ?? "");
      box?.click();
      return box !== null && box !== undefined;
    }, repo);
    if (!picked) throw new Error(`the repo picker offers no ${repo}`);
  }
  await press('[role="dialog"]', "Create workspace");
}

/** Presses the tab of the project at `path`. */
async function theTabOf(path: string): Promise<void> {
  await browser.execute(
    (at: string) =>
      document
        .querySelector<HTMLElement>(`[data-strip="Projects"] [role="tab"][data-directory="${at}"]`)
        ?.click(),
    path,
  );
}

/** A repo's row among the ones not cloned here. */
const rowOf = (repo: string) => `[data-testid="absent-${repo}"]`;

/** A repo's folder and its hidden one, under `plane`'s workspace. */
function foldersOf(plane: string, workspace: string, repo: string) {
  const dir = join(plane, "workspaces", workspace);
  return { landed: join(dir, repo), hidden: join(dir, `.cloning-${repo}`) };
}

/** A checkout that holds what the forge served, by git's own say. */
function aWholeClone(at: string): void {
  const said = execFileSync("git", ["-C", at, "rev-parse", "--verify", "HEAD"], {
    env: { PATH: process.env.PATH, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" },
    encoding: "utf8",
  });
  expect(said.trim()).toMatch(/^[0-9a-f]{40}$/);
  expect(existsSync(join(at, "file-0.bin"))).toBe(true);
}

async function untilLanded(at: string, within: number, why: string): Promise<void> {
  await browser.waitUntil(() => existsSync(join(at, ".git")), {
    timeout: within,
    interval: 500,
    timeoutMsg: `${why}: nothing landed at ${at}`,
  });
}

/** What a meter says now, by its `aria-valuenow`; `null` with none drawn. */
async function percentOf(css: string): Promise<number | null> {
  return browser.execute((meter: string) => {
    const now = document.querySelector(meter)?.getAttribute("aria-valuenow");
    return now === null || now === undefined ? null : Number(now);
  }, css);
}

/** A row's own meter. */
const meterOf = (repo: string) => `${rowOf(repo)} .clone-bar`;

/** The window as it is, and each of `parts` alone, for a person to look at. */
async function pictures(name: string, parts: Record<string, string>): Promise<void> {
  mkdirSync(WHERE_SHOTS_GO, { recursive: true });
  await browser.saveScreenshot(join(WHERE_SHOTS_GO, `${name}.png`));
  for (const [part, css] of Object.entries(parts))
    await (await $(css)).saveScreenshot(join(WHERE_SHOTS_GO, `${name}-${part}.png`));
}

/** Nothing of the repo anywhere in the workspace, finished or hidden. */
function nothingOf(plane: string, workspace: string, repo: string): void {
  const { landed, hidden } = foldersOf(plane, workspace, repo);
  const there = readdirSync(join(plane, "workspaces", workspace)).filter((one) =>
    one.includes(repo),
  );
  expect({ landed: existsSync(landed), hidden: existsSync(hidden), there }).toEqual({
    landed: false,
    hidden: false,
    there: [],
  });
}

/** Words that would name a forge's CLI to the person. */
const NAMES_A_CLI = /\b(gh|glab)\b/;

describe("cloning from a self-hosted GitLab in the real window", function () {
  // On the describe, where WebdriverIO reads it: one clone alone takes over two minutes.
  this.timeout(600_000);

  let first = "";
  let plane = "";
  let forge: FakeGitLab;
  let gitConfigWas = "";
  const clis: string[] = [];

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    forge = await startFakeGitLab(
      join(THE_RUNS_TREE, "fake-gitlab"),
      HOST,
      THE_RUNS_FAKE_FORGE,
      PAGE,
    );
    for (const [name, served] of Object.entries(REPOS)) forge.addRepo(name, served.files, served);
    theForgeAnswers(theListing(forge));
    gitConfigWas = readFileSync(THE_RUNS_GIT_CONFIG, "utf8");
    writeFileSync(THE_RUNS_GIT_CONFIG, gitConfigWas + gitConfigFor(forge));
    clis.push(aCliThatNotes("gh"), aCliThatNotes("glab"));
    plane = aProjectOfItsOwn(4090, (root) => theProjectKnowsTheForge(root, forge));
    await openFromTheStrip(plane);
    await theProjectsFoldersAreAccepted();
  });

  // What the window looked like when a case failed: a spec of the real window can't be run anywhere
  // but CI, and a selector's timeout says nothing of what was drawn instead.
  afterEach(async function () {
    if (this.currentTest?.state !== "failed") return;
    const name = `failed-${(this.currentTest.title ?? "").replace(/[^a-z0-9]+/gi, "-").slice(0, 60)}`;
    await pictures(name, {});
    const said = await browser.execute(() => document.body.innerText);
    writeFileSync(join(WHERE_SHOTS_GO, `${name}.txt`), said);
  });

  // Each step whatever became of the last: one app process serves the whole run, and a fake forge
  // left named would take every later spec's git and sign-in.
  after(async () => {
    const failed: unknown[] = [];
    const step = async (act: () => unknown) => {
      try {
        await act();
      } catch (err: unknown) {
        failed.push(err);
      }
    };
    rmSync(THE_RUNS_CLONE_STALL, { force: true });
    for (const cli of clis) rmSync(cli, { force: true });
    if (gitConfigWas !== "") writeFileSync(THE_RUNS_GIT_CONFIG, gitConfigWas);
    await step(() => forge?.stop());
    rmSync(THE_RUNS_FAKE_FORGE, { force: true });
    await step(async () => {
      for (const account of await ask<Account[]>("forge_accounts"))
        await ask("forge_sign_out", { id: account.id });
    });
    noForgeAnswers();
    await step(closeTheProjectsOfItsOwn);
    await step(async () => {
      await browser.waitUntil(
        async () => {
          if ((await inFront()) === first) return true;
          await theTabOf(first);
          return false;
        },
        { timeout: 20_000, interval: 250, timeoutMsg: "the launch's project never came back" },
      );
    });
    if (failed.length > 0) throw failed[0];
  });

  it("with no sign-in, the forge refuses the clone and the row says to sign in to it", async () => {
    await aWorkspaceNaming(plane, "before-signing-in", ["first-repo"]);
    await press(rowOf("first-repo"), "Clone", "starting");
    const trouble = await $(`${rowOf("first-repo")} .trouble`);
    await trouble.waitForExist({ timeout: 60_000, timeoutMsg: "the refused clone said nothing" });
    const said = await trouble.getText();
    expect(said).toContain(`Sign in to ${forge.server}`);
    expect(said).not.toMatch(NAMES_A_CLI);
    nothingOf(plane, "before-signing-in", "first-repo");
    expect(forge.heard().some((one) => one.repo === "first-repo" && !one.signedIn)).toBe(true);
    expect(forge.heard().some((one) => one.signedIn)).toBe(false);
  });

  it("signs in once through the browser flow", async () => {
    // With no account the fallback above is the CLI's, by design: so the stand-ins are what the
    // window finds, and the note would show one signing a clone. From here on none may.
    expect(readFileSync(CLI_NOTE, "utf8")).toContain("git-credential");
    rmSync(CLI_NOTE, { force: true });
    await browser.execute((css: string) => {
      document.querySelector<HTMLElement>(css)?.click();
    }, OPEN_ONE);
    await $(DOORS).waitForExist({ timeout: 20_000, timeoutMsg: "the welcome never came" });
    await press(WELCOME, SIGN_IN_DOOR);
    await untilSays(WELCOME, "Sign in with GitLab");
    await press(WELCOME, "Sign in with GitLab");
    await browser.waitUntil(() => existsSync(PAGE), {
      timeout: 30_000,
      timeoutMsg: "the sign-in never opened the forge's page",
    });
    await forge.playBrowser(readFileSync(PAGE, "utf8"));
    await browser.waitUntil(
      async () => (await ask<Account[]>("forge_accounts")).some((a) => a.host === forge.server),
      { timeout: 30_000, timeoutMsg: "the browser's redirect signed no account in" },
    );
    expect(forge.grants()).toBe(1);
    // Back to the project, from its tab.
    await theTabOf(plane);
    await browser.waitUntil(async () => !(await $(WELCOME).isExisting()), {
      timeout: 20_000,
      timeoutMsg: "the project never came back from the sign-in page",
    });
  });

  it("clones from its row with that sign-in, where it had just been refused", async () => {
    await focusWorkspace("before-signing-in");
    await press(rowOf("first-repo"), "Retry", "starting");
    const { landed, hidden } = foldersOf(plane, "before-signing-in", "first-repo");
    await untilLanded(landed, 90_000, "the retry after signing in");
    aWholeClone(landed);
    expect(existsSync(hidden)).toBe(false);
    const signed = forge.heard().filter((one) => one.repo === "first-repo" && one.signedIn);
    expect(signed.length).toBeGreaterThan(0);
    // GitLab's name for an OAuth or personal token, from the account's helper and no CLI's.
    expect(new Set(signed.map((one) => one.user))).toEqual(new Set(["oauth2"]));
  });

  it("a clone past the old 120 seconds climbs on the chip's notice and its row, and lands", async () => {
    const started = Date.now();
    await aNewWorkspaceCloning("long-clone", ["long-repo"]);
    const { landed } = foldersOf(plane, "long-clone", "long-repo");
    const onCard: number[] = [];
    const onRow: number[] = [];
    let pictured = false;
    await browser.waitUntil(
      async () => {
        const [card, row] = [
          await percentOf(cardMeterOf("long-clone")),
          await percentOf(meterOf("long-repo")),
        ];
        if (card !== null) onCard.push(card);
        if (row !== null) onRow.push(row);
        // Part way, with both on screen, for a person to look at.
        if (!pictured && card !== null && row !== null && card >= 25 && row >= 25) {
          pictured = true;
          await browser.execute((css: string) => {
            document.querySelector(css)?.scrollIntoView({ block: "nearest" });
          }, meterOf("long-repo"));
          await pictures("mid-clone", {
            "chip-notice": cardOf("long-clone"),
            "repos-row": rowOf("long-repo"),
          });
        }
        return existsSync(join(landed, ".git"));
      },
      {
        timeout: 400_000,
        interval: 1_000,
        timeoutMsg: "the long clone never landed",
      },
    );
    aWholeClone(landed);
    expect(Date.now() - started).toBeGreaterThan(120_000);
    await browser.waitUntil(async () => !(await $(rowOf("long-repo")).isExisting()), {
      timeout: 30_000,
      timeoutMsg: "the landed repo stayed a row of the ones not cloned here",
    });
    await untilSays('[data-testid="repos-view"]', "long-repo");
    expect(pictured).toBe(true);
    for (const seen of [onCard, onRow]) {
      expect(seen).toEqual([...seen].sort((a, b) => a - b));
      expect(new Set(seen).size).toBeGreaterThanOrEqual(5);
      expect(seen.at(-1)).toBeGreaterThanOrEqual(70);
    }
  });

  it("several repos cloned together all land, each with the one sign-in", async () => {
    const grants = forge.grants();
    const repos = ["second-repo", "third-repo", "fourth-repo"];
    await aWorkspaceNaming(plane, "several", repos);
    await press('[data-testid="absent"]', "Clone all");
    for (const repo of repos) {
      const { landed } = foldersOf(plane, "several", repo);
      await untilLanded(landed, 120_000, `${repo} of the Clone all`);
      aWholeClone(landed);
      const heard = forge.heard().filter((one) => one.repo === repo && one.signedIn);
      expect(new Set(heard.map((one) => one.user))).toEqual(new Set(["oauth2"]));
    }
    expect(forge.grants()).toBe(grants);
  });

  it("Cancel on the row, mid-clone, leaves no folder and no hidden folder", async () => {
    const repo = "stopped-on-its-row";
    await aWorkspaceNaming(plane, "stopped-row", [repo]);
    await press(rowOf(repo), "Clone", "starting");
    const { hidden } = foldersOf(plane, "stopped-row", repo);
    await browser.waitUntil(async () => ((await percentOf(meterOf(repo))) ?? 0) >= 10, {
      timeout: 60_000,
      timeoutMsg: "the clone never got part way",
    });
    expect(existsSync(hidden)).toBe(true);
    await press(rowOf(repo), `Cancel cloning ${repo}`);
    await browser.waitUntil(() => !existsSync(hidden), {
      timeout: 30_000,
      timeoutMsg: "the hidden folder outlived the Cancel",
    });
    await browser.waitUntil(async () => (await percentOf(meterOf(repo))) === null, {
      timeout: 30_000,
      timeoutMsg: "the row still shows a clone running",
    });
    nothingOf(plane, "stopped-row", repo);
  });

  it("Cancel on the chip's notice, mid-clone, leaves no folder and no hidden folder", async () => {
    const repo = "stopped-on-its-card";
    await aNewWorkspaceCloning("stopped-card", [repo]);
    const { hidden } = foldersOf(plane, "stopped-card", repo);
    await browser.waitUntil(
      async () => ((await percentOf(cardMeterOf("stopped-card"))) ?? 0) >= 10,
      {
        timeout: 60_000,
        timeoutMsg: "the card's meter never got part way",
      },
    );
    expect(existsSync(hidden)).toBe(true);
    await press(cardOf("stopped-card"), "Cancel");
    await browser.waitUntil(() => !existsSync(hidden), {
      timeout: 30_000,
      timeoutMsg: "the hidden folder outlived the Cancel",
    });
    nothingOf(plane, "stopped-card", repo);
  });

  it("a forge gone silent ends with the stall message, on no forge CLI's say", async () => {
    writeFileSync(THE_RUNS_CLONE_STALL, SHORT_STALL);
    try {
      const repo = "gone-silent";
      await aWorkspaceNaming(plane, "silent", [repo]);
      await press(rowOf(repo), "Clone", "starting");
      const trouble = await $(`${rowOf(repo)} .trouble`);
      await trouble.waitForExist({ timeout: 120_000, timeoutMsg: "the silent clone never ended" });
      const said = await trouble.getText();
      expect(said).toContain("stalled");
      expect(said).toContain(forge.server);
      expect(said).not.toMatch(NAMES_A_CLI);
      nothingOf(plane, "silent", repo);
    } finally {
      rmSync(THE_RUNS_CLONE_STALL, { force: true });
    }
  });

  it("no forge CLI signed any clone of that sign-in", () => {
    expect(cliRunsForThisForge()).toEqual([]);
  });

  it("a token, the way a shipped build signs in to this host, clones as well", async () => {
    for (const account of await ask<Account[]>("forge_accounts"))
      await ask("forge_sign_out", { id: account.id });
    forge.revokeEveryToken();
    forge.acceptToken("a-pasted-token");
    await ask("forge_sign_in_with_token", {
      kind: "gitlab",
      host: forge.server,
      token: "a-pasted-token",
    });
    const repo = "token-repo";
    await aWorkspaceNaming(plane, "by-token", [repo]);
    await press(rowOf(repo), "Clone", "starting");
    const { landed } = foldersOf(plane, "by-token", repo);
    await untilLanded(landed, 90_000, "the clone with a pasted token");
    aWholeClone(landed);
    expect(cliRunsForThisForge()).toEqual([]);
  });
});
