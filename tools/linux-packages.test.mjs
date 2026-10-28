// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The Linux jobs' package install, cached and bounded (#1477). Run with `node --test tools/`.
//
// The script runs against stand-ins for `apt-get`, `timeout` and `dpkg-query` put first on
// PATH, and with no `sudo`, so what it would ask of the mirror is read from a log and never
// sent anywhere.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "linux-packages.sh");

// A machine to run the script on: a bin/ of stand-ins, an archives/ apt downloads into, a log
// of every apt-get call, and the files GitHub hands a step for its outputs and its summary.
// `osRelease` is the distribution's /etc/os-release, or null for none.
function machine({
  failUpdates = 0,
  failInstalls = 0,
  failOffline = false,
  manyInstalled = false,
  osRelease = "ID=ubuntu\nVERSION_CODENAME=noble\n",
  offlineHas = "socat bubblewrap",
} = {}) {
  const root = mkdtempSync(join(tmpdir(), "linux-packages-"));
  const release = join(root, "os-release");
  if (osRelease !== null) writeFileSync(release, osRelease);
  const bin = join(root, "bin");
  const archives = join(root, "archives");
  mkdirSync(bin);
  mkdirSync(archives);
  const log = join(root, "apt.log");
  const write = (name, body) => {
    writeFileSync(join(bin, name), `#!/bin/sh\n${body}\n`);
    chmodSync(join(bin, name), 0o755);
  };
  // `timeout -k 10 SECONDS COMMAND...`: the limit is logged, and the command runs.
  write("timeout", `echo "timeout $3" >> "${log}"; shift 3; exec "$@"`);
  // A count per kind of call, so the Nth update or install can be made to time out (124).
  write(
    "apt-get",
    `echo "apt-get $*" >> "${log}"
# A sources list named on the command line is kept, so a test can read what it asked for.
for a in "$@"; do case "$a" in Dir::Etc::SourceList=*) cp "\${a#*=}" "${root}/sources-seen" ;; esac; done
kind=install
case "$*" in *update*) kind=update ;; *--no-download*) kind=offline ;; esac
n=$(cat "${root}/$kind.n" 2>/dev/null || echo 0); n=$((n + 1)); echo $n > "${root}/$kind.n"
case $kind in
  update) [ $n -le ${failUpdates} ] && exit 124 ;;
  offline) [ ${failOffline ? 1 : 0} = 1 ] && { echo "E: Unable to correct problems, you have held broken packages." >&2; exit 100; }
    # As apt does: a .deb named on the command line counts as downloaded only when it lies in
    # the archives directory. Anywhere else apt queues it as a download, which --no-download
    # drops, and the install fails (#1477: every warm run on 2026-10-10 failed this way).
    # A package named by name installs offline only when the image has it or its file is in the
    # archives: offlineHas stands for both.
    for a in "$@"; do
      case "$a" in
        *.deb)
          [ "$(dirname "$a")" = "${archives}" ] && [ -e "$a" ] || {
            echo "E: Unable to fetch some archives, maybe run apt-get update or try with --fix-missing?" >&2; exit 100; } ;;
        -* | install | *=* | *:*) ;;
        *)
          case " ${offlineHas} " in *" $a "*) ;; *)
            echo "E: Unable to fetch some archives, maybe run apt-get update or try with --fix-missing?" >&2; exit 100 ;;
          esac ;;
      esac
    done ;;
  install) [ $n -le ${failInstalls} ] && exit 124
    touch "${archives}/libfoo_1.0-1_amd64.deb" "${archives}/libbar_1%3a2.0_amd64.deb" "${archives}/stale_0.1_amd64.deb" ;;
esac
exit 0`,
  );
  // What is installed: the two the install brought, and not the stale one.
  // With `manyInstalled`, thousands more after them, as on a runner image: more than a pipe holds.
  write(
    "dpkg-query",
    `printf 'libfoo_1.0-1_amd64\\nlibbar_1:2.0_amd64\\n'${
      manyInstalled ? `; i=0; while [ $i -lt 4000 ]; do echo "filler-package-$i"_1.2.3-4ubuntu0.1_amd64; i=$((i + 1)); done` : ""
    }`,
  );
  const output = join(root, "output");
  const summary = join(root, "summary");
  writeFileSync(output, "");
  writeFileSync(summary, "");
  const run = (...args) => {
    const r = spawnSync("bash", [script, ...args], {
      encoding: "utf8",
      env: {
        PATH: `${bin}:${process.env.PATH}`,
        HOME: root,
        GITHUB_OUTPUT: output,
        GITHUB_STEP_SUMMARY: summary,
        ImageOS: "ubuntu24",
        ImageVersion: "20261005.1",
        LINUX_PACKAGES_SUDO: "",
        LINUX_PACKAGES_ARCHIVES: archives,
        LINUX_PACKAGES_PAUSE: "0",
        LINUX_PACKAGES_OS_RELEASE: release,
        RUNNER_TEMP: root,
      },
    });
    return {
      ...r,
      apt: existsSync(log) ? readFileSync(log, "utf8").trim().split("\n") : [],
      output: readFileSync(output, "utf8"),
      summary: readFileSync(summary, "utf8"),
    };
  };
  return { root, archives, run };
}

