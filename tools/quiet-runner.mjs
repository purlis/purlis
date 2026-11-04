// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Quiets a macOS runner before the Skill studio's outcome bars measure on it (#4351, the person's
// ruling of 2026-11-05).
//
// Run by the `outcome bars` job's macOS leg: `node tools/quiet-runner.mjs`, and again with
// `--log-only` once the bars have run. GitHub's macOS image spends much of a job on its own: on
// 2026-11-04 its simulator disk-image service sat at 55 to 100% of a core through nearly every L10
// opening, with Spotlight indexing up to 90% beside it, and the same code measured a p95 of about
// 300 ms on one run and 500 on the next. So this turns Spotlight's indexing off, waits for that
// service to settle, and says what is still busy.
//
// The bars and what they measure stay as they are. This only takes noise out, and never fails the
// job: a runner that did not settle is said in a warning, and the bars still judge what they
// measure on it.

import { execFileSync } from "node:child_process";
import process from "node:process";
import { setTimeout as pause } from "node:timers/promises";
import { fileURLToPath } from "node:url";

/** The service that keeps a macOS runner busy, as `ps` names it. */
const SERVICE = "simdiskimaged";
/** Its CPU, in percent of a core, below which it counts as quiet. */
const BELOW_PERCENT = 20;
/** How many samples in a row it has to be quiet for. */
const QUIET_SAMPLES = 3;
const EVERY_MS = 5_000;
/** How long the job waits for it: a macOS runner is scarce, and the bars run either way. */
const BUDGET_MS = 300_000;

/** The first `count` lines of `ps -Ao pcpu=,comm= -r`, each by its program's own name. */
export function busiest(ps, count = 6) {
  const found = [];
  for (const line of ps.split("\n")) {
    const read = line.match(/^\s*([\d.]+)\s+(.+?)\s*$/);
    if (read === null) continue;
    found.push({ cpu: Number(read[1]), name: read[2].split("/").pop() });
  }
  return found.slice(0, count);
}

/**
 * **Waits until `service` has stayed under `belowPercent` for `quietSamples` samples in a row**, or
 * until `budgetMs` has gone. A service with no process running is quiet.
 */
export async function settle({
  sample,
  pause: wait,
  now,
  service,
  belowPercent,
  quietSamples,
  everyMs,
  budgetMs,
}) {
  const from = now();
  let quiet = 0;
  for (;;) {
    const last = sample();
    const cpu = last
      .filter((one) => one.name === service)
      .reduce((sum, one) => sum + one.cpu, 0);
    quiet = cpu < belowPercent ? quiet + 1 : 0;
    const waitedMs = now() - from;
    if (quiet >= quietSamples)
      return { settled: true, waitedMs, service, last };
    if (waitedMs >= budgetMs)
      return { settled: false, waitedMs, service, last };
    await wait(everyMs);
  }
}

const inWords = (busy) =>
  busy.map((one) => `${one.cpu} ${one.name}`).join(", ");

/** What the wait came to, as a GitHub notice or, where the runner stayed busy, a warning. */
export function said({ settled, waitedMs, service, last }) {
  const seconds = Math.round(waitedMs / 1000);
  return settled
    ? `::notice title=Quieting the runner::${service} settled after ${seconds} s. Busiest now: ${inWords(last)}`
    : `::warning title=Quieting the runner::${service} did not settle in ${seconds} s, so the bars measure on a busy runner. Busiest now: ${inWords(last)}`;
}

/** What is busiest on this machine now. */
function sampled() {
  return busiest(
    execFileSync("ps", ["-Ao", "pcpu=,comm=", "-r"], { encoding: "utf8" }),
  );
}

async function main() {
  if (process.argv.includes("--log-only")) {
    console.log(
      `::notice title=Busiest after the outcome bars::${inWords(sampled())}`,
    );
    return;
  }
  try {
    console.log(
      execFileSync("sudo", ["mdutil", "-a", "-i", "off"], { encoding: "utf8" }),
    );
  } catch (error) {
    console.log(
      `::warning title=Quieting the runner::Spotlight stayed on: ${String(error)}`,
    );
  }
  const result = await settle({
    sample: () => {
      const busy = sampled();
      console.log(inWords(busy));
      return busy;
    },
    pause,
    now: () => Date.now(),
    service: SERVICE,
    belowPercent: BELOW_PERCENT,
    quietSamples: QUIET_SAMPLES,
    everyMs: EVERY_MS,
    budgetMs: BUDGET_MS,
  });
  console.log(said(result));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
