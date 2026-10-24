// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, realpathSync, renameSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { press, typeInto, untilSays } from "../acting.js";
import { anAnswerIsWaiting, noAnswerWaits, theNextFolderPickAnswers } from "../dialogs.js";
import { copyFixturePlane, THE_RUNS_HOME, THE_RUNS_TREE } from "../harness.js";
import { closeProject } from "../opening.js";
import {
  aGitHubListing,
  aGitHubTokenCheck,
  noForgeAnswers,
  theForgeAnswers,
} from "../recordedForge.js";
import { ask, PROJECTS } from "../switching.js";

/**
 * **Get a project from your repos, in the built window** (#1818, spec #1810's first seam).
 *
 * A person with no forge account opens Get a project from your forge: each forge says to sign in
 * or paste a URL. They sign in with a token right there, and the list comes: their repos, the
 * purlis project on top and marked. They search it by name and by owner, pick the project, and
 * it is cloned and opened through the real trust question.
 *
 * **The forge is recorded** (`recordedForge.ts`): the `e2e` build answers the window's native
 * forge calls and its token check from the run's recording, in the core's `forge_contract`
 * format, so the sign-in, the listing and the manifest probes go through the shipped account
 * store, hand-over, resolver and budget. The project's repo is a bare copy of the `daily`
 * fixture reached by a `file://` address that only the `e2e` build clones (#1815).
 *
 * **Nothing of it is left behind**: the account is signed out, the recording taken back, the
 * project closed and forgotten, and the launch's project brought back to the front. It starts no
 * chat.
 */

const OPEN_ONE = `${PROJECTS} button[aria-label="Open a project…"]`;
const DOORS = 'ul[aria-label="Ways to start"]';
const WELCOME = "section.welcome";
const URL_BOX = '[data-setting="get-project-url"] input';
const SEARCH = '[data-setting="your-repos-search"] input';
const REPOS = 'ul[aria-label="Your repos"]';
const FORGES = 'ul[aria-label="Your forges"]';
const DIALOG = '[role="dialog"]';
const GOES_INTO = `${WELCOME} .get-project-folder code`;
const DOOR = "Get a project from your forge";

/** The token the spec pastes: the recorded forge takes any, and it is shaped like none. */
const TOKEN = "pasted-for-the-recorded-forge";

/** The project's name, which the clone's folder is named after. */
const NAME = "team-project";

/** git for the stand-in forge: never the machine's own config, so never its signer. */
function git(dir: string, args: string[]): void {
  execFileSync("git", ["-c", "commit.gpgsign=false", ...args], {
    cwd: dir,
    env: {
      ...process.env,
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_AUTHOR_NAME: "fixture",
      GIT_AUTHOR_EMAIL: "fixture@example.invalid",
      GIT_COMMITTER_NAME: "fixture",
      GIT_COMMITTER_EMAIL: "fixture@example.invalid",
    },
    stdio: ["ignore", "ignore", "ignore"],
  });
}

/** The project's repo on the stand-in forge: a bare copy of the `daily` fixture project. */
const bare = (() => {
  const copied = copyFixturePlane();
  const work = join(dirname(copied), `${NAME}-listed-work`);
  renameSync(copied, work);
  git(work, ["init", "-q", "-b", "main"]);
  git(work, ["add", "-A"]);
  git(work, ["commit", "-q", "-m", "the project"]);
  const at = join(THE_RUNS_TREE, "listed-forge", "acme", `${NAME}.git`);
  mkdirSync(dirname(at), { recursive: true });
  git(dirname(at), ["clone", "-q", "--bare", work, at]);
  return realpathSync(at);
})();

/** Where the person picks to keep their projects in this run. */
const projects = (() => {
  const at = join(THE_RUNS_TREE, "listed-projects");
  mkdirSync(at, { recursive: true });
  return realpathSync(at);
})();

const landed = join(projects, NAME);

type MachineProject = { path: string; name: string; gone: string | null };
type Account = { id: string; login: string };

/** What `dir` holds, for a message: its names, or that it is not there. */
function holds(dir: string): string {
  return existsSync(dir) ? `[${readdirSync(dir).join(", ")}]` : "nothing (not there)";
}

/** The repos' buttons in the list, by what a screen reader says of each. */
async function listed(): Promise<string[]> {
  return browser.execute(
    (css: string) =>
      [...document.querySelectorAll<HTMLElement>(`${css} button`)].map(
        (b) => b.getAttribute("aria-label") ?? "",
      ),
    REPOS,
  );
}

/** Draws the welcome from the strip's `+`, and goes through the Get door to its page. */
async function toTheGetPage(): Promise<void> {
  if (await $(URL_BOX).isExisting()) await press(WELCOME, "Back");
  const pressed = await browser.execute((css: string) => {
    const button = document.querySelector<HTMLElement>(css);
    button?.click();
    return button !== null;
  }, OPEN_ONE);
  if (!pressed) throw new Error("the Projects strip offers no Open a project…");
  await $(DOORS).waitForExist({ timeout: 20_000, timeoutMsg: "the welcome never came" });
  await press(DOORS, DOOR);
  await $(URL_BOX).waitForExist({ timeout: 20_000, timeoutMsg: "the Get page drew no URL box" });
}

async function untilOpen(open: boolean, why: string): Promise<void> {
  await browser.waitUntil(
    async () => (await ask<string[]>("open_planes")).includes(landed) === open,
    { timeout: 60_000, timeoutMsg: why },
  );
}

