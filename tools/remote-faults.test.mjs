// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The fault-injection script's argument handling and the commands it would run (#7188). The
// script's effects are the rigs' and run in CI; `FAULTS_DRY_RUN=1` prints each command it would
// send to a container instead, which is what is read here.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(new URL("../tests/remote-faults/faults.sh", import.meta.url));
const VERBS = [
  "drop",
  "restore",
  "restart-sshd",
  "rotate-hostkey",
  "kill-runner",
  "slow",
  "break-registry",
  "blackhole",
];

const run = (args, env = {}) =>
  spawnSync("bash", [SCRIPT, ...args], {
    encoding: "utf8",
    env: { PATH: process.env.PATH, FAULTS_DRY_RUN: "1", ...env },
  });

test("with no verb it exits 2 and lists every verb", () => {
  const ran = run([]);
  assert.equal(ran.status, 2);
  for (const verb of VERBS) assert.match(ran.stderr, new RegExp(`\\b${verb}\\b`));
});

test("--help lists every verb on standard output and exits 0", () => {
  const ran = run(["--help"]);
  assert.equal(ran.status, 0);
  for (const verb of VERBS) assert.match(ran.stdout, new RegExp(`\\b${verb}\\b`));
});

test("a verb it does not know exits 2 and names it", () => {
  const ran = run(["explode"]);
  assert.equal(ran.status, 2);
  assert.match(ran.stderr, /explode/);
});

test("--machine without a name exits 2", () => {
  assert.equal(run(["--machine"]).status, 2);
});

test("slow takes whole seconds, 15 by default, and refuses anything else", () => {
  assert.match(run(["slow"]).stdout, /delay 15s/);
  assert.match(run(["slow", "4"]).stdout, /delay 4s/);
  for (const bad of ["fast", "1.5", "-3", "0"]) {
    const ran = run(["slow", bad]);
    assert.equal(ran.status, 2, bad);
    assert.match(ran.stderr, /whole number of seconds|unknown option/);
  }
});

test("every verb says what it would run, against the machine it is given", () => {
  for (const verb of VERBS) {
    const ran = run(["--machine", "rig-one", verb]);
    assert.equal(ran.status, 0, `${verb}: ${ran.stderr}`);
    assert.match(ran.stdout, /docker exec/, verb);
    assert.match(ran.stdout, /rig-one|dind/, verb);
  }
});

test("drop and blackhole are different faults: a refusal, and a silence", () => {
  const drop = run(["drop"]).stdout;
  assert.match(drop, /iptables/);
  assert.match(drop, /tcp-reset/);
  assert.doesNotMatch(drop, /netem/);
  const blackhole = run(["blackhole"]).stdout;
  assert.match(blackhole, /netem loss 100%/);
  assert.doesNotMatch(blackhole, /iptables/);
});

test("drop checks for its rule before adding it, so a second drop adds nothing", () => {
  assert.match(run(["drop"]).stdout, /iptables -C [^|]*\|\| iptables -I/);
});

test("a fault that changes the link keeps what the link was first, once", () => {
  for (const verb of ["blackhole", "slow"]) {
    const out = run([verb]).stdout;
    assert.match(out, /test -e [^ ]*netem[^ ]* \|\|/, `${verb} saves the baseline only if none is saved`);
  }
});

test("rotate-hostkey keeps the first key once and makes a new one", () => {
  const out = run(["rotate-hostkey"]).stdout;
  assert.match(out, /test -e [^ ]*ssh_host_ed25519_key[^ ]* \|\|/);
  assert.match(out, /ssh-keygen/);
});

test("restore puts back the rule, the link, the host key, the registry and the runners", () => {
  const out = run(["--machine", "sshd", "restore"]).stdout;
  assert.match(out, /iptables -D/);
  assert.match(out, /tc qdisc/);
  assert.match(out, /ssh_host_ed25519_key/);
  assert.match(out, /docker start registry/);
  assert.match(out, /purlis\.runner/);
});

test("restoring the systemd machine leaves dind alone", () => {
  const out = run(["--machine", "ssh-runner-systemd", "restore"]).stdout;
  assert.match(out, /iptables -D/);
  assert.doesNotMatch(out, /dind|docker start|purlis\.runner/);
});

test("the systemd machine's sshd is restarted by systemd, the plain one by a hangup", () => {
  assert.match(run(["--machine", "ssh-runner-systemd", "restart-sshd"]).stdout, /systemctl restart ssh/);
  const plain = run(["--machine", "sshd", "restart-sshd"]).stdout;
  assert.match(plain, /kill -HUP 1/);
  assert.doesNotMatch(plain, /systemctl/);
});

test("kill-runner on a docker machine kills the runners' containers, on a systemd one the host", () => {
  assert.match(run(["--machine", "dind", "kill-runner"]).stdout, /docker kill/);
  assert.match(run(["--machine", "ssh-runner-systemd", "kill-runner"]).stdout, /pkill -KILL/);
});

test("break-registry stops the registry inside dind, whichever machine is named", () => {
  assert.match(run(["--machine", "sshd", "break-registry"]).stdout, /docker exec dind docker stop registry/);
});

test("an sshd restart reloads the daemon before it ends the sessions, on either machine", () => {
  // The other order leaves a window in which the old daemon, with the old host key, still takes
  // the keeper's retry: a link made on the key the fault just replaced (#7188, case 4).
  for (const machine of ["sshd", "ssh-runner-systemd"]) {
    for (const verb of ["restart-sshd", "rotate-hostkey"]) {
      const lines = run(["--machine", machine, verb]).stdout.split("\n");
      const reload = lines.findIndex((line) => /kill -HUP 1|systemctl restart ssh/.test(line));
      const sessions = lines.findIndex((line) => /pkill -KILL -f '\^sshd/.test(line));
      assert.ok(reload >= 0 && sessions >= 0, `${machine} ${verb}: ${lines.join(" | ")}`);
      assert.ok(reload < sessions, `${machine} ${verb} ends the sessions before the reload`);
    }
  }
});
