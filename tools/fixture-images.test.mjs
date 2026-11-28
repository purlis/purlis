// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The remote parity job's other machines, pulled by digest (#7983): what `fixture-images.mjs`
// makes of `tests/parity/fixture-images.json`, and how the workflows that build, push and use the
// images are wired. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { copiedSources, loadManifest, prepare, sourceOf, withPin } from "./fixture-images.mjs";
import { refusals, triggers } from "./no-self-hosted-on-pull-request.test.mjs";

const root = new URL("../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");
const DIGEST = `sha256:${"ab".repeat(32)}`;

const fixture = {
  dockerfile: "tests/remote-docker/sshd.Dockerfile",
  context: "tests/remote-docker",
  files: ["sshd-entrypoint.sh"],
  buildArgs: {},
  image: null,
};
const files = {
  "tests/remote-docker/sshd.Dockerfile": "FROM alpine\nCOPY sshd-entrypoint.sh /e\n",
  "tests/remote-docker/sshd-entrypoint.sh": "#!/bin/sh\n",
};
const readFile = (path) => {
  if (!(path in files)) throw new Error(`no ${path}`);
  return files[path];
};

/** A fake `docker`: records each call, and answers or throws as `answers` says for its verb. */
function docker(answers = {}) {
  const calls = [];
  const run = (args) => {
    calls.push(args.join(" "));
    const answer = answers[args[0]];
    if (answer instanceof Error) throw answer;
    return answer ?? "";
  };
  return { calls, run };
}

test("the source of a fixture is its Dockerfile, the files it copies and its build arguments", () => {
  const one = sourceOf(fixture, readFile);
  assert.match(one, /^sha256:[0-9a-f]{64}$/);
  assert.equal(sourceOf(fixture, readFile), one, "the same files give the same source");
  assert.notEqual(sourceOf({ ...fixture, buildArgs: { POOL_UID: "1001" } }, readFile), one);
  const changed = { ...files, "tests/remote-docker/sshd-entrypoint.sh": "#!/bin/sh\nexit 1\n" };
  assert.notEqual(sourceOf(fixture, (p) => changed[p]), one, "a copied file changed");
});

test("every file a fixture's Dockerfile copies is one its source names", () => {
  assert.deepEqual(copiedSources("FROM x\nCOPY --chmod=0755 a.sh /a\nCOPY b c /d/\nRUN true\n"), ["a.sh", "b", "c"]);
  const manifest = loadManifest((p) => read(p));
  assert.deepEqual(Object.keys(manifest.fixtures).sort(), [
    "remote-docker-sshd",
    "remote-podman-sshd",
    "ssh-runner-sshd",
    "ssh-runner-systemd",
  ]);
  for (const [name, f] of Object.entries(manifest.fixtures)) {
    for (const copied of copiedSources(read(f.dockerfile))) {
      assert.ok(f.files.includes(copied), `${name}: ${f.dockerfile} copies ${copied}, which its files do not name`);
    }
    // Pinned by digest, in the organisation's own registry, or not pinned yet.
    if (f.image !== null) {
      assert.match(f.image.ref, /^ghcr\.io\/purlis\/purlis-parity-[a-z-]+@sha256:[0-9a-f]{64}$/, name);
      assert.match(f.image.source, /^sha256:[0-9a-f]{64}$/, name);
    }
  }
});

test("a current pin is pulled by its digest and tagged for its up.sh, and nothing is built", () => {
  const ref = `ghcr.io/purlis/purlis-parity-remote-docker-sshd@${DIGEST}`;
  const pinned = { ...fixture, image: { ref, source: sourceOf(fixture, readFile) } };
  const d = docker();
  const said = prepare({ name: "remote-docker-sshd", fixture: pinned, tag: "remote-docker-sshd", readFile, docker: d.run });
  assert.deepEqual(d.calls, [`pull ${ref}`, `tag ${ref} remote-docker-sshd`]);
  assert.equal(said.how, "pulled");
});

