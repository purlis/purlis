// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The theme schema against the last release's (#2524). Run with `node --test tools/`.
//
// The script runs against a stand-in `git` put first on PATH: the tags `origin` has and the
// file each tag holds are the test's, so nothing is fetched. The test command it runs is a
// stand-in too, which records what it was handed and exits as the test says.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "theme-schema-since-release.sh");
const FILE = "docs/schema/theme.v2.schema.json";

// `tags`: each tag `origin` has, with the schema it holds (or null for none). `exit`: what the
// test command exits with.
function run({ tags = {}, exit = 0, env = {} } = {}) {
  const root = mkdtempSync(join(tmpdir(), "theme-schema-since-release-"));
  const bin = join(root, "bin");
  const files = join(root, "files");
  mkdirSync(bin);
  mkdirSync(files);
  const log = join(root, "git.log");
  const ran = join(root, "ran");
  for (const [tag, schema] of Object.entries(tags)) {
    if (schema !== null) writeFileSync(join(files, tag), schema);
  }
  const write = (name, body) => {
    writeFileSync(join(bin, name), `#!/bin/sh\n${body}\n`);
    chmodSync(join(bin, name), 0o755);
  };
  const listing = Object.keys(tags)
    .map((tag) => `0000000000000000000000000000000000000000\trefs/tags/${tag}`)
    .join("\n");
  write(
    "git",
    `echo "git $*" >> "${log}"
case "$1" in
  ls-remote) printf '%s\\n' "${listing}" ;;
  rev-parse) exit 1 ;;
  fetch) exit 0 ;;
  cat-file) tag="\${3%%:*}"; [ -e "${files}/$tag" ] ;;
  show) tag="\${2%%:*}"; cat "${files}/$tag" ;;
  *) exit 9 ;;
esac`,
  );
  write("the-test", `{ echo "$PURLIS_THEME_SCHEMA_RELEASED"; cat "$PURLIS_THEME_SCHEMA_RELEASED"; } > "${ran}"; exit ${exit}`);
  const result = spawnSync("bash", [script, "the-test"], {
    encoding: "utf8",
    env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, RUNNER_TEMP: root, ...env },
  });
  return {
    status: result.status,
    stdout: result.stdout,
    stderr: result.stderr,
    git: existsSync(log) ? readFileSync(log, "utf8") : "",
    ran: existsSync(ran) ? readFileSync(ran, "utf8").split("\n") : null,
  };
}

test("with no release yet, it says so and passes without running the test", () => {
  const out = run({ tags: { dev: "{}" } });
  assert.equal(out.status, 0, out.stderr);
  assert.match(out.stdout, /::notice title=Theme schema since the last release::No release yet/);
  assert.equal(out.ran, null);
});

test("before the first release with a v2 schema, it says so and passes", () => {
  const out = run({ tags: { "v0.4.0": null, "v0.4.1": null } });
  assert.equal(out.status, 0, out.stderr);
  assert.match(out.stdout, /v0\.4\.1 has no docs\/schema\/theme\.v2\.schema\.json/);
  assert.equal(out.ran, null);
});

test("the last release is the highest vX.Y.Z tag, and its schema is what the test reads", () => {
  const out = run({
    tags: {
      "v0.9.1": '{"old":true}',
      "v0.10.0": '{"last":true}',
      "v0.11.0-rc.1": '{"pre":true}',
      dev: '{"dev":true}',
    },
  });
  assert.equal(out.status, 0, out.stderr);
  assert.match(out.git, /git fetch --quiet --no-tags --depth=1 origin refs\/tags\/v0\.10\.0:refs\/tags\/v0\.10\.0/);
  assert.match(out.git, new RegExp(`git show v0\\.10\\.0:${FILE}`));
  assert.ok(out.ran[0].endsWith(".json"), out.ran[0]);
  assert.equal(out.ran[1], '{"last":true}');
});

test("a narrowing the test finds fails the step", () => {
  const out = run({ tags: { "v0.5.0": '{"type":"object"}' }, exit: 101 });
  assert.equal(out.status, 101);
  assert.equal(out.ran[1], '{"type":"object"}');
});

test("PURLIS_RELEASE_TAG names the release to compare with", () => {
  const out = run({
    tags: { "v0.5.0": '{"chosen":true}', "v0.6.0": '{"last":true}' },
    env: { PURLIS_RELEASE_TAG: "v0.5.0" },
  });
  assert.equal(out.status, 0, out.stderr);
  assert.doesNotMatch(out.git, /ls-remote/);
  assert.equal(out.ran[1], '{"chosen":true}');
});

test("with no test command, it refuses", () => {
  const result = spawnSync("bash", [script], { encoding: "utf8" });
  assert.equal(result.status, 2);
});
