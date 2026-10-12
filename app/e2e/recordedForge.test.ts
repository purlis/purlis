// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import {
  RECORDED_FORGE_VARIABLE,
  THE_RUNS_RECORDED_FORGE,
  THE_RUNS_TREE,
  theRunsEnvironment,
} from "./harness.js";
import {
  aGitHubListing,
  aGitHubTokenCheck,
  noForgeAnswers,
  theForgeAnswers,
} from "./recordedForge.js";

/**
 * The scenario's half of the recorded forge (#1818), checked here because the spec that uses it
 * first runs in CI: a spelling the app does not read, or a request the core does not send,
 * would leave the spec's list empty for a reason nobody could see.
 */

const here = dirname(fileURLToPath(import.meta.url));
const e2eforge = () => readFileSync(join(here, "..", "src-tauri", "src", "e2eforge.rs"), "utf8");

afterEach(noForgeAnswers);

describe("the run's recorded forge", () => {
  it("is named by the variable the e2e build reads, in every launcher's environment", () => {
    expect(e2eforge()).toContain(`const RECORDED_FORGE: &str = "${RECORDED_FORGE_VARIABLE}";`);
    expect(theRunsEnvironment(join(THE_RUNS_TREE, "plane"))[RECORDED_FORGE_VARIABLE]).toBe(
      THE_RUNS_RECORDED_FORGE,
    );
  });

  it("answers the requests the core sends: the token check, the listing and each probe", () => {
    theForgeAnswers([
      ...aGitHubTokenCheck(),
      ...aGitHubListing([
        { full: "acme/plan", url: "https://github.com/acme/plan", top: ["purlis.toml"] },
      ]),
    ]);
    const written = JSON.parse(readFileSync(THE_RUNS_RECORDED_FORGE, "utf8")) as {
      exchanges: { call: { endpoint: { rest: { path: string } } }; reply: { out: string } }[];
    };
    expect(written.exchanges.map((e) => e.call.endpoint.rest.path)).toEqual([
      "user",
      "user/repos?affiliation=owner,collaborator,organization_member&per_page=100&page=1",
      "repos/acme/plan/git/trees/main",
    ]);
    const [listed] = JSON.parse(written.exchanges[1].reply.out) as { full_name: string }[];
    expect(listed.full_name).toBe("acme/plan");
  });
});
