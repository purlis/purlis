// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Waits for a macOS runner to go quiet before the Skill studio's outcome bars measure on it (#4351,
// the person's ruling of 2026-11-05).
//
// Run by the `outcome bars` job's macOS leg before each harness's bars: `node tools/quiet-runner.mjs`,
// and with `--log-only` once they have run. GitHub's macOS image spends much of a job on its own:
// on 2026-11-04 its simulator disk-image service sat at 55 to 100% of a core through nearly every
// L10 opening, with Spotlight up to 90% beside it, and the same code measured a p95 of about 300 ms
// on one run and 500 on the next. So the job turns Spotlight's indexing off before it builds
// (`ci.yml`), and this waits for what is left of it and for that service, and says what is busy.
//
// The bars and what they measure stay as they are. This only takes noise out, and never fails the
// job: a runner that did not settle is said in a warning, and the bars still judge what they
// measure on it.

import { execFileSync } from "node:child_process";
import process from "node:process";
import { setTimeout as pause } from "node:timers/promises";
import { fileURLToPath } from "node:url";

/** What keeps a macOS runner busy, as `ps` names it: the disk images, and Spotlight's backlog. */
const SERVICES = ["simdiskimaged", "mds_stores", "mds", "mdworker_shared"];
/** Their CPU together, in percent of a core, below which the runner counts as quiet. */
const BELOW_PERCENT = 20;
/** How many samples in a row it has to be quiet for: half a minute, more than a lull. */
const QUIET_SAMPLES = 6;
const EVERY_MS = 5_000;
/** How long a harness's bars wait for it: a macOS runner is scarce, and they run either way. */
const BUDGET_MS = 180_000;

/** Every line of `ps -Ao pcpu=,comm= -r`, by its program's own name. */
export function busiest(ps) {
  const found = [];
  for (const line of ps.split("\n")) {
    const read = line.match(/^\s*([\d.,]+)\s+(.+?)\s*$/);
    if (read === null) continue;
    found.push({
      cpu: Number(read[1].replace(",", ".")),
      name: read[2].split("/").pop(),
    });
  }
  return found;
}

/** The busiest few of a reading, which is what is said of it. */
export const topOf = (busy, count = 6) => busy.slice(0, count);

/**
 * **Waits until `services` together have stayed under `belowPercent` for `quietSamples` samples in
 * a row**, or until `budgetMs` has gone. One with no process running is quiet; a reading of nothing
 * at all is no reading, and never counts as quiet.
 */
export async function settle({
  sample,
  pause: wait,
  now,
  services,
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
      .filter((one) => services.includes(one.name))
      .reduce((sum, one) => sum + one.cpu, 0);
    quiet = last.length > 0 && cpu < belowPercent ? quiet + 1 : 0;
    const waitedMs = now() - from;
    const ended = { waitedMs, services, last: topOf(last) };
    if (quiet >= quietSamples) return { settled: true, ...ended };
    if (waitedMs >= budgetMs) return { settled: false, ...ended };
    await wait(everyMs);
  }
}

const inWords = (busy) => busy.map((one) => `${one.cpu} ${one.name}`).join(", ");

/** What the wait came to, as a GitHub notice or, where the runner stayed busy, a warning. */
export function said({ settled, waitedMs, services, last }) {
  const seconds = Math.round(waitedMs / 1000);
  const service = services.join(", ");
  return settled
    ? `::notice title=Quieting the runner::${service} settled after ${seconds} s. Busiest now: ${inWords(last)}`
    : `::warning title=Quieting the runner::${service} did not settle in ${seconds} s, so the bars measure on a busy runner. Busiest now: ${inWords(last)}`;
}

/** What is running on this machine now, busiest first; nothing where `ps` could not say. */
function sampled() {
  try {
    return busiest(
      execFileSync("ps", ["-Ao", "pcpu=,comm=", "-r"], {
        encoding: "utf8",
        timeout: 10_000,
      }),
    );
  } catch (error) {
    console.log(`ps failed: ${String(error).split("\n")[0]}`);
    return [];
  }
}

async function main() {
  if (process.argv.includes("--log-only")) {
    console.log(`::notice title=Busiest after the outcome bars::${inWords(topOf(sampled()))}`);
    return;
  }
  const result = await settle({
    sample: () => {
      const busy = sampled();
      console.log(inWords(topOf(busy)));
      return busy;
    },
    pause,
    now: () => Date.now(),
    services: SERVICES,
    belowPercent: BELOW_PERCENT,
    quietSamples: QUIET_SAMPLES,
    everyMs: EVERY_MS,
    budgetMs: BUDGET_MS,
  });
  console.log(said(result));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
