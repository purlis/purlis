// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// install.sh, run by `sh` against stand-ins for every program it calls (uname, curl, ditto,
// xattr, apt, dpkg, sudo, gh), on a PATH that holds nothing else, so the real ones on the
// machine running the test are never reached and nothing is downloaded or installed.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(new URL("../install.sh", import.meta.url));

// The real programs install.sh may use besides the stand-ins.
const REAL = ["sh", "mktemp", "mkdir", "rm", "mv", "chmod", "id", "cat"];

const STANDINS = {
  uname: `[ "$1" = -s ] && echo "$STUB_OS" || echo "$STUB_ARCH"`,
  curl: `out=""; url=""
while [ $# -gt 0 ]; do case "$1" in -o) out="$2"; shift 2 ;; *) url="$1"; shift ;; esac; done
echo "curl $url" >> "$LOG"
[ -n "\${STUB_CURL_FAIL:-}" ] && exit 22
echo bytes > "$out"`,
  ditto: `echo "ditto $*" >> "$LOG"; mkdir -p "$4/purlis.app/Contents"`,
  xattr: `echo "xattr $*" >> "$LOG"`,
  sudo: `echo "sudo $*" >> "$LOG"; "$@"`,
  apt: `echo "apt $*" >> "$LOG"`,
  dpkg: `exit 0`,
  gh: `echo "gh $*" >> "$LOG"
[ "$1" = auth ] && exit 0
exit "\${STUB_GH_VERIFY:-0}"`,
};

function run({ os, arch, env = {}, without = [] }) {
  const root = mkdtempSync(join(tmpdir(), "install-sh-"));
  const bin = join(root, "bin");
  spawnSync("mkdir", ["-p", bin]);
  for (const name of REAL) {
    const found = spawnSync("sh", ["-c", `command -v ${name}`], { encoding: "utf8" }).stdout.trim();
    symlinkSync(found, join(bin, name));
  }
  for (const [name, body] of Object.entries(STANDINS)) {
    if (without.includes(name)) continue;
    writeFileSync(join(bin, name), `#!/bin/sh\n${body}\n`);
    chmodSync(join(bin, name), 0o755);
  }
  const log = join(root, "log");
  writeFileSync(log, "");
  const apps = join(root, "Applications");
  const result = spawnSync(join(bin, "sh"), [SCRIPT], {
    encoding: "utf8",
    env: {
      PATH: bin,
      HOME: root,
      TMPDIR: root,
      LOG: log,
      STUB_OS: os,
      STUB_ARCH: arch,
      PURLIS_APPLICATIONS: apps,
      ...env,
    },
  });
  return { ...result, apps, calls: readFileSync(log, "utf8").trim().split("\n").filter(Boolean) };
}

test("on Apple silicon it installs the stable app into the applications folder and clears quarantine", () => {
  const r = run({ os: "Darwin", arch: "arm64", without: ["gh"] });
  assert.equal(r.status, 0, r.stderr);
  assert.ok(
    r.calls.includes("curl https://github.com/purlis/purlis/releases/latest/download/purlis-macos-arm64.app.zip"),
  );
  assert.ok(existsSync(join(r.apps, "purlis.app", "Contents")));
  assert.ok(r.calls.some((c) => c.startsWith("xattr -dr com.apple.quarantine") && c.endsWith("purlis.app")));
});

test("PURLIS_CHANNEL=dev downloads from the dev prerelease", () => {
  const r = run({ os: "Darwin", arch: "arm64", without: ["gh"], env: { PURLIS_CHANNEL: "dev" } });
  assert.equal(r.status, 0, r.stderr);
  assert.ok(r.calls.includes("curl https://github.com/purlis/purlis/releases/download/dev/purlis-macos-arm64.app.zip"));
});

test("on Linux with apt it installs the .deb through sudo apt", () => {
  const r = run({ os: "Linux", arch: "x86_64", without: ["gh"] });
  assert.equal(r.status, 0, r.stderr);
  assert.ok(r.calls.includes("curl https://github.com/purlis/purlis/releases/latest/download/purlis-linux-x86_64.deb"));
  const apt = r.calls.find((c) => c.startsWith("apt install -y "));
  assert.ok(apt?.endsWith("/purlis-linux-x86_64.deb"), r.calls.join("\n"));
});

test("on Linux without apt it installs the AppImage, executable, into the applications folder", () => {
  const r = run({ os: "Linux", arch: "x86_64", without: ["gh", "apt", "dpkg"] });
  assert.equal(r.status, 0, r.stderr);
  const appimage = join(r.apps, "purlis.AppImage");
  assert.ok(existsSync(appimage));
  assert.ok(statSync(appimage).mode & 0o100);
});

test("a platform with no build is refused by name, before anything is downloaded", () => {
  const r = run({ os: "Darwin", arch: "x86_64" });
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /no build for macOS on x86_64/);
  assert.deepEqual(r.calls, []);
});

test("an unknown channel is refused, naming the two there are", () => {
  const r = run({ os: "Darwin", arch: "arm64", env: { PURLIS_CHANNEL: "nightly" } });
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /Use stable or dev/);
});

test("a failed download installs nothing", () => {
  const r = run({ os: "Darwin", arch: "arm64", without: ["gh"], env: { STUB_CURL_FAIL: "1" } });
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /could not download/);
  assert.equal(existsSync(join(r.apps, "purlis.app")), false);
});

test("with gh signed in, a download that fails its provenance check is not installed", () => {
  const r = run({ os: "Darwin", arch: "arm64", env: { STUB_GH_VERIFY: "1" } });
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /failed its build provenance check/);
  assert.ok(r.calls.some((c) => c.startsWith("gh attestation verify")));
  assert.equal(existsSync(join(r.apps, "purlis.app")), false);
});

test("with gh signed in, a download that passes its provenance check is installed", () => {
  const r = run({ os: "Darwin", arch: "arm64" });
  assert.equal(r.status, 0, r.stderr);
  assert.ok(r.calls.some((c) => c.includes("--signer-workflow purlis/purlis/.github/workflows/release.yml")));
  assert.ok(existsSync(join(r.apps, "purlis.app")));
});
