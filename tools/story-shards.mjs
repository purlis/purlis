#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **How many parts each browser's stories run in** (#3040, `.github/workflows/stories.yml`): the
// sharding trigger. A stories step past `TRIGGER` seconds on main's last good run means more parts
// on the next run, as many as bring the slowest part back under it, so the job's time stays
// bounded as the stories and the pictures grow.
//
//   node tools/story-shards.mjs      (in CI) appends `shards=[1,2,...]` and `of=[N]` to
//                                    `$GITHUB_OUTPUT`
//
// A measurement that cannot be had, such as an API that does not answer, leaves `FLOOR` parts: the
// stories run as they did, and the next run measures again. The runner's own Node runs this, so it
// needs no package.

import { appendFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

/** The longest a stories step should take, in seconds: 8 minutes, the ticket's trigger. A job's
 *  limit is 30, which has to hold its setup and its drawing of every picture that failed too. */
export const TRIGGER = 8 * 60;

/** The fewest parts, what the stories ran in before there was a trigger (#4038). */
export const FLOOR = 3;

/** The most parts, so a measurement that is wrong cannot start dozens of jobs. */
export const CAP = 12;

/** The step of `stories.yml` the trigger measures. */
export const STEP = "The stories, in this shard of this browser";

/** How many parts the next run needs when `of` parts' slowest step took `slowest` seconds. */
export function partsFor(of, slowest) {
  return Math.min(CAP, Math.max(FLOOR, Math.ceil((of * slowest) / TRIGGER)));
}

/**
 * The parts a finished run measured and its slowest stories step in seconds, from the jobs of the
 * Actions API; `undefined` when it holds no finished shard, so there is nothing to measure.
 */
export function measured(jobs) {
  let of;
  let slowest = 0;
  for (const job of jobs) {
    const parts = /^stories \(\w+ \d+\/(\d+)\)$/.exec(job.name);
    const step = job.steps?.find((one) => one.name === STEP);
    if (!parts || step?.conclusion !== "success") continue;
    of = Number(parts[1]);
    const seconds = (Date.parse(step.completed_at) - Date.parse(step.started_at)) / 1000;
    if (seconds > slowest) slowest = seconds;
  }
  return of === undefined ? undefined : { of, slowest };
}

/** The lines the workflow reads, for `of` parts: each a list, as a matrix's every key must be, or
 *  the job silently starts nothing. */
export function outputs(of) {
  const shards = Array.from({ length: of }, (_, at) => at + 1);
  return `shards=${JSON.stringify(shards)}\nof=[${of}]\n`;
}

/** Main's last good stories run's jobs, from the API. */
async function lastGoodJobs(repo, token) {
  const get = async (path) => {
    const answer = await fetch(`https://api.github.com/repos/${repo}/${path}`, {
      headers: { authorization: `Bearer ${token}`, accept: "application/vnd.github+json" },
      signal: AbortSignal.timeout(30_000),
    });
    if (!answer.ok) throw new Error(`${path}: ${answer.status}`);
    return answer.json();
  };
  const runs = await get(
    "actions/workflows/stories.yml/runs?branch=main&event=push&status=success&per_page=1",
  );
  const run = runs.workflow_runs?.[0];
  if (!run) throw new Error("main has no good stories run");
  return (await get(`actions/runs/${run.id}/jobs?per_page=100`)).jobs;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  let of = FLOOR;
  try {
    const { GITHUB_REPOSITORY: repo, GITHUB_TOKEN: token } = process.env;
    const last = measured(await lastGoodJobs(repo, token));
    if (!last) throw new Error("main's last good stories run measured no shard");
    of = partsFor(last.of, last.slowest);
    console.log(`slowest stories step ${Math.round(last.slowest)}s in ${last.of} parts: ${of} now`);
  } catch (error) {
    console.log(`::notice::the stories run in ${FLOOR} parts, unmeasured: ${error.message}`);
  }
  appendFileSync(process.env.GITHUB_OUTPUT, outputs(of));
}
