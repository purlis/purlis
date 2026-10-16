// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// What `operator-image.yml` checks on the cluster operator's image (#2238) before anything names
// it: its config, and that its binary starts on the image's own architecture. Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { configProblems, startProblems } from "./operator-image-check.mjs";

const GOOD = {
  Os: "linux",
  Architecture: "arm64",
  Config: { User: "65532:65532", Entrypoint: ["/usr/local/bin/purlis-cluster-operator"], Cmd: null },
};

test("the operator's image as operator/Dockerfile makes it has no problem", () => {
  assert.deepEqual(configProblems(GOOD, "arm64"), []);
});

test("an image for another system or architecture is refused", () => {
  assert.match(configProblems({ ...GOOD, Os: "windows" }, "arm64").join("\n"), /linux/);
  assert.match(configProblems(GOOD, "amd64").join("\n"), /arm64.*amd64|amd64.*arm64/);
});

test("an image whose user is root, or not a number, is refused", () => {
  for (const User of ["", "0", "0:0", "root", "nonroot:nonroot"]) {
    const problems = configProblems({ ...GOOD, Config: { ...GOOD.Config, User } }, "arm64");
    assert.match(problems.join("\n"), /user/, User);
  }
});

test("an image that runs anything but the operator is refused", () => {
  for (const Entrypoint of [null, ["/bin/sh"], ["/usr/local/bin/purlis-cluster-operator", "--x"]]) {
    const problems = configProblems({ ...GOOD, Config: { ...GOOD.Config, Entrypoint } }, "arm64");
    assert.match(problems.join("\n"), /default command/, JSON.stringify(Entrypoint));
  }
  const withCmd = configProblems({ ...GOOD, Config: { ...GOOD.Config, Cmd: ["sh"] } }, "arm64");
  assert.match(withCmd.join("\n"), /default command/);
});

const NO_CLUSTER = "ERROR could not reach the cluster's API server error=...";

test("an operator that starts and says it has no cluster to reach has no problem", () => {
  assert.deepEqual(startProblems({ status: 1, output: NO_CLUSTER, timedOut: false }), []);
  // The message comes through the log formatter's colours.
  assert.deepEqual(
    startProblems({ status: 1, output: "\u001b[31mERROR\u001b[0m could not reach the cluster's API server", timedOut: false }),
    [],
  );
});

test("a binary that cannot start, or does not say why it stopped, is refused", () => {
  const cases = [
    { status: 255, output: "exec /usr/local/bin/purlis-cluster-operator: exec format error", timedOut: false },
    { status: 1, output: "/usr/local/bin/purlis-cluster-operator: /lib/aarch64-linux-gnu/libc.so.6: version `GLIBC_2.40' not found", timedOut: false },
    { status: 0, output: NO_CLUSTER, timedOut: false },
    { status: null, output: "", timedOut: true },
  ];
  for (const ran of cases) assert.notDeepEqual(startProblems(ran), [], JSON.stringify(ran));
});