test("a pin its files have moved past is built here, and says so", () => {
  const ref = `ghcr.io/purlis/purlis-parity-remote-docker-sshd@${DIGEST}`;
  const stale = { ...fixture, image: { ref, source: `sha256:${"0".repeat(64)}` } };
  const d = docker();
  const said = prepare({ name: "remote-docker-sshd", fixture: stale, tag: "t", readFile, docker: d.run, contextDir: "/ctx", stage: () => {} });
  assert.equal(said.how, "built");
  assert.match(said.notice, /changed since it was pinned/);
  assert.equal(d.calls.length, 1);
  assert.match(d.calls[0], /^build --file \/ctx\/sshd\.Dockerfile --tag t \/ctx$/);
});

test("an unpinned fixture is built here, with its build arguments", () => {
  const d = docker();
  const said = prepare({
    name: "remote-podman-sshd",
    fixture: { ...fixture, buildArgs: { POOL_UID: "1001" } },
    tag: "t",
    readFile,
    docker: d.run,
    contextDir: "/ctx",
    stage: () => {},
  });
  assert.equal(said.how, "built");
  assert.match(said.notice, /not pinned yet/);
  assert.match(d.calls[0], /--build-arg POOL_UID=1001/);
});

test("a pinned image that will not pull is one cause, naming the fixture and its image", () => {
  const ref = `ghcr.io/purlis/purlis-parity-ssh-runner-systemd@${DIGEST}`;
  const pinned = { ...fixture, image: { ref, source: sourceOf(fixture, readFile) } };
  const d = docker({ pull: new Error("Error response from daemon: denied\n") });
  assert.throws(
    () => prepare({ name: "ssh-runner-systemd", fixture: pinned, tag: "t", readFile, docker: d.run, tries: 2, wait: () => {} }),
    (error) => {
      assert.match(error.message, /^fixture image ssh-runner-systemd \(ghcr\.io\/purlis\/purlis-parity-ssh-runner-systemd@sha256:[0-9a-f]+\) unavailable/);
      assert.match(error.message, /denied/);
      return true;
    },
  );
  assert.deepEqual(d.calls, [`pull ${ref}`, `pull ${ref}`], "asked twice, then given up");
});

test("a fixture that will not build here is one cause too", () => {
  const d = docker({ build: new Error("ERROR: unable to select packages") });
  assert.throws(
    () => prepare({ name: "remote-docker-sshd", fixture, tag: "t", readFile, docker: d.run, contextDir: "/ctx", stage: () => {} }),
    (error) =>
      /^fixture image remote-docker-sshd could not be built from tests\/remote-docker\/sshd\.Dockerfile.*unable to select packages/s.test(
        error.message,
      ),
  );
});

test("a pin is written with the image's digest and the source it was built from", () => {
  const manifest = { fixtures: { "ssh-runner-sshd": { ...fixture } } };
  const ref = `ghcr.io/purlis/purlis-parity-ssh-runner-sshd@${DIGEST}`;
  const next = withPin(manifest, "ssh-runner-sshd", ref, readFile);
  assert.deepEqual(next.fixtures["ssh-runner-sshd"].image, { ref, source: sourceOf(fixture, readFile) });
  assert.equal(manifest.fixtures["ssh-runner-sshd"].image, null, "the manifest read is left as it was");
  assert.throws(() => withPin(manifest, "ssh-runner-sshd", "ghcr.io/purlis/x:latest", readFile), /by digest/);
  assert.throws(() => withPin(manifest, "nope", ref, readFile), /no fixture/);
});

// ---- The workflows ----

// Read when it is there, so the tests of the tool run (and the workflow's fail) without it.
const workflow = (() => {
  try {
    return read(".github/workflows/parity-fixture-images.yml");
  } catch {
    return "";
  }
})();

/** The jobs of a workflow's text, by name, each its lines with comments dropped. */
function jobs(text) {
  const lines = text.split("\n");
  const out = new Map();
  let current = null;
  for (const line of lines.slice(lines.indexOf("jobs:") + 1)) {
    if (/^\S/.test(line)) break;
    if (/^ {2}[\w-]+:$/.test(line)) out.set((current = line.trim().slice(0, -1)), []);
    else if (current && !line.trim().startsWith("#")) out.get(current).push(line);
  }
  return out;
}

