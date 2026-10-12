// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { THE_RUNS_FORGE } from "./harness.js";

/**
 * **A forge to create a project on, for a scenario spec** (#1819): the seam a spec uses in place
 * of github.com, where no spec may make a repo.
 *
 * The app built with the `e2e` feature reads [`THE_RUNS_FORGE`] (`scenario` in
 * `src-tauri/src/createproject.rs`): `owners.json` answers the owner picker and `create.json` the
 * creation, both recordings in the forge contract's shape (`purlis-core/tests/forge_contract/`),
 * and the first commit is pushed to the bare repository `remote.git` there. While
 * `hold-before-push` is there, the push waits, so a spec can press Cancel with the repo made and
 * the push not yet run. Every other build reads nothing.
 */

const OWNERS = {
  login: "octocat",
  id: 583231,
  orgs: [{ login: "acme", id: 9919 }],
};

function get(path: string, out: unknown) {
  return {
    call: { endpoint: { rest: { method: null, path } }, fields: [] },
    reply: { code: 0, out: JSON.stringify(out), err: "" },
  };
}

function write(method: string, path: string, fields: unknown[], out: unknown) {
  return {
    call: { endpoint: { rest: { method, path } }, fields },
    reply: { code: 0, out: out === "" ? "" : JSON.stringify(out), err: "" },
  };
}

function recording(exchanges: unknown[]): string {
  return JSON.stringify({ source: "the e2e stand-in forge (#1819)", exchanges }, null, 2);
}

/** The owners the stand-in's account lists: itself, then one organisation. */
function ownerExchanges() {
  return [
    get("user", { login: OWNERS.login, id: OWNERS.id, name: "The Octocat" }),
    get("user/orgs?per_page=100&page=1", OWNERS.orgs),
  ];
}

/** The repo the stand-in makes for `name` under the account, private. */
function made(name: string) {
  return {
    id: 1296269,
    name,
    full_name: `${OWNERS.login}/${name}`,
    private: true,
    html_url: `https://github.com/${OWNERS.login}/${name}`,
    clone_url: `https://github.com/${OWNERS.login}/${name}.git`,
    default_branch: "main",
  };
}

/** git for the stand-in, never the runner's own config. */
function git(args: string[]): void {
  execFileSync("git", args, {
    env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" },
    stdio: ["ignore", "ignore", "ignore"],
  });
}

/** The repository the first commit lands in. */
export const THE_REMOTE = join(THE_RUNS_FORGE, "remote.git");

const HOLD = join(THE_RUNS_FORGE, "hold-before-push");

/**
 * Sets the stand-in up for a project named `name` made under the account: its owners, its
 * creation (and the delete a Cancel sends), and an empty repository to push to.
 */
export function aForgeThatCreates(name: string): void {
  mkdirSync(THE_RUNS_FORGE, { recursive: true });
  writeFileSync(join(THE_RUNS_FORGE, "owners.json"), recording(ownerExchanges()));
  const repo = made(name);
  writeFileSync(
    join(THE_RUNS_FORGE, "create.json"),
    recording([
      ...ownerExchanges(),
      write(
        "POST",
        "user/repos",
        [
          { text: ["name", name] },
          { typed: ["private", "true"] },
          { typed: ["auto_init", "false"] },
        ],
        repo,
      ),
      get(`repos/${OWNERS.login}/${name}`, repo),
      write("DELETE", `repos/${OWNERS.login}/${name}`, [], ""),
    ]),
  );
  rmSync(THE_REMOTE, { recursive: true, force: true });
  git(["init", "-q", "--bare", "-b", "main", THE_REMOTE]);
}

/** Has the next creation's push wait until [`letThePushGo`], or a Cancel. */
export function holdThePush(): void {
  mkdirSync(THE_RUNS_FORGE, { recursive: true });
  writeFileSync(HOLD, "held");
}

/** Lets a held push go, and takes the hold back for the next spec. */
export function letThePushGo(): void {
  rmSync(HOLD, { force: true });
}

/** Whether the push is still held. */
export function thePushIsHeld(): boolean {
  return existsSync(HOLD);
}

/** The commits the stand-in's repository holds on `main`, newest first, by their subjects. */
export function pushedCommits(): string[] {
  try {
    return execFileSync("git", ["--git-dir", THE_REMOTE, "log", "--format=%s", "main"], {
      env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" },
      encoding: "utf8",
    })
      .split("\n")
      .filter(Boolean);
  } catch {
    return [];
  }
}
