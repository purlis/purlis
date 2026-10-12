// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { THE_RUNS_RECORDED_FORGE } from "./harness.js";

/**
 * **The forge a scenario spec signs in to and lists from** (#1818): the seam in place of
 * github.com, which no run reaches.
 *
 * The app built with the `e2e` feature answers the window's native forge calls, and its token
 * sign-in's check, from the file `THE_RUNS_RECORDED_FORGE` (`src-tauri/src/e2eforge.rs`), in
 * the format of the core's `forge_contract` recordings. The exchanges here are built from those
 * very recordings (`crates/purlis-core/tests/forge_contract/`), so the spec and the core's own
 * tests answer the same calls the same way. The file is read for each request: a spec writes it
 * when it starts, and takes it back when it ends.
 */

const recordings = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "crates",
  "purlis-core",
  "tests",
  "forge_contract",
);

/** One recorded request and its answer, as a recording holds it. */
export type Exchange = {
  call: { endpoint: { rest: { method: null; path: string } }; fields: [] };
  reply: { code: number; out: string; err: string; status: number };
};

function recorded(path: string): Exchange[] {
  return (JSON.parse(readFileSync(join(recordings, path), "utf8")) as { exchanges: Exchange[] })
    .exchanges;
}

/** A request for `path` answered with `out`, shaped as the recordings shape theirs. */
function answering(path: string, out: unknown): Exchange {
  const [shape] = recorded("repolist/github/reachable.json");
  return {
    call: { ...shape.call, endpoint: { rest: { method: null, path } } },
    reply: { ...shape.reply, out: JSON.stringify(out) },
  };
}

/** GitHub's check of a pasted token: it is octocat's, with the scopes purlis needs. */
export function aGitHubTokenCheck(): Exchange[] {
  return recorded("accounts/github/token_check.json");
}

/** A repo as GitHub lists it: `full` is `owner/name`, `url` where it is cloned from. */
export type Repo = { full: string; url: string; top: string[] };

/** GitHub's list of what the account reaches, and each repo's top level for the probe. */
export function aGitHubListing(repos: Repo[]): Exchange[] {
  const [listing] = recorded("repolist/github/reachable.json");
  return [
    answering(
      listing.call.endpoint.rest.path,
      repos.map((repo, id) => ({
        id,
        name: repo.full.split("/")[1],
        full_name: repo.full,
        default_branch: "main",
        description: null,
        html_url: repo.url,
        ssh_url: "",
        topics: [],
      })),
    ),
    ...repos.map((repo) =>
      answering(`repos/${repo.full}/git/trees/main`, {
        sha: "e5bd3914e2e596debea16f433f57875b5b90bcd6",
        tree: repo.top.map((path) => ({ path, type: "blob" })),
        truncated: false,
      }),
    ),
  ];
}

/** Has the run's forge answer `exchanges`, each as often as it is asked. */
export function theForgeAnswers(exchanges: Exchange[]): void {
  writeFileSync(
    THE_RUNS_RECORDED_FORGE,
    JSON.stringify({ source: "a scenario spec's recorded forge", exchanges }),
  );
}

/** Takes the run's forge back: every native call is refused again. */
export function noForgeAnswers(): void {
  rmSync(THE_RUNS_RECORDED_FORGE, { force: true });
}
