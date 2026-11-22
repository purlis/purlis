// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The wait a build job makes for its commit's `ci` before it signs or uploads (R2, #7381). The
// step's text is read out of `.github/workflows/release.yml` and run against a fake `gh`, so each
// answer a ci run can come to is seen to end the step the way the workflow needs: green passes,
// and red, cancelled, unfinished, unreadable and never listed do not. Only the lines that name
// the runner's own `PATH` and `gh` are pointed at the fake. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const release = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const WAIT = "      - name: Wait for this commit's ci on main, before anything is signed or uploaded\n";
const RUNNER_PATH = "PATH=/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin";

/** The `run:` text of every step of that name, de-indented. */
function waitScripts() {
  const out = [];
  let at = release.indexOf(WAIT);
  while (at !== -1) {
    const rest = release.slice(at + WAIT.length);
    const start = rest.indexOf("        run: |\n") + "        run: |\n".length;
    const lines = [];
    for (const line of rest.slice(start).split("\n")) {
      if (line.trim() !== "" && !line.startsWith("          ")) break;
      lines.push(line.replace(/^ {10}/, ""));
    }
    out.push(lines.join("\n").trimEnd());
    at = release.indexOf(WAIT, at + 1);
  }
  return out;
}

/** Runs the script with a `gh` that answers `listed` to `run list` and `views` (one per call) to `run view`. */
function run({ dev = "true", listed = "4242", views = ["completed success"], extraEnv = {} } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "release-wait-"));
  const log = join(dir, "calls.log");
  writeFileSync(join(dir, "count"), "0");
  writeFileSync(
    join(dir, "gh"),
    `#!/bin/bash
echo "$* host=$GH_HOST" >> "${log}"
case "$1 $2" in
  "run list") printf '%s' "${listed}" ;;
  "run watch") exit 1 ;;
  "run view")
    n=$(cat "${dir}/count"); echo $((n + 1)) > "${dir}/count"
    set -- ${views.map((v) => `"${v}"`).join(" ")}
    [ "$n" -lt "$#" ] || n=$(($# - 1))
    shift "$n"
    [ "$1" = error ] && { echo "HTTP 502" >&2; exit 1; }
    printf '%s' "$1" ;;
esac
`,
  );
  writeFileSync(join(dir, "sleep"), "#!/bin/bash\nexit 0\n");
  chmodSync(join(dir, "gh"), 0o755);
  chmodSync(join(dir, "sleep"), 0o755);
  const script = waitScripts()[0]
    .replace(RUNNER_PATH, `PATH=${dir}:/usr/bin:/bin`)
    .replace("/usr/bin/gh |", `${dir}/gh |`);
  assert.ok(script.includes(`${dir}/gh |`), "the gh allow-list line moved");
  const result = spawnSync("/bin/bash", ["--noprofile", "--norc", "-eo", "pipefail", "-c", script], {
    encoding: "utf8",
    env: { ...extraEnv, DEV: dev, GITHUB_REPOSITORY: "purlis/purlis", GITHUB_SHA: "abc123", GH_TOKEN: "x", PATH: "/usr/bin:/bin" },
  });
  let calls = "";
  try {
    calls = readFileSync(log, "utf8");
  } catch {
    // no call was made
  }
  return { code: result.status, out: result.stdout + result.stderr, calls };
}

test("both builds carry the same wait, word for word", () => {
  const scripts = waitScripts();
  assert.equal(scripts.length, 2, "build and runner-binary-aarch64 each have the wait");
  assert.equal(scripts[0], scripts[1]);
});

test("a green ci passes", () => {
  const r = run();
  assert.equal(r.code, 0, r.out);
  assert.match(r.calls, /run list .*--commit abc123 --event push --branch main/);
});

test("a red ci ends the step, so nothing after it signs or uploads", () => {
  const r = run({ views: ["completed failure"] });
  assert.equal(r.code, 1);
  assert.match(r.out, /::error::.*ended failure/);
});

test("a cancelled ci ends the step", () => {
  const r = run({ views: ["completed cancelled"] });
  assert.equal(r.code, 1);
  assert.match(r.out, /ended cancelled/);
});

test("a ci with no conclusion yet ends the step", () => {
  const r = run({ views: ["in_progress none"] });
  assert.equal(r.code, 1);
  assert.match(r.out, /is in_progress after the wait/);
});

test("a ci run that is never listed ends the step", () => {
  const r = run({ listed: "" });
  assert.equal(r.code, 1);
  assert.match(r.out, /no ci run of abc123/);
});

test("an API error on the read is tried again, and a green answer after it passes", () => {
  const r = run({ views: ["error", "error", "completed success"] });
  assert.equal(r.code, 0, r.out);
  assert.equal(r.calls.split("\n").filter((l) => l.startsWith("run view")).length, 3);
});

test("three API errors in a row end the step, and a red answer after an error still ends it", () => {
  assert.equal(run({ views: ["error", "error", "error"] }).code, 1);
  assert.equal(run({ views: ["error", "completed failure"] }).code, 1);
});

test("anything but a push to main passes at once and asks nothing", () => {
  const r = run({ dev: "false", views: ["completed failure"] });
  assert.equal(r.code, 0, r.out);
  assert.equal(r.calls, "");
});

test("the gate asks the forge with the same call and answers false for a red or cancelled ci", () => {
  const gate = release.slice(release.indexOf("\n  gate:\n"), release.indexOf("\n  plan:\n"));
  const ask = (text) => text.slice(text.indexOf("ask() {"), text.indexOf("\n          }\n", text.indexOf("ask() {")));
  const wait = waitScripts()[0].replace(/^/gm, "          ");
  assert.equal(ask(gate).replaceAll('gh run view', '"$gh" run view'), ask(wait));
  assert.match(gate, /conclusion" = success \]; then\n\s+echo "green=true"/);
});

test("what the compile left in the environment never reaches gh", () => {
  const r = run({ extraEnv: { GH_HOST: "evil.example", LD_PRELOAD: "/tmp/x.so" } });
  assert.equal(r.code, 0, r.out);
  assert.doesNotMatch(r.calls, /evil\.example/);
  assert.match(r.calls, /host=\n/);
});
