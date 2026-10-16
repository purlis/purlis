#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Checks the cluster operator's image (operator/Dockerfile, #2238) before anything names it.
//
//   node tools/operator-image-check.mjs IMAGE ARCH
//
// IMAGE is an image the local Docker already has, ARCH `amd64` or `arm64`. Two checks, and every
// problem each one finds is printed, one to a line, before the exit status says whether there
// was any:
//
//   1. its config (`docker image inspect`): Linux on ARCH, a numbered user that is not root, and
//      the operator as its whole default command (`configProblems`);
//   2. that its binary starts on ARCH: run as the cluster runs it (read-only, no capability, no
//      network) and with no cluster to reach, it stops and says so (`startProblems`).
//
// What the operator does in a cluster is `operator.yml`'s kind tests, on the same commit, which
// `operator-image.yml` waits for before it builds this image.
import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { fileURLToPath } from "node:url";

const BINARY = "/usr/local/bin/purlis-cluster-operator";

/** What the operator says when it finds no cluster's API server to reach (operator/src/main.rs). */
const NO_CLUSTER = "could not reach the cluster's API server";

/** How long the binary has to stop when there is no cluster. */
const START_SECONDS = 30;

/**
 * What is wrong with an image's config, as `docker image inspect` reports one image (its `Os`,
 * `Architecture` and `Config`), one sentence per problem; empty when there is nothing.
 */
export function configProblems(image, arch) {
  const problems = [];
  const config = image.Config ?? {};
  if (image.Os !== "linux") {
    problems.push(`the image is for ${image.Os || "no operating system"}, and the operator's image is for linux`);
  }
  if (image.Architecture !== arch) {
    problems.push(`the image is for ${image.Architecture || "no architecture"}, and this build is for ${arch}`);
  }
  const user = (config.User ?? "").split(":")[0];
  if (!/^[0-9]+$/.test(user) || Number(user) === 0) {
    problems.push(
      `the image's user is ${JSON.stringify(config.User ?? "")}; it has to be a user number other than 0 (USER 65532), so the cluster can check runAsNonRoot`,
    );
  }
  const command = [...(config.Entrypoint ?? []), ...(config.Cmd ?? [])];
  if (command.length !== 1 || command[0] !== BINARY) {
    problems.push(`the image's default command is ${JSON.stringify(command)}; it has to be ${JSON.stringify([BINARY])}`);
  }
  return problems;
}

/**
 * What is wrong with how the binary ran with no cluster to reach: `{ status, output, timedOut }`,
 * its exit status, everything it printed, and whether it was stopped for taking too long.
 */
export function startProblems({ status, output, timedOut }) {
  const said = output.trim() || "nothing";
  if (timedOut) return [`with no cluster to reach, the operator did not stop within ${START_SECONDS} s; it said: ${said}`];
  if (status === 0 || !output.includes(NO_CLUSTER)) {
    return [`with no cluster to reach, the operator should stop saying "${NO_CLUSTER}"; it ended (exit ${status}) saying: ${said}`];
  }
  return [];
}

function docker(args, options = {}) {
  return spawnSync("docker", args, { encoding: "utf8", ...options });
}

function main(image, arch) {
  const inspected = docker(["image", "inspect", image]);
  if (inspected.status !== 0) {
    console.log(`there is no image ${image} here: ${inspected.stderr.trim()}`);
    return 1;
  }
  // As operator/deploy/operator.yaml runs it, with neither a network nor a cluster's credential.
  const name = `operator-image-check-${randomBytes(4).toString("hex")}`;
  const ran = docker(
    ["run", "--rm", "--name", name, "--network", "none", "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges", image],
    { timeout: START_SECONDS * 1000 },
  );
  // A run stopped for taking too long leaves its container behind.
  docker(["rm", "--force", name]);
  const problems = [
    ...configProblems(JSON.parse(inspected.stdout)[0], arch),
    ...startProblems({
      status: ran.status,
      output: `${ran.stdout ?? ""}${ran.stderr ?? ""}`,
      timedOut: ran.error?.code === "ETIMEDOUT",
    }),
  ];
  for (const problem of problems) console.log(problem);
  console.log(problems.length === 0 ? `${image} is the operator's image for ${arch}` : `${image} is not a good operator image`);
  return problems.length === 0 ? 0 : 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [image, arch] = process.argv.slice(2);
  if (!image || !/^(amd64|arm64)$/.test(arch ?? "") || process.argv.length > 4) {
    console.error("usage: node tools/operator-image-check.mjs IMAGE amd64|arm64");
    process.exit(2);
  }
  process.exitCode = main(image, arch);
}