// The step output file is appended to, as GitHub's is, so the last key= line is this run's.
const outputOf = (r, name) =>
  [...r.output.matchAll(new RegExp(`^${name}=(.*)$`, "gm"))].at(-1)?.[1];
const keyOf = (r) => outputOf(r, "key");

test("the cache key is the package list, whatever its order, and the runner image", () => {
  const { run } = machine();
  const a = keyOf(run("key", "socat", "bubblewrap"));
  const b = keyOf(run("key", "bubblewrap", "socat", "socat"));
  const c = keyOf(run("key", "bubblewrap"));
  assert.ok(a, "no key= line in the step's output");
  assert.equal(a, b);
  assert.notEqual(a, c);
  assert.match(a, /^linux-packages-v2-ubuntu24-[\w-]+-[0-9a-f]{16}-20261005\.1$/);
});

test("another runner image restores an older image's cache of the same list, and of no other", () => {
  const { run } = machine();
  const r = run("key", "socat", "bubblewrap");
  const key = keyOf(r);
  const prefix = outputOf(r, "prefix");
  // The key is the prefix and the image, so a cache this list saved on any image shares it.
  assert.equal(key, `${prefix}20261005.1`);
  assert.ok(!keyOf(run("key", "bubblewrap")).startsWith(prefix));
  // And the same cache under the name it was saved by before the image moved to the end.
  const hash = prefix.match(/([0-9a-f]{16})-$/)[1];
  assert.match(
    outputOf(r, "previous"),
    new RegExp(`^linux-packages-v1-ubuntu24-20261005\\.1-[\\w-]+-${hash}$`),
  );
});

test("--tauri names the same list as writing Tauri's packages out", () => {
  const { run } = machine();
  const tauri = keyOf(run("key", "--tauri", "socat"));
  const written = keyOf(
    run(
      "key",
      "socat",
      "libwebkit2gtk-4.1-dev",
      "libxdo-dev",
      "libssl-dev",
      "libayatana-appindicator3-dev",
      "librsvg2-dev",
    ),
  );
  assert.equal(tauri, written);
});

test("a first install from the mirror is bounded, keeps what it downloaded, and says how long it took", () => {
  const { root, run } = machine();
  const cache = join(root, "cache");
  const r = run("install", "--cache", cache, "socat");
  assert.equal(r.status, 0, r.stderr);
  assert.ok(r.apt.some((l) => l.startsWith("apt-get") && l.includes("update")));
  assert.ok(r.apt.some((l) => l.startsWith("apt-get") && l.includes("install") && l.endsWith("socat")));
  assert.ok(r.apt.filter((l) => l.startsWith("timeout")).length >= 2, "every apt-get runs under a timeout");
  assert.match(r.output, /^save=true$/m);
  assert.deepEqual(readdirSync(cache).sort(), ["libbar_1%3a2.0_amd64.deb", "libfoo_1.0-1_amd64.deb"]);
  assert.match(r.summary, /from the mirror/);
  assert.match(r.summary, /\d+ s/);
});

test("the files are kept when the image lists thousands of installed packages", () => {
  const { root, run } = machine({ manyInstalled: true });
  const cache = join(root, "cache");
  const r = run("install", "--cache", cache, "socat");
  assert.equal(r.status, 0, r.stderr);
  assert.deepEqual(readdirSync(cache).sort(), ["libbar_1%3a2.0_amd64.deb", "libfoo_1.0-1_amd64.deb"]);
});