test("the fixture images are pushed only from main, by jobs that run no pull request's code", () => {
  assert.deepEqual(triggers(workflow), ["push", "pull_request", "workflow_dispatch"]);
  assert.match(workflow, /^ {2}push:\n {4}branches: \[main\]$/m);
  assert.match(workflow, /^permissions: \{\}$/m);
  assert.deepEqual(refusals(workflow), []);
  const all = jobs(workflow);
  const writers = [...all].filter(([, body]) => body.some((l) => /: write$/.test(l.trim())));
  assert.deepEqual(writers.map(([name]) => name).sort(), ["pin", "publish"]);
  for (const [name, body] of writers) {
    assert.ok(
      body.includes(
        "    if: needs.plan.outputs.publish == 'true' && github.ref == 'refs/heads/main' && (github.event_name == 'push' || github.event_name == 'workflow_dispatch')",
      ),
      `${name} runs where plan does not publish, or off a push or dispatch on main`,
    );
    // No checkout of anything but main's commit: `plan`'s sha, which is the push's own.
    for (const line of body.filter((l) => l.trim().startsWith("ref:"))) {
      assert.equal(line.trim(), "ref: ${{ needs.plan.outputs.sha }}", name);
    }
    assert.ok(!body.some((l) => l.includes("pull_request")), `${name} reads the pull request`);
  }
  const plan = all.get("plan").map((l) => l.trim());
  assert.ok(plan.includes("EVENT: ${{ github.event_name }}"));
  assert.ok(plan.includes("REF: ${{ github.ref }}"));
  assert.ok(plan.some((l) => l.startsWith("if [ \"$REF\" = refs/heads/main ] && { [ \"$EVENT\" = push ] || [ \"$EVENT\" = workflow_dispatch ]; }")));
  // The build of a pull request holds nothing that writes.
  assert.ok(all.get("build").includes("      contents: read"));
});

test("the pins a publish writes are not a change that starts another publish", () => {
  const paths = workflow.split("\n");
  const at = paths.indexOf("  push:");
  const pushPaths = [];
  for (const line of paths.slice(at + 1)) {
    if (/^ {2}\S/.test(line)) break;
    const m = /^ {6}- (.+)$/.exec(line);
    if (m) pushPaths.push(m[1]);
  }
  assert.ok(pushPaths.length > 0);
  assert.ok(!pushPaths.some((p) => p.includes("fixture-images.json")), pushPaths.join(", "));
});

test("remote parity prepares every fixture image before any machine comes up", () => {
  const ci = read(".github/workflows/ci.yml");
  const job = jobs(ci).get("remote-parity").join("\n");
  const prepareAt = job.indexOf("tools/fixture-images.mjs prepare-all");
  assert.ok(prepareAt > 0, "no step prepares the fixture images");
  for (const up of ["tests/remote-docker/up.sh", "tests/remote-podman/up.sh", "tests/ssh-runner/up.sh", "tests/ssh-runner/up-systemd.sh"]) {
    assert.ok(job.indexOf(up) > prepareAt, `${up} comes before the images are prepared`);
  }
  for (const script of ["tests/remote-docker/up.sh", "tests/remote-podman/up.sh", "tests/ssh-runner/up.sh", "tests/ssh-runner/up-systemd.sh"]) {
    assert.ok(!/docker build[^\n]*sshd\.Dockerfile|docker build[^\n]*systemd\.Dockerfile/.test(read(script)), `${script} builds its own image`);
  }
});

test("no step that runs after a failure runs where a fixture it needs did not come up", () => {
  const ci = read(".github/workflows/ci.yml");
  const body = jobs(ci).get("remote-parity");
  const steps = [];
  for (const line of body) {
    if (/^ {6}- /.test(line)) steps.push([line]);
    else if (steps.length) steps.at(-1).push(line);
  }
  const named = (name) => steps.find((s) => s.some((l) => l.includes(`name: ${name}`)));
  const ifOf = (step) => step.map((l) => l.trim()).find((l) => l.startsWith("if:")) ?? "";
  const fault = named("The fault suite, on the pool and the systemd machine");
  assert.ok(fault, "no fault suite step");
  for (const needed of ["images", "machine", "systemd"]) {
    assert.ok(ifOf(fault).includes(`steps.${needed}.outcome == 'success'`), `the fault suite runs without ${needed}: ${ifOf(fault)}`);
  }
  const behaviour = named("The behaviour suite on its remote Docker pool target");
  assert.ok(ifOf(behaviour).includes("steps.machine.outcome == 'success'"), ifOf(behaviour));
});
