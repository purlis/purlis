// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The runner image contract (docs/runner-image.md, #2043). Run with `node --test tools/`.
//
// Two halves are checked here without Docker: what an image's config says (`configProblems`,
// fed the `Config` of `docker image inspect` as JSON), and what the image holds, by running
// the script that runs inside an image against a folder standing in for its root, with
// stand-ins for `id` and the tools first on PATH. Starting `purlis serve` in a real image is
// the third half, and runs in `runner-image.yml` only.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { configProblems, parseArguments } from "./runner-image-contract.mjs";

const inside = join(dirname(fileURLToPath(import.meta.url)), "runner-image-contract.sh");

// The config of an image that keeps the contract, as `docker image inspect` reports it.
function good() {
  return {
    Os: "linux",
    Config: {
      User: "10001:10001",
      Env: ["PATH=/usr/local/bin:/usr/bin:/bin", "HOME=/home/purlis"],
      Entrypoint: ["/usr/bin/tini", "--", "/usr/local/bin/purlis"],
      Cmd: ["serve"],
    },
  };
}

function with_(change) {
  const image = good();
  change(image.Config, image);
  return image;
}

test("an image that keeps the contract has no config problem", () => {
  assert.deepEqual(configProblems(good()), []);
});

test("an image whose default user is root, or named rather than numbered, is refused", () => {
  for (const user of ["", "0", "0:0", "root", "purlis"]) {
    const problems = configProblems(with_((c) => (c.User = user)));
    assert.equal(problems.length, 1, `${JSON.stringify(user)}: ${problems}`);
    assert.match(problems[0], /user/);
  }
  assert.deepEqual(configProblems(with_((c) => (c.User = "10001"))), []);
});

test("an image with no absolute HOME is refused", () => {
  const problems = configProblems(with_((c) => (c.Env = ["PATH=/usr/bin:/bin"])));
  assert.equal(problems.length, 1, `${problems}`);
  assert.match(problems[0], /HOME/);
  assert.match(configProblems(with_((c) => (c.Env = ["HOME=home"])))[0], /HOME/);
});

test("an image that does not start purlis serve under an init is refused", () => {
  const bare = configProblems(with_((c) => ((c.Entrypoint = ["/usr/local/bin/purlis"]), (c.Cmd = ["serve"]))));
  assert.equal(bare.length, 1, `${bare}`);
  assert.match(bare[0], /init/);

  const elsewhere = configProblems(with_((c) => (c.Cmd = ["mcp"])));
  assert.equal(elsewhere.length, 1, `${elsewhere}`);
  assert.match(elsewhere[0], /purlis serve/);

  const shell = configProblems(with_((c) => ((c.Entrypoint = null), (c.Cmd = ["/bin/sh"]))));
  assert.match(shell.join("\n"), /purlis serve/);

  // The other common inits, and the whole command written in Cmd alone.
  for (const init of [["/usr/bin/dumb-init", "--"], ["/usr/bin/catatonit", "--"], ["/sbin/tini"]]) {
    const image = with_((c) => ((c.Entrypoint = null), (c.Cmd = [...init, "/usr/local/bin/purlis", "serve"])));
    assert.deepEqual(configProblems(image), [], init.join(" "));
  }
});

test("an image for another operating system is refused", () => {
  assert.match(configProblems(with_((_, image) => (image.Os = "windows")))[0], /linux/);
});