test("with the cache restored, the install never asks the mirror", () => {
  const { root, archives, run } = machine();
  const cache = join(root, "cache");
  mkdirSync(cache);
  writeFileSync(join(cache, "libfoo_1.0-1_amd64.deb"), "");
  const r = run("install", "--cache", cache, "socat");
  assert.equal(r.status, 0, r.stderr);
  const apt = r.apt.filter((l) => l.startsWith("apt-get"));
  assert.equal(apt.length, 1, apt.join("\n"));
  assert.match(apt[0], /--no-download/);
  assert.ok(apt[0].includes(join(archives, "libfoo_1.0-1_amd64.deb")), apt[0]);
  assert.doesNotMatch(r.stdout + r.stderr, /::warning/);
  assert.doesNotMatch(r.output, /^save=true$/m);
  assert.match(r.summary, /from the cache/);
});

test("a cache refilled from the mirror holds the files now installed, and not the ones that did not fit", () => {
  const { root, run } = machine({ failOffline: true });
  const cache = join(root, "cache");
  mkdirSync(cache);
  writeFileSync(join(cache, "libfoo_0.9-1_amd64.deb"), "");
  const r = run("install", "--cache", cache, "socat");
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.output, /^save=true$/m);
  assert.deepEqual(readdirSync(cache).sort(), [
    "libbar_1%3a2.0_amd64.deb",
    "libfoo_1.0-1_amd64.deb",
  ]);
});

test("where the packages came from is a notice on the run, read without its log", () => {
  const { root, run } = machine();
  const cache = join(root, "cache");
  mkdirSync(cache);
  writeFileSync(join(cache, "libfoo_1.0-1_amd64.deb"), "");
  assert.match(
    run("install", "--cache", cache, "socat").stdout,
    /^::notice title=Linux packages::1 packages installed in \d+ s from the cache, without asking the mirror\.$/m,
  );
  assert.match(
    machine().run("install", "socat").stdout,
    /^::notice title=Linux packages::1 packages installed in \d+ s from the mirror, on attempt 1 of 2\./m,
  );
});

test("a restored cache that lacks a package asked for goes to the mirror", () => {
  // Restored from an older image's key (#3547), the files are what that image lacked: a package it
  // had preinstalled and this image has not is in none of them, so the offline install must name
  // the packages too, fail on the missing one, and go on to the mirror.
  const { root, run } = machine({ offlineHas: "" });
  const cache = join(root, "cache");
  mkdirSync(cache);
  writeFileSync(join(cache, "libfoo_1.0-1_amd64.deb"), "");
  const r = run("install", "--cache", cache, "socat");
  assert.equal(r.status, 0, r.stderr);
  const offline = r.apt.find((l) => l.includes("--no-download"));
  assert.ok(offline && / socat( |$)/.test(offline), `the offline install did not name socat: ${offline}`);
  assert.ok(r.apt.some((l) => l.includes("update")), "never went to the mirror");
});

test("a cache the image no longer fits falls back to the mirror", () => {
  const { root, run } = machine({ failOffline: true });
  const cache = join(root, "cache");
  mkdirSync(cache);
  writeFileSync(join(cache, "libfoo_1.0-1_amd64.deb"), "");
  const r = run("install", "--cache", cache, "socat");
  assert.equal(r.status, 0, r.stderr);
  assert.ok(r.apt.some((l) => l.includes("update")));
  assert.match(r.stdout + r.stderr, /::warning[^\n]*apt-get said: E: Unable to correct problems/);
  assert.match(r.summary, /from the mirror/);
});

test("a mirror that hangs once is tried once more", () => {
  const { run } = machine({ failUpdates: 1 });
  const r = run("install", "socat");
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.apt.filter((l) => l.startsWith("apt-get") && l.includes("update")).length, 2);
  assert.match(r.summary, /attempt 2 of 2/);
});

test("the second attempt asks another mirror, the distribution's own, for the update and the install", () => {
  const { root, run } = machine({ failUpdates: 1 });
  const r = run("install", "socat");
  assert.equal(r.status, 0, r.stderr);
  const [first, ...rest] = r.apt.filter((l) => l.startsWith("apt-get"));
  assert.doesNotMatch(first, /SourceList/);
  assert.equal(rest.length, 2, rest.join("\n"));
  // That list alone: the image's own sources, its mirror among them, are left out.
  const lists = rest.map((l) => l.match(/-o Dir::Etc::SourceList=(\S+)/)?.[1]);
  assert.ok(lists[0], rest[0]);
  assert.equal(lists[0], lists[1]);
  for (const l of rest) assert.match(l, /-o Dir::Etc::SourceParts=\S+/);
  const seen = readFileSync(join(root, "sources-seen"), "utf8");
  for (const suite of ["noble", "noble-updates", "noble-security"])
    assert.match(
      seen,
      new RegExp(
        `^deb \\[signed-by=/usr/share/keyrings/ubuntu-archive-keyring\\.gpg\\] http://\\S+\\.ubuntu\\.com/\\S+ ${suite} main restricted universe multiverse$`,
        "m",
      ),
    );
  assert.match(r.stdout, /::warning[^\n]*trying once more from http:\/\/\S+\.ubuntu\.com/);
  assert.match(r.summary, /attempt 2 of 2, from http:\/\/\S+\.ubuntu\.com/);
});

