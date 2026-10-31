// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFile } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { type FakeGitLab, gitConfigFor, OWNER, startFakeGitLab } from "./fakeGitLab.js";
import {
  CLONE_STALL_VARIABLE,
  FAKE_FORGE_VARIABLE,
  THE_RUNS_CLONE_STALL,
  THE_RUNS_FAKE_FORGE,
  THE_RUNS_TREE,
  theRunsEnvironment,
} from "./harness.js";

/**
 * The fake forge, checked here with a real git before the real window relies on it: a forge that
 * let anyone in, or a spelling the app does not read, would make the window's spec prove nothing.
 */

const here = dirname(fileURLToPath(import.meta.url));
const HOST = "gitlab.fake-forge-test.test";

function cloneWith(
  dir: string,
  forge: FakeGitLab,
  repo: string,
  into: string,
  token: string | null,
): Promise<{ ok: boolean; said: string }> {
  const home = join(dir, "git-config");
  const helper = token === null ? "" : `!f() { echo username=oauth2; echo password=${token}; }; f`;
  writeFileSync(home, gitConfigFor(forge));
  return new Promise((resolve) => {
    execFile(
      "git",
      [
        "-c",
        "credential.helper=",
        "-c",
        `credential.helper=${helper}`,
        "clone",
        "--",
        forge.webUrl(repo),
        join(dir, into),
      ],
      {
        env: {
          PATH: process.env.PATH,
          GIT_CONFIG_GLOBAL: home,
          GIT_CONFIG_NOSYSTEM: "1",
          GIT_TERMINAL_PROMPT: "0",
        },
      },
      (err, _out, said) => resolve({ ok: err === null, said }),
    );
  });
}

describe("the fake GitLab", () => {
  let dir = "";
  let forge: FakeGitLab;

  beforeAll(async () => {
    dir = mkdtempSync(join(tmpdir(), "fake-gitlab-"));
    forge = await startFakeGitLab(
      join(dir, "forge"),
      HOST,
      join(dir, "named.json"),
      join(dir, "page"),
    );
    forge.addRepo("plain", 3, { way: "whole" });
    forge.addRepo("slow", 40, { way: "slowly", seconds: 2 });
    forge.acceptToken("the-token");
  });

  afterAll(async () => {
    await forge.stop();
    rmSync(dir, { recursive: true, force: true });
  });

  it("is named by the variables the e2e build reads, in every launcher's environment", () => {
    const rust = (...at: string[]) => readFileSync(join(here, "..", "..", ...at), "utf8");
    expect(rust("app", "src-tauri", "src", "e2eforge.rs")).toContain(
      `const FAKE_FORGE: &str = "${FAKE_FORGE_VARIABLE}";`,
    );
    expect(rust("crates", "purlis-core", "src", "cloning.rs")).toContain(
      `crate::envvar::var("${CLONE_STALL_VARIABLE}")`,
    );
    const env = theRunsEnvironment(join(THE_RUNS_TREE, "plane"));
    expect(env[FAKE_FORGE_VARIABLE]).toBe(THE_RUNS_FAKE_FORGE);
    expect(env[CLONE_STALL_VARIABLE]).toBe(THE_RUNS_CLONE_STALL);
  });

  it("refuses a clone with no credential and with a wrong one", async () => {
    for (const [into, token] of [
      ["none", null],
      ["wrong", "not-the-token"],
    ] as const) {
      const said = await cloneWith(dir, forge, "plain", into, token);
      expect(said.ok).toBe(false);
      expect(said.said).toMatch(/could not read Username|Authentication failed/);
    }
  });

  it("serves a clone over TLS to the token it took, on its own host name", async () => {
    const said = await cloneWith(dir, forge, "plain", "plain", "the-token");
    expect(said).toEqual(expect.objectContaining({ ok: true }));
    expect(existsSync(join(dir, "plain", "file-2.bin"))).toBe(true);
    expect(forge.heard().some((one) => one.repo === "plain" && one.user === "oauth2")).toBe(true);
    expect(forge.webUrl("plain")).toBe(`https://${forge.server}/${OWNER}/plain`);
  });

  it("takes as long as it was told over a slow repo's pack, and still lands it", async () => {
    const started = Date.now();
    const said = await cloneWith(dir, forge, "slow", "slow", "the-token");
    expect(said).toEqual(expect.objectContaining({ ok: true }));
    expect(Date.now() - started).toBeGreaterThanOrEqual(1_900);
    expect(existsSync(join(dir, "slow", "file-39.bin"))).toBe(true);
  });

  it("grants a token for a PKCE code only with its verifier", async () => {
    const callback: Server = createServer((_, response) => {
      response.writeHead(200);
      response.end("ok");
    });
    await new Promise<void>((resolve) => callback.listen(0, "127.0.0.1", resolve));
    const redirect = `http://127.0.0.1:${(callback.address() as AddressInfo).port}/purlis/signin`;
    const verifier = randomBytes(32).toString("base64url");
    const page = new URL(`https://${forge.server}/oauth/authorize`);
    page.searchParams.set("redirect_uri", redirect);
    page.searchParams.set("response_type", "code");
    page.searchParams.set("code_challenge_method", "S256");
    page.searchParams.set("state", "s");
    page.searchParams.set(
      "code_challenge",
      createHash("sha256").update(verifier).digest("base64url"),
    );
    const codes: string[] = [];
    callback.removeAllListeners("request");
    callback.on("request", (request, response) => {
      codes.push(new URL(request.url ?? "/", redirect).searchParams.get("code") ?? "");
      response.writeHead(200);
      response.end("ok");
    });
    await forge.playBrowser(page.href);
    const post = (code_verifier: string) =>
      fetch(`${forge.oauth}/oauth/token`, {
        method: "POST",
        body: new URLSearchParams({
          grant_type: "authorization_code",
          code: codes[0],
          redirect_uri: redirect,
          code_verifier,
        }),
      });
    expect((await post("not-the-verifier")).status).toBe(400);
    expect(forge.grants()).toBe(0);
    const granted = (await (await post(verifier)).json()) as { access_token: string };
    expect(granted.access_token.length).toBeGreaterThan(20);
    expect(forge.grants()).toBe(1);
    callback.close();
  });
});
