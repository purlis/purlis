// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The wait a build job makes for its commit's `ci` before it signs or uploads (R2, #7381). The
// step's text is read out of `.github/workflows/release.yml` and run against a fake `gh`, so each
// answer a ci run can come to is seen to end the step the way the workflow needs: green passes,
// and red or cancelled ends it green with `green=false`, a notice and a summary (so every later step
// of the job skips and the dev build stops without a red job), while an unfinished, unreadable or
// never-listed ci ends it non-zero: those are faults, not a red commit. Only the lines that name
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

/**
 * Runs the script with a `gh` that answers `listed` to `run list` and `views` (one per call) to `run view`.
 * The wait loop watches again until `completed` or a two-hour deadline, and the fake `sleep` returns at
 * once, so a case that expects the step to fail on an unfinished or unreadable ci sets `expired`: the
 * deadline is already past, as after the two hours, and the first read that is not `completed` ends it.
 * Every run is cut at 20 s, so a loop that never ends fails the test instead of hanging a runner.
 */
function run({ dev = "true", listed = "4242", views = ["completed success"], extraEnv = {}, expired = false } = {}) {
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
  const original = waitScripts()[0];
  assert.ok(original.includes("deadline=$((SECONDS + 7200))"), "the wait's deadline moved");
  const script = original
    .replace(RUNNER_PATH, `PATH=${dir}:/usr/bin:/bin`)
    .replace("/usr/bin/gh |", `${dir}/gh |`)
    .replace("deadline=$((SECONDS + 7200))", expired ? "deadline=$((SECONDS - 1))" : "deadline=$((SECONDS + 7200))");
  assert.ok(script.includes(`${dir}/gh |`), "the gh allow-list line moved");
  const output = join(dir, "output");
  const summary = join(dir, "summary");
  writeFileSync(output, "");
  writeFileSync(summary, "");
  const result = spawnSync("/bin/bash", ["--noprofile", "--norc", "-eo", "pipefail", "-c", script], {
    encoding: "utf8",
    timeout: 20_000,
    env: { ...extraEnv, DEV: dev, GITHUB_OUTPUT: output, GITHUB_STEP_SUMMARY: summary, GITHUB_REPOSITORY: "purlis/purlis", GITHUB_SHA: "abc123", GH_TOKEN: "x", PATH: "/usr/bin:/bin" },
  });
  let calls = "";
  try {
    calls = readFileSync(log, "utf8");
  } catch {
    // no call was made
  }
  assert.ok(!result.error, `the step did not end within 20 s: ${result.error}`);
  return {
    code: result.status,
    out: result.stdout + result.stderr,
    calls,
    output: readFileSync(output, "utf8"),
    summary: readFileSync(summary, "utf8"),
  };
}

test("both builds carry the same wait, word for word", () => {
  const scripts = waitScripts();
  assert.equal(scripts.length, 2, "build and runner-binary-aarch64 each have the wait");
  assert.equal(scripts[0], scripts[1]);
});

test("a green ci passes and answers green=true", () => {
  const r = run();
  assert.equal(r.code, 0, r.out);
  assert.equal(r.output, "green=true\n");
  assert.match(r.calls, /run list .*--commit abc123 --event push --branch main/);
});

test("a red ci ends the step green with green=false, a notice and a summary, so the later steps skip", () => {
  const r = run({ views: ["completed failure"] });
  assert.equal(r.code, 0, r.out);
  assert.equal(r.output, "green=false\n");
  assert.match(
    r.out,
    /::notice::this commit's main ci ended failure, so this dev build stops here: nothing signed, uploaded or published/,
  );
  assert.doesNotMatch(r.out, /::error::/);
  assert.match(r.summary, /ended failure/);
  assert.match(r.summary, /nothing signed, uploaded or published/);
});

test("a cancelled or timed-out ci ends the same way", () => {
  for (const conclusion of ["cancelled", "timed_out"]) {
    const r = run({ views: [`completed ${conclusion}`] });
    assert.equal(r.code, 0, r.out);
    assert.equal(r.output, "green=false\n");
    assert.match(r.out, new RegExp(`ended ${conclusion}, so this dev build stops here`));
  }
});

test("a conclusion the step cannot read as green or red ends it non-zero and answers nothing", () => {
  for (const conclusion of ["skipped", "neutral", "action_required", "startup_failure", "none", "stale"]) {
    const r = run({ views: [`completed ${conclusion}`] });
    assert.equal(r.code, 1, `${conclusion}: ${r.out}`);
    assert.match(r.out, /::error::.*ended .*cannot read as green or red/);
    assert.equal(r.output, "", conclusion);
  }
});

test("a ci with no conclusion yet ends the step", () => {
  const r = run({ views: ["in_progress none"], expired: true });
  assert.equal(r.code, 1);
  assert.match(r.out, /is in_progress after the wait/);
  assert.equal(r.output, "");
});

test("a ci still running when a watch ends early is watched again until it completes", () => {
  const r = run({ views: ["in_progress none", "queued none", "error", "error", "error", "completed success"] });
  assert.equal(r.code, 0, r.out);
  assert.equal(r.output, "green=true\n");
  assert.ok(r.calls.split("\n").filter((l) => l.startsWith("run watch")).length >= 3, r.calls);
});

test("a ci run that is never listed ends the step", () => {
  const r = run({ listed: "" });
  assert.equal(r.code, 1);
  assert.match(r.out, /no ci run of abc123/);
  assert.equal(r.output, "");
});

test("an API error on the read is tried again, and a green answer after it passes", () => {
  const r = run({ views: ["error", "error", "completed success"] });
  assert.equal(r.code, 0, r.out);
  assert.equal(r.calls.split("\n").filter((l) => l.startsWith("run view")).length, 3);
});

test("three API errors in a row end the step non-zero, and a red answer after an error is still a red commit", () => {
  const faulty = run({ views: ["error", "error", "error"], expired: true });
  assert.equal(faulty.code, 1);
  assert.equal(faulty.output, "");
  const red = run({ views: ["error", "completed failure"] });
  assert.equal(red.code, 0, red.out);
  assert.equal(red.output, "green=false\n");
});

test("anything but a push to main passes at once, answers green=true and asks nothing", () => {
  const r = run({ dev: "false", views: ["completed failure"] });
  assert.equal(r.code, 0, r.out);
  assert.equal(r.output, "green=true\n");
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
