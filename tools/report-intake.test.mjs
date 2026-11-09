// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The steward's daily Report intake routine (#5701), read from recorded answers of Sentry's
// project events API (`tools/report-intake-fixtures/`, `full=true`) and of
// `gh issue list --label reported`. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  INTAKE,
  MISSING_TOKEN,
  fetchEvents,
  group,
  isoWeek,
  markersIn,
  nextPage,
  parseArgs,
  plan,
  reportOf,
  summary,
  weekly,
} from "./report-intake.mjs";

const fixture = (name) =>
  JSON.parse(readFileSync(new URL(`./report-intake-fixtures/${name}`, import.meta.url), "utf8"));

const reports = fixture("events.json").map(reportOf);
const issues = fixture("reported-issues.json");
const byAction = (steps, action) => steps.filter((step) => step.action === action);

test("no new events: nothing to file or add, and the summary says so", () => {
  const steps = plan(fixture("events-none.json").map(reportOf), issues);
  assert.deepEqual(steps, []);
  const text = summary(steps, [], { day: "2026-11-09" });
  assert.match(text, /No new reports since the last run\./);
  assert.match(text, /No reports in the window\./);
});

test("a new report becomes an issue labelled reported, with a link back to each of its events", () => {
  const [filed] = byAction(plan(reports, issues), "file").filter((step) => step.title.includes("window failed"));
  assert.equal(filed.label, "reported");
  assert.equal(filed.reports, 2, "the same device fingerprint twice is one issue");
  assert.equal(filed.versions, "0.42.1 (dev)");
  assert.match(filed.body, /https:\/\/purlis\.sentry\.io\/issues\/7002\/events\/e2000000000000000000000000000002\//);
  assert.match(filed.body, /https:\/\/purlis\.sentry\.io\/issues\/7002\/events\/e3000000000000000000000000000003\//);
  assert.deepEqual([...markersIn(filed.body).fingerprints], ["0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b"]);
  assert.deepEqual([...markersIn(filed.body).events], ["e2000000000000000000000000000002", "e3000000000000000000000000000003"]);
});

test("a duplicate of an open issue becomes a 👍 and a comment on it, matched by the device fingerprint", () => {
  const [added] = byAction(plan(reports, issues), "add");
  assert.equal(added.issue, 9001);
  assert.equal(added.reaction, "+1");
  assert.equal(added.reports, 1, "the event a comment already names is not counted again");
  assert.match(added.comment, /e1000000000000000000000000000001/);
  assert.doesNotMatch(added.comment, /e7000000000000000000000000000007/);
});

test("a report of a closed issue is put to the steward on that issue, matched by its Sentry issue", () => {
  const [closed] = byAction(plan(reports, issues), "closed");
  assert.equal(closed.issue, 9002);
  assert.match(closed.comment, /after this issue was closed/);
});

test("words an agent added to a sent draft join that draft's group, though Sentry filed them apart", () => {
  const groups = group(reports.filter((report) => report.id.startsWith("e4") || report.id.startsWith("e5")));
  assert.equal(groups.length, 1);
  assert.deepEqual([...groups[0].sentryIssues], ["7004", "7005"]);
  const [draft] = byAction(plan(reports, issues), "file").filter((step) => step.title.includes("Changes tab"));
  assert.equal(draft.reports, 2);
  assert.match(draft.body, /written by an agent and sent unread/);
  assert.equal(draft.texts.length, 2, "the steward reads both the draft and the words added to it");
  assert.ok(
    draft.texts.every((text) => !draft.body.includes(text)),
    "an unread report's words are the steward's to read, never pasted into a public issue (ADR 0118)",
  );
});

test("an event a marker already names is never planned again, so a second run the same day plans nothing new", () => {
  const first = plan(reports, issues);
  const applied = [
    ...issues.map((issue) => ({ ...issue, comments: [...issue.comments] })),
    ...byAction(first, "file").map((step, i) => ({ number: 9100 + i, title: step.title, state: "OPEN", body: step.body, comments: [] })),
  ];
  for (const step of first.filter((step) => step.comment)) {
    applied.find((issue) => issue.number === step.issue).comments.push({ body: step.comment });
  }
  assert.deepEqual(plan(reports, applied), []);
});

test("the summary counts reports per week by version and channel, and how many are distinct", () => {
  const older = { ...reports[1], id: "older", at: "2026-11-02T10:00:00Z", repeats: [] };
  const rows = weekly([...reports, older]);
  assert.deepEqual(rows, [
    { week: "2026-W46", version: "0.42.1", channel: "stable", reports: 3, repeats: 0, distinct: 2 },
    { week: "2026-W46", version: "0.42.1", channel: "dev", reports: 2, repeats: 3, distinct: 1 },
    { week: "2026-W45", version: "0.42.0", channel: "stable", reports: 2, repeats: 0, distinct: 1 },
    { week: "2026-W45", version: "0.42.1", channel: "dev", reports: 1, repeats: 0, distinct: 1 },
  ]);
  const text = summary(plan(reports, issues), reports, { day: "2026-11-09" });
  assert.match(text, /6 new reports in 4 distinct groups: 2 to file, 1 added to open issues, 1 on closed issues\./);
  assert.match(text, /\| 2026-W46 \| 0\.42\.1 \| stable \| 3 \| 2 \| 0 \|/);
});

test("weeks are ISO weeks, a year's first days belonging to the week of their Thursday", () => {
  assert.equal(isoWeek("2026-11-09T00:00:00Z"), "2026-W46");
  assert.equal(isoWeek("2026-11-08T23:59:59Z"), "2026-W45");
  assert.equal(isoWeek("2027-01-01T12:00:00Z"), "2026-W53");
  assert.equal(isoWeek("2024-12-30T12:00:00Z"), "2025-W01");
});

const answer = (status, body, link) => ({
  status,
  ok: status >= 200 && status < 300,
  headers: { get: (name) => (name === "link" ? link : null) },
  json: async () => body,
});

test("every page of events is read, with the token sent to the intake's EU host only", async () => {
  const base = `${INTAKE.api}/api/0/projects/purlis/purlis-app/events/`;
  const asked = [];
  const pages = [
    answer(200, fixture("events.json").slice(0, 4), `<${base}?&cursor=0:0:1>; rel="previous"; results="false"; cursor="0:0:1", <${base}?&cursor=0:100:0>; rel="next"; results="true"; cursor="0:100:0"`),
    answer(200, fixture("events.json").slice(4), `<${base}?&cursor=0:100:1>; rel="previous"; results="true"; cursor="0:100:1", <${base}?&cursor=0:200:0>; rel="next"; results="false"; cursor="0:200:0"`),
  ];
  const events = await fetchEvents({
    token: "t0ken",
    hours: 26,
    fetcher: async (url, init) => {
      asked.push({ url, auth: init.headers.Authorization });
      return pages.shift();
    },
  });
  assert.equal(events.length, 7);
  assert.equal(asked[0].url, `${base}?full=true&statsPeriod=26h`);
  assert.ok(asked.every(({ url, auth }) => url.startsWith("https://de.sentry.io/") && auth === "Bearer t0ken"));
});

test("a refused token says to run the wizard's token step again, without the token", async () => {
  await assert.rejects(
    fetchEvents({ token: "t0ken", hours: 26, fetcher: async () => answer(401, fixture("refused.json")) }),
    (error) => /HTTP 401/.test(error.message) && /#5687/.test(error.message) && !error.message.includes("t0ken"),
  );
});

test("a next page on another host is refused, so the token never leaves for it", async () => {
  await assert.rejects(
    fetchEvents({ token: "t0ken", hours: 26, fetcher: async () => answer(200, [], `<https://evil.example/x>; rel="next"; results="true"`) }),
    /refused to send the token to https:\/\/evil\.example/,
  );
});

test("the last page is the one whose next link has no results", () => {
  assert.equal(nextPage('<https://de.sentry.io/a>; rel="next"; results="false"; cursor="x"'), null);
  assert.equal(nextPage(null), null);
});

test("a missing token says to run the Sentry setup wizard (#5687), exits 2 and reads nothing", () => {
  const env = { ...process.env };
  delete env.SENTRY_INTAKE_TOKEN;
  const run = spawnSync(process.execPath, [fileURLToPath(new URL("./report-intake.mjs", import.meta.url)), "plan"], {
    encoding: "utf8",
    env,
  });
  assert.equal(run.status, 2);
  assert.equal(run.stdout, "");
  assert.equal(run.stderr, `${MISSING_TOKEN}\n`);
  assert.match(run.stderr, /vault `sentry` has no `intake-read-token`/);
  assert.match(run.stderr, /Sentry setup wizard \(#5687\)/);
});

test("options are whole numbers above 0, and only plan is a command", () => {
  assert.deepEqual(parseArgs(["plan"]), { command: "plan", sinceHours: 26, weeks: 4 });
  assert.deepEqual(parseArgs(["plan", "--since-hours", "48", "--weeks", "8"]), { command: "plan", sinceHours: 48, weeks: 8 });
  assert.throws(() => parseArgs(["plan", "--weeks", "0"]), /whole number/);
  assert.throws(() => parseArgs(["file"]), /usage/);
});
