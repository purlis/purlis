// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `tools/macos-dev-sign-run.sh` does: which identity it signs with, that it does nothing on
// CI, and that the binary always runs. `security` and `codesign` are stand-ins on PATH that log
// their calls; nothing here signs anything. Every spawned process has a timeout.
// Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const SCRIPT = join(import.meta.dirname, "macos-dev-sign-run.sh");

const APPLE_1 = "1111111111111111111111111111111111111111";
const APPLE_2 = "2222222222222222222222222222222222222222";
const LISTING = [
  `  1) AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA "Developer ID Application: Purlis, Inc (TEAM)"`,
  `  2) ${APPLE_1} "Apple Development: Someone (ABCDE12345)"`,
  `  3) ${APPLE_2} "Apple Development: Someone Else (FGHIJ67890)"`,
  `  4) 3333333333333333333333333333333333333333 "purlis dev"`,
  `     4 valid identities found`,
].join("\n");
const ONLY_SELF_SIGNED = [
  `  1) 3333333333333333333333333333333333333333 "purlis dev"`,
  `     1 valid identities found`,
].join("\n");
const NONE = "     0 valid identities found";

function program(dir, name, body) {
  const path = join(dir, name);
  writeFileSync(path, `#!/bin/sh\n${body}\n`);
  chmodSync(path, 0o755);
  return path;
}

/**
 * The script run on a fresh machine: `security` lists `listing`, `codesign` logs to `calls` and
 * says `signed` for `-dv` (nothing, as an unsigned file, when empty), and the binary prints its
 * arguments. `env` is all the script sees besides PATH and HOME.
 */
function run({
  listing = NONE,
  signed = "",
  codesignExit = 0,
  env = {},
  binName = "purlis",
  args = ["a", "b"],
} = {}) {
  const dir = mkdtempSync(join(tmpdir(), "macos-dev-sign-test-"));
  const fakes = join(dir, "fakes");
  const calls = join(dir, "calls");
  mkdirSync(fakes);
  writeFileSync(calls, "");
  writeFileSync(join(dir, "listing"), listing);
  writeFileSync(join(dir, "signed"), signed);
  program(
    fakes,
    "security",
    `echo "security $*" >> "${calls}"; cat "${join(dir, "listing")}"`,
  );
  program(
    fakes,
    "codesign",
    `echo "codesign $*" >> "${calls}"
case "$*" in
  -dv*) [ -s "${join(dir, "signed")}" ] || { echo "code object is not signed at all" >&2; exit 1; }
        cat "${join(dir, "signed")}" >&2; exit 0 ;;
esac
exit ${codesignExit}`,
  );
  const bin = program(dir, binName, `echo "ran $*"; exit 7`);
  const once = () =>
    spawnSync(SCRIPT, [bin, ...args], {
      env: { PATH: `${fakes}:/usr/bin:/bin`, HOME: join(dir, "home"), ...env },
      encoding: "utf8",
      timeout: 30_000,
      killSignal: "SIGKILL",
    });
  const result = once();
  const log = readFileSync(calls, "utf8").trim().split("\n").filter(Boolean);
  return { ...result, calls: log, bin, dir, rerun: once };
}

function done(r) {
  rmSync(r.dir, { recursive: true, force: true });
}

const signs = (r) => r.calls.filter((c) => c.startsWith("codesign --force"));

test("on CI it execs the binary and starts neither security nor codesign, with no hint", () => {
  const r = run({ listing: LISTING, env: { CI: "true" } });
  assert.equal(r.status, 7);
  assert.match(r.stdout, /^ran a b$/m);
  assert.deepEqual(r.calls, []);
  assert.equal(r.stderr, "");
  done(r);
});

test("CI=false is not CI", () => {
  const r = run({ listing: LISTING, env: { CI: "false" } });
  assert.equal(signs(r).length, 1);
  done(r);
});

test("PURLIS_DEV_SIGN_IDENTITY wins and security is not asked", () => {
  const r = run({
    listing: LISTING,
    env: { PURLIS_DEV_SIGN_IDENTITY: "My Cert" },
  });
  assert.equal(r.status, 7);
  assert.deepEqual(signs(r), [
    `codesign --force --sign My Cert --identifier dev.purlis.local.purlis ${r.bin}`,
  ]);
  assert.ok(!r.calls.some((c) => c.startsWith("security")));
  done(r);
});

test("without an override, the first Apple Development identity signs, not the second or other kinds", () => {
  const r = run({ listing: LISTING });
  assert.deepEqual(signs(r), [
    `codesign --force --sign ${APPLE_1} --identifier dev.purlis.local.purlis ${r.bin}`,
  ]);
  assert.equal(r.stderr, "");
  done(r);
});

test("with no Apple Development identity, a self-signed purlis dev signs", () => {
  const r = run({ listing: ONLY_SELF_SIGNED });
  assert.deepEqual(signs(r), [
    `codesign --force --sign 3333333333333333333333333333333333333333 --identifier dev.purlis.local.purlis ${r.bin}`,
  ]);
  done(r);
});

test("with no identity it runs unsigned, calls no codesign and hints once a day", () => {
  const r = run({ listing: NONE });
  assert.equal(r.status, 7);
  assert.match(r.stdout, /ran a b/);
  assert.deepEqual(signs(r), []);
  assert.match(r.stderr, /Local macOS signing/);
  assert.equal(r.stderr.trim().split("\n").length, 1);
  const again = r.rerun();
  assert.equal(again.status, 7);
  assert.equal(again.stderr, "");
  done(r);
});

test("the identifier is the binary's name with a test binary's hash dropped, never the release app's", () => {
  const r = run({ listing: LISTING, binName: "it-0123456789abcdef" });
  assert.match(signs(r)[0], /--identifier dev\.purlis\.local\.it /);
  assert.ok(!signs(r)[0].includes("dev.purlis.app"));
  done(r);
});

test("a binary already signed with this identity and identifier is not signed again", () => {
  const info = [
    "Executable=/x/purlis",
    "Identifier=dev.purlis.local.purlis",
    "Format=Mach-O thin (arm64)",
    "Authority=Apple Development: Someone (ABCDE12345)",
    "Authority=Apple Worldwide Developer Relations Certification Authority",
    "",
  ].join("\n");
  const r = run({ listing: LISTING, signed: info });
  assert.equal(r.status, 7);
  assert.deepEqual(signs(r), []);
  done(r);
});

test("an ad-hoc signature, another identifier or another authority is signed over", () => {
  const adhoc = "Identifier=dev.purlis.local.purlis\nSignature=adhoc\n";
  const other =
    "Identifier=dev.purlis.local.other\nAuthority=Apple Development: Someone (ABCDE12345)\n";
  const elsewhere =
    "Identifier=dev.purlis.local.purlis\nAuthority=Someone Else\n";
  for (const signed of [adhoc, other, elsewhere]) {
    const r = run({ listing: LISTING, signed });
    assert.equal(signs(r).length, 1, signed);
    done(r);
  }
});

test("a codesign that fails still runs the binary, with its exit status and one hint", () => {
  const r = run({ listing: LISTING, codesignExit: 1 });
  assert.equal(r.status, 7);
  assert.match(r.stdout, /ran a b/);
  assert.match(r.stderr, /runs unsigned/);
  assert.ok(!existsSync(`${r.bin}.signlock`));
  done(r);
});

test("it passes the program's arguments through untouched", () => {
  const r = run({ listing: NONE, args: ["--flag", "two words", "--exact"] });
  assert.match(r.stdout, /^ran --flag two words --exact$/m);
  done(r);
});