test("off Ubuntu there is no other mirror to ask, so the second attempt asks the same sources", () => {
  for (const osRelease of ["ID=fedora\nVERSION_CODENAME=\n", null]) {
    const { run } = machine({ failUpdates: 1, osRelease });
    const r = run("install", "socat");
    assert.equal(r.status, 0, r.stderr);
    assert.ok(r.apt.every((l) => !l.includes("SourceList")));
  }
});

test("a mirror that hangs twice fails the step with a message saying so", () => {
  const { run } = machine({ failInstalls: 2 });
  const r = run("install", "socat");
  assert.notEqual(r.status, 0);
  assert.equal(r.apt.filter((l) => l.startsWith("apt-get") && l.includes("update")).length, 2);
  assert.match(r.stdout + r.stderr, /::error[^\n]*mirror/);
  assert.match(r.summary, /failed/);
});

test("without --cache nothing is kept for a cache and nothing is read from one", () => {
  const { run } = machine();
  const r = run("install", "--tauri");
  assert.equal(r.status, 0, r.stderr);
  assert.doesNotMatch(r.output, /save=/);
  assert.ok(r.apt.every((l) => !l.includes("--no-download")));
  assert.ok(r.apt.some((l) => l.includes("libwebkit2gtk-4.1-dev")));
});

test("with --refresh the cached files go to apt's archives but the install asks the mirror, and the set is saved anew", () => {
  const { root, archives, run } = machine();
  const cache = join(root, "cache");
  mkdirSync(cache);
  writeFileSync(join(cache, "libfoo_0.9-1_amd64.deb"), "");
  const r = run("install", "--cache", cache, "--refresh", "socat");
  assert.equal(r.status, 0, r.stderr);
  const apt = r.apt.filter((l) => l.startsWith("apt-get"));
  assert.ok(apt.every((l) => !l.includes("--no-download")), apt.join("\n"));
  assert.ok(apt.some((l) => l.includes("update")));
  // There for apt to reuse when its version is still the one the index names.
  assert.ok(existsSync(join(archives, "libfoo_0.9-1_amd64.deb")));
  assert.match(r.output, /^save=true$/m);
  assert.deepEqual(readdirSync(cache).sort(), ["libbar_1%3a2.0_amd64.deb", "libfoo_1.0-1_amd64.deb"]);
  assert.match(r.summary, /from the mirror/);
});

test("the second attempt keeps the image's own package lists", () => {
  const { run } = machine({ failUpdates: 1 });
  const r = run("install", "socat");
  const update = r.apt.filter((l) => l.startsWith("apt-get") && l.includes("update"))[1];
  assert.match(update, /-o APT::Get::List-Cleanup=0/);
});

test("off Ubuntu a failed install blames the one mirror there is", () => {
  const { run } = machine({ failInstalls: 2, osRelease: null });
  const r = run("install", "socat");
  assert.notEqual(r.status, 0);
  assert.match(r.stdout, /::error[^\n]*the distribution's mirror is slow or down/);
});

// The action around the script: a pull request cannot run it, so its wiring is read here.
test("the action restores this image's key, then the old name, then another image's; main refreshes, and saves only what the mirror gave", () => {
  const action = readFileSync(
    join(dirname(script), "..", ".github", "actions", "linux-packages", "action.yml"),
    "utf8",
  );
  assert.match(
    action,
    /key: \$\{\{ steps\.key\.outputs\.key \}\}\n\s+restore-keys: \|\n\s+\$\{\{ steps\.key\.outputs\.previous \}\}\n\s+\$\{\{ steps\.key\.outputs\.prefix \}\}\n/,
  );
  assert.match(
    action,
    /REFRESH: \$\{\{ github\.ref == 'refs\/heads\/main' && steps\.restore\.outputs\.cache-hit != 'true' \}\}/,
  );
  assert.match(action, /\[ "\$REFRESH" = true \] && args\+=\(--refresh\)/);
  const save = action.match(/- if: (.*)\n\s+uses: actions\/cache\/save@/)?.[1];
  assert.equal(
    save,
    "github.ref == 'refs/heads/main' && steps.restore.outputs.cache-hit != 'true' && steps.install.outputs.save == 'true'",
  );
});