/** Brings the launch's project to the front from its tab, which puts the welcome away. */
async function backTo(first: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      if (!(await $(DOORS).isExisting()) && !(await $(URL_BOX).isExisting())) return true;
      await browser.execute(
        (strip: string, path: string) =>
          [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)]
            .find((tab) => tab.getAttribute("data-directory") === path)
            ?.click(),
        PROJECTS,
        first,
      );
      return false;
    },
    { timeout: 20_000, interval: 250, timeoutMsg: "the launch's project never came back" },
  );
}

describe("getting a project from your repos", function () {
  this.timeout(240_000);

  let first = "";

  before(async () => {
    first = (await ask<string[]>("open_planes"))[0];
    theForgeAnswers([
      ...aGitHubTokenCheck(),
      ...aGitHubListing([
        { full: "acme/api", url: "https://github.com/acme/api", top: ["Cargo.toml", "src"] },
        { full: `acme/${NAME}`, url: `file://${bare}`, top: ["charter.toml", "workspaces"] },
        { full: "octocat/dots", url: "https://github.com/octocat/dots", top: ["README.md"] },
      ]),
    ]);
  });

  after(async () => {
    noAnswerWaits();
    for (const account of await ask<Account[]>("forge_accounts"))
      await ask("forge_sign_out", { id: account.id });
    noForgeAnswers();
    for (const plane of (await ask<string[]>("open_planes")).filter((one) => one !== first))
      await ask("close_plane", { plane });
    for (const one of (await ask<{ projects: MachineProject[] }>("this_machine")).projects)
      if (one.path === landed) await ask("forget_project", { path: one.path });
    await backTo(first);
  });

  it("says, with no account, to sign in or paste a URL on each forge", async () => {
    await toTheGetPage();

    await untilSays(FORGES, "Not signed in to github.com");
    await untilSays(FORGES, "Not signed in to gitlab.com");
    await untilSays(FORGES, "paste a project's URL above");
    expect(await $(REPOS).isExisting()).toBe(false);
  });

  it("signs in with a token right there, then lists the repos with the project on top, marked", async () => {
    await press(FORGES, "Sign in…");
    await $(DIALOG).waitForExist({ timeout: 20_000, timeoutMsg: "the token sign-in never opened" });
    await typeInto(`${DIALOG} input[type="password"]`, TOKEN);
    await press(DIALOG, "Sign in");

    await $(REPOS).waitForExist({ timeout: 30_000, timeoutMsg: "no repo list after signing in" });
    expect(await listed()).toEqual([
      `Get acme/${NAME}, a purlis project, from github.com as octocat`,
      "Get acme/api from github.com as octocat",
      "Get octocat/dots from github.com as octocat",
    ]);
    await untilSays(REPOS, "purlis project");
    await untilSays(FORGES, "3 repos on github.com as octocat.");
    await untilSays(FORGES, "Not signed in to gitlab.com");
  });

  it("searches the list by name and by owner", async () => {
    await typeInto(SEARCH, "dots");
    await browser.waitUntil(async () => (await listed()).length === 1, {
      timeoutMsg: "searching a name kept other repos",
    });
    expect(await listed()).toEqual(["Get octocat/dots from github.com as octocat"]);

    await typeInto(SEARCH, "acme");
    await browser.waitUntil(async () => (await listed()).length === 2, {
      timeoutMsg: "searching an owner did not list its two repos",
    });

    await typeInto(SEARCH, "nothing-like-it");
    await untilSays(WELCOME, "No repo matches");
    await typeInto(SEARCH, "");
  });

  it("clones the project picked and opens it through the trust question", async () => {
    theNextFolderPickAnswers(projects);
    await press(WELCOME, "Choose a folder…");
    await browser.waitUntil(async () => !anAnswerIsWaiting(), {
      timeoutMsg: "the folder pick was never asked",
    });
    // The place picked, as the page says it: the clone goes where the page says, and a repo
    // picked before the page took the pick would go to `~/Projects` instead.
    await untilSays(GOES_INTO, projects);

    await press(REPOS, `Get acme/${NAME}`, "starting");
    // The app's own signal that the clone landed (#1858): the window asks the trust question
    // only once the clone has answered, and says a failed clone in its alert. Waited on rather
    // than the folder appearing on disk, which a loaded runner can show late or early. The
    // question's own button: the Welcome's open-by-path form has an "Open project" of its own.
    // The question is named, not the first dialog: one found before it is not it (#2126).
    const gate = await $('[role="dialog"][aria-labelledby="approve-plane"]').$(
      "button=Open project",
    );
    const refused = await $(`${WELCOME} .trouble[role="alert"]`);
    await browser.waitUntil(async () => (await gate.isExisting()) || (await refused.isExisting()), {
      timeout: 120_000,
      interval: 500,
      timeoutMsg: `the clone never answered; it ${
        existsSync(landed) ? "landed" : "did not land"
      } in ${landed}`,
    });
    if (await refused.isExisting())
      throw new Error(`the clone was refused: ${await refused.getText()}`);
    // Where the question asks about, and where the clone went, named when it is not `landed`.
    const asked = await browser.execute(
      (css: string) =>
        [...document.querySelectorAll(css)].map((one) => one.textContent ?? "").join(" | "),
      `${DIALOG} .where code`,
    );
    if (!existsSync(join(landed, ".git")))
      throw new Error(
        `the trust question asks about ${JSON.stringify(asked)}, but ${landed} holds no .git; ` +
          `${projects} holds ${holds(projects)}, ${landed} holds ${holds(landed)}, and ` +
          `~/Projects holds ${holds(join(THE_RUNS_HOME, "Projects"))}`,
      );
    await gate.click();
    await untilOpen(true, "the picked project never opened");

    await closeProject(`${PROJECTS} [role="tab"][aria-label="${basename(landed)}"]`);
    await untilOpen(false, "the picked project stayed open");
  });
});