// A folder standing for an image's root, a bin/ of stand-ins first on PATH, and a HOME, made to
// keep the contract; `change` breaks one part of it.
function machine(change = {}) {
  const root = mkdtempSync(join(tmpdir(), "runner-image-contract-"));
  const bin = join(root, "stand-ins");
  const home = join(root, "home", "purlis");
  mkdirSync(bin);
  mkdirSync(home, { recursive: true, mode: 0o700 });
  mkdirSync(join(root, "usr", "local", "bin"), { recursive: true });
  mkdirSync(join(root, "etc", "ssl", "certs"), { recursive: true });
  mkdirSync(join(root, "tmp"), { mode: 0o1777 });
  const program = (path, body) => {
    writeFileSync(path, `#!/bin/sh\n${body}\n`);
    chmodSync(path, 0o755);
  };
  program(join(bin, "id"), `[ "$1" = -u ] && echo ${change.uid ?? 10001}`);
  for (const tool of ["git", "bash"]) {
    if (change.missing !== tool) program(join(bin, tool), "exit 0");
  }
  if (change.harness) program(join(bin, change.harness), "exit 0");
  if (change.purlis !== "missing") {
    program(
      join(root, "usr", "local", "bin", "purlis"),
      change.purlis === "broken" ? "exit 1" : '[ "$1" = --version ] && echo "purlis 0.9.0 (dev-release profile)"',
    );
  }
  if (!change.noCertificates) writeFileSync(join(root, "etc", "ssl", "certs", "ca-certificates.crt"), "");
  if (!change.noPlugin) {
    const manifest = join(root, "usr", "local", "lib", "purlis", "plugin", ".claude-plugin");
    mkdirSync(manifest, { recursive: true });
    writeFileSync(join(manifest, "plugin.json"), '{"name":"purlis"}');
  }
  if (change.readOnlyTmp) chmodSync(join(root, "tmp"), 0o555);
  const env = {
    PATH: bin,
    HOME: change.home ?? home,
    CONTRACT_ROOT: root,
  };
  if (change.readOnlyHome) chmodSync(home, 0o500);
  return env;
}

function check(env) {
  const run = spawnSync("/bin/sh", ["-s"], { input: readFileSync(inside, "utf8"), env, encoding: "utf8" });
  return { status: run.status, problems: run.stdout.split("\n").filter(Boolean), stderr: run.stderr };
}

test("an image that holds what the contract asks passes the inside check", () => {
  const { status, problems, stderr } = check(machine());
  assert.deepEqual(problems, [], stderr);
  assert.equal(status, 0);
});

test("a program the image is to carry has to answer, and one nobody named is not asked for", () => {
  const env = machine();
  assert.deepEqual(check(env).problems, [], "the contract itself asks for neither");
  const carrying = { ...env, CONTRACT_CARRIES: "gh glab" };
  const { status, problems } = check(carrying);
  assert.equal(status, 1);
  assert.equal(problems.length, 2, `${problems}`);
  assert.match(problems[0], /carry gh/);
  assert.match(problems[1], /carry glab/);

  for (const tool of ["gh", "glab"]) {
    const answers = join(env.PATH, tool);
    writeFileSync(answers, "#!/bin/sh\n[ \"$1\" = --version ]\n");
    chmodSync(answers, 0o755);
  }
  assert.deepEqual(check(carrying).problems, []);
  assert.deepEqual(check({ ...carrying, CONTRACT_CARRIES: "" }).problems, []);
});

test("the command line takes an image, and the programs it carries before it", () => {
  assert.deepEqual(parseArguments(["image"]), { image: "image", carries: [] });
  assert.deepEqual(parseArguments(["--carries", "gh,glab", "image"]), { image: "image", carries: ["gh", "glab"] });
  for (const wrong of [[], ["a", "b"], ["--carries"], ["--carries", "gh"], ["--carries", "gh;id", "image"], ["--other", "image"]]) {
    assert.equal(parseArguments(wrong), null, wrong.join(" "));
  }
});

test("the inside check names each thing an image lacks, and fails", () => {
  const cases = [
    [{ uid: 0 }, /root/],
    [{ purlis: "missing" }, /\/usr\/local\/bin\/purlis/],
    [{ purlis: "broken" }, /--version/],
    [{ missing: "git" }, /git/],
    [{ missing: "bash" }, /bash/],
    [{ noCertificates: true }, /certificate/],
    [{ noPlugin: true }, /plugin/],
    [{ home: "/nonexistent/purlis" }, /HOME/],
    [{ readOnlyHome: true }, /HOME/],
    [{ readOnlyTmp: true }, /\/tmp/],
    [{ harness: "claude" }, /harness/],
    [{ harness: "codex" }, /harness/],
    [{ harness: "opencode" }, /harness/],
  ];
  for (const [change, named] of cases) {
    const { status, problems } = check(machine(change));
    assert.notEqual(status, 0, JSON.stringify(change));
    assert.equal(problems.length, 1, `${JSON.stringify(change)}: ${problems.join(" | ")}`);
    assert.match(problems[0], named, JSON.stringify(change));
  }
});
