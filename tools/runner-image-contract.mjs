#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Checks a container image against the runner image contract (docs/runner-image.md, #2043).
//
//   node tools/runner-image-contract.mjs [--carries PROGRAM,PROGRAM] IMAGE
//
// IMAGE is an image the local Docker already has. `--carries` names programs this image promises
// beyond the contract, each of which has to answer `--version`: purlis's own carries `gh` and
// `glab` (#4354), and a team's need not. Three checks, and every problem each one finds
// is printed, one to a line, before the exit status says whether there was any:
//
//   1. its config (`docker image inspect`): Linux, a numbered user that is not root, an absolute
//      HOME, and a default command that runs `purlis serve` under an init (`configProblems`);
//   2. what it holds, by running `runner-image-contract.sh` inside it as its own user;
//   3. that its default command starts: `purlis serve` says `ready` within a minute.
//
// `runner-image.yml` runs it on purlis's runner image and on the example of a team's own image,
// for each architecture, before anything is pushed.
import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** The inits that reap the processes a session host's chats leave behind when it is PID 1. */
const INITS = ["tini", "dumb-init", "catatonit", "docker-init"];

/** How long `purlis serve` has to say it is ready. */
const READY_SECONDS = 60;

/**
 * What is wrong with an image's config, as `docker image inspect` reports one image (its `Os`
 * and its `Config`), one sentence per problem; empty when it keeps the contract.
 */
export function configProblems(image) {
  const problems = [];
  const config = image.Config ?? {};
  if (image.Os !== "linux") {
    problems.push(`the image is for ${image.Os || "no operating system"}, and a runner image is for linux`);
  }

  const user = (config.User ?? "").split(":")[0];
  if (!/^[0-9]+$/.test(user) || Number(user) === 0) {
    problems.push(
      `the image's user is ${JSON.stringify(config.User ?? "")}; it has to be a user number other than 0 (USER 10001), so a cluster can check it is not root`,
    );
  }

  const home = (config.Env ?? []).find((entry) => entry.startsWith("HOME="))?.slice("HOME=".length);
  if (!home?.startsWith("/")) {
    problems.push("the image sets no absolute HOME; purlis keeps its config, data and a kept claim's volume there");
  }

  const command = [...(config.Entrypoint ?? []), ...(config.Cmd ?? [])];
  const serve = command.findIndex((word, i) => basename(word) === "purlis" && command[i + 1] === "serve");
  if (serve < 0 || serve + 2 !== command.length) {
    problems.push(`the image's default command is ${JSON.stringify(command)}; it has to end in purlis serve`);
  } else if (serve === 0 || !INITS.includes(basename(command[0]))) {
    problems.push(
      `the image's default command starts ${JSON.stringify(command[0])}; purlis serve runs under an init that reaps (${INITS.join(", ")})`,
    );
  }
  return problems;
}

function docker(args, options = {}) {
  return spawnSync("docker", args, { encoding: "utf8", ...options });
}

/** What `runner-image-contract.sh` finds wrong inside `image`, run as the image's own user. */
function insideProblems(image, carries) {
  const script = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "runner-image-contract.sh"), "utf8");
  const carried = ["--env", `CONTRACT_CARRIES=${carries.join(" ")}`];
  const run = docker(["run", "--rm", "-i", "--network", "none", ...carried, "--entrypoint", "/bin/sh", image, "-s"], {
    input: script,
  });
  const problems = run.stdout.split("\n").filter(Boolean);
  if (run.status !== 0 && problems.length === 0) {
    problems.push(`the inside check could not run in the image (exit ${run.status}): ${run.stderr.trim()}`);
  }
  return problems;
}

/** Whether the image's own default command, `purlis serve`, says `ready` in time. */
function serveProblems(image) {
  const name = `runner-image-contract-${randomBytes(4).toString("hex")}`;
  return new Promise((resolve) => {
    const child = spawn("docker", ["run", "--rm", "--name", name, "--network", "none", image], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let out = "";
    let err = "";
    let done = false;
    const finish = (problems) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      docker(["rm", "--force", name]);
      resolve(problems);
    };
    const timer = setTimeout(
      () => finish([`purlis serve did not say ready within ${READY_SECONDS} s; it said: ${(out + err).trim()}`]),
      READY_SECONDS * 1000,
    );
    child.stdout.on("data", (chunk) => {
      out += chunk;
      if (out.split("\n").some((line) => line.trim() === "ready")) finish([]);
    });
    child.stderr.on("data", (chunk) => (err += chunk));
    child.on("exit", (code) => finish([`purlis serve ended (exit ${code}) before it was ready: ${(out + err).trim()}`]));
  });
}

/**
 * The command line's image and the programs it is to carry, or null where it is not one this
 * tool takes. A program is a plain name, as it goes into the inside check's shell.
 */
export function parseArguments(args) {
  const words = [...args];
  let carries = [];
  if (words[0] === "--carries") {
    carries = (words[1] ?? "").split(",");
    if (!carries.every((program) => /^[a-z][a-z0-9-]*$/.test(program))) return null;
    words.splice(0, 2);
  }
  return words.length === 1 && !words[0].startsWith("-") ? { image: words[0], carries } : null;
}

async function main(image, carries) {
  const inspected = docker(["image", "inspect", image]);
  if (inspected.status !== 0) {
    console.log(`there is no image ${image} here: ${inspected.stderr.trim()}`);
    return 1;
  }
  const problems = [
    ...configProblems(JSON.parse(inspected.stdout)[0]),
    ...insideProblems(image, carries),
    ...(await serveProblems(image)),
  ];
  for (const problem of problems) console.log(problem);
  console.log(problems.length === 0 ? `${image} keeps the runner image contract` : `${image} breaks the runner image contract`);
  return problems.length === 0 ? 0 : 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const parsed = parseArguments(process.argv.slice(2));
  if (!parsed) {
    console.error("usage: node tools/runner-image-contract.mjs [--carries PROGRAM,PROGRAM] IMAGE");
    process.exit(2);
  }
  process.exitCode = await main(parsed.image, parsed.carries);
}
