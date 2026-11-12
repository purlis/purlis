// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The steward's daily Report intake routine (#5701), read from recorded answers of Sentry's
// project events API as curl saved them (`tools/report-intake-fixtures/`, `full=true`) and of
// `gh issue list --label reported`. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  MISSING_TOKEN,
  firstPage,
  group,
  hasToken,
  isoWeek,
  markersIn,
  merge,
  milestoneFor,
  nextPage,
  nextPageOf,
  parseArgs,
  plan,
  readPages,
  readShortIds,
  reportOf,
  summary,
  weekly,
} from "./report-intake.mjs";

const fixtures = fileURLToPath(new URL("./report-intake-fixtures/", import.meta.url));
const fixture = (name) => JSON.parse(readFileSync(join(fixtures, name), "utf8"));
const saved = (name) => readFileSync(join(fixtures, name), "utf8");
const tool = fileURLToPath(new URL("./report-intake.mjs", import.meta.url));

const shortIds = readShortIds(join(fixtures, "pages"));
const reports = readPages(join(fixtures, "pages")).map((event) => reportOf(event, shortIds));
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
  assert.deepEqual(filed.labels, ["reported", "priority:p0"], "every issue the routine files is urgent");
  assert.equal(filed.reports, 2, "the same device fingerprint twice is one issue");
  assert.equal(filed.versions, "0.42.1 (dev)");
  assert.match(filed.body, /https:\/\/purlis-inc-ff\.sentry\.io\/issues\/7002\/events\/e2000000000000000000000000000002\//);
  assert.match(filed.body, /https:\/\/purlis-inc-ff\.sentry\.io\/issues\/7002\/events\/e3000000000000000000000000000003\//);
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

test("every page curl saved is read in order, the proxy's CONNECT answer before a page's own read past", () => {
  const events = readPages(join(fixtures, "pages"));
  assert.equal(events.length, 7);
  assert.deepEqual(
    events.map((event) => event.eventID.slice(0, 2)),
    ["e1", "e2", "e3", "e4", "e5", "e6", "e7"],
  );
});

test("the first page asks for the whole window of weeks, on the intake's EU host", () => {
  assert.equal(firstPage(4 * 7 * 24), "https://de.sentry.io/api/0/projects/purlis-inc-ff/purlis-app/events/?full=true&statsPeriod=672h");
  const run = spawnSync(process.execPath, [tool, "url", "--weeks", "1"], { encoding: "utf8" });
  assert.equal(run.status, 0);
  assert.equal(run.stdout, "https://de.sentry.io/api/0/projects/purlis-inc-ff/purlis-app/events/?full=true&statsPeriod=168h\n");
});

test("the next page is the Link header's next with results, and none after the last page", () => {
  assert.equal(
    nextPageOf(saved("pages/page-1.headers")),
    "https://de.sentry.io/api/0/projects/purlis-inc-ff/purlis-app/events/?full=true&statsPeriod=672h&cursor=0:100:0",
  );
  assert.equal(nextPageOf(saved("pages/page-2.headers")), null);
  assert.equal(nextPageOf("HTTP/1.1 200 OK\nLink: <https://de.sentry.io/b>; rel=\"next\"; results=\"true\"\n\n"), "https://de.sentry.io/b", "HTTP/1.1 spells it Link");
  const run = spawnSync(process.execPath, [tool, "next", join(fixtures, "pages/page-1.headers")], { encoding: "utf8" });
  assert.equal(run.status, 0);
  assert.match(run.stdout, /cursor=0:100:0\n$/);
  const last = spawnSync(process.execPath, [tool, "next", join(fixtures, "pages/page-2.headers")], { encoding: "utf8" });
  assert.deepEqual([last.status, last.stdout], [0, ""], "no address, so the fetch loop ends");
});

test("the first live answer, an empty page through the proxy, ends the loop and plans nothing new", () => {
  // Recorded on 2026-11-10 from org `purlis-inc-ff`, trimmed to the CONNECT block, the status and `link`.
  assert.equal(nextPageOf(saved("live-empty/page-1.headers")), null);
  const run = spawnSync(process.execPath, [tool, "next", join(fixtures, "live-empty/page-1.headers")], { encoding: "utf8" });
  assert.deepEqual([run.status, run.stdout], [0, ""]);
  const steps = plan(readPages(join(fixtures, "live-empty")).map(reportOf), issues);
  assert.deepEqual(steps, []);
  assert.match(summary(steps, [], { day: "2026-11-10" }), /No new reports since the last run\./);
});

test("a refused token says to run the wizard's token step again, at the next page and at the plan", () => {
  const refused = (error) => /HTTP 401/.test(error.message) && /#5687/.test(error.message);
  assert.throws(() => nextPageOf(saved("refused/page-1.headers")), refused);
  assert.throws(() => readPages(join(fixtures, "refused")), refused);
  const run = spawnSync(process.execPath, [tool, "next", join(fixtures, "refused/page-1.headers")], { encoding: "utf8" });
  assert.equal(run.status, 1);
  assert.equal(run.stdout, "", "no address, so curl is not run again with a refused token");
});

test("a next page on another host is refused, so the token never leaves for it", () => {
  assert.throws(
    () => nextPageOf('HTTP/2 200\nlink: <https://evil.example/x>; rel="next"; results="true"\n\n'),
    /refused to send the token to https:\/\/evil\.example/,
  );
});

test("the last page is the one whose next link has no results", () => {
  assert.equal(nextPage('<https://de.sentry.io/a>; rel="next"; results="false"; cursor="x"'), null);
  assert.equal(nextPage(null), null);
});

test("a plan from a folder no page was saved in says so rather than planning nothing", () => {
  const empty = mkdtempSync(join(tmpdir(), "report-intake-"));
  try {
    assert.throws(() => readPages(empty), /no fetched pages/);
  } finally {
    rmSync(empty, { recursive: true });
  }
});

test("a missing token says to run the Sentry setup wizard (#5687), exits 2 and reads nothing", () => {
  assert.equal(hasToken("intake-read-token\n"), true);
  assert.equal(hasToken("other-token\nintake-read-token-old\n"), false);
  const run = spawnSync(process.execPath, [tool, "has-token"], { encoding: "utf8", input: "other-token\n" });
  assert.equal(run.status, 2);
  assert.equal(run.stdout, "");
  assert.equal(run.stderr, `${MISSING_TOKEN}\n`);
  assert.match(run.stderr, /vault `sentry` has no `intake-read-token`/);
  assert.match(run.stderr, /Sentry setup wizard \(#5687\)/);
  const listed = spawnSync(process.execPath, [tool, "has-token"], { encoding: "utf8", input: "intake-read-token\n" });
  assert.deepEqual([listed.status, listed.stdout, listed.stderr], [0, "", ""]);
});

test("options are whole numbers above 0, plan reads saved pages, and only its six commands run", () => {
  assert.deepEqual(parseArgs(["plan", "--from", "d"]), { command: "plan", from: "d", sinceHours: 26, weeks: 4 });
  assert.deepEqual(parseArgs(["plan", "--from", "d", "--since-hours", "48", "--weeks", "8"]), { command: "plan", from: "d", sinceHours: 48, weeks: 8 });
  assert.deepEqual(parseArgs(["url", "--weeks", "2"]), { command: "url", sinceHours: 26, weeks: 2 });
  assert.deepEqual(parseArgs(["next", "h"]), { command: "next", headers: "h" });
  assert.deepEqual(parseArgs(["has-token"]), { command: "has-token" });
  assert.throws(() => parseArgs(["plan", "--from", "d", "--weeks", "0"]), /whole number/);
  assert.throws(() => parseArgs(["plan"]), /usage/, "a plan never fetches, so it wants the saved pages");
  assert.throws(() => parseArgs(["url", "--since-hours", "4"]), /unknown option/);
  assert.throws(() => parseArgs(["file"]), /usage/);
});

test("each event is listed by its Sentry short id where the intake's issues page names one", () => {
  const [filed] = byAction(plan(reports, issues), "file").filter((step) => step.title.includes("window failed"));
  assert.match(filed.body, /^- PURLIS-APP-2: https:\/\/purlis-inc-ff\.sentry\.io\/issues\/7002\/events\/e2/m);
  assert.deepEqual(filed.shortIds, ["PURLIS-APP-2"]);
  const [closed] = byAction(plan(reports, issues), "closed");
  assert.match(closed.comment, /^- https:\/\/purlis-inc-ff\.sentry\.io\/issues\/7006\//m, "no short id known: the link alone");
  assert.equal(readShortIds(join(fixtures, "refused")).size, 0, "no issues page saved: no short ids, nothing refused");
});

test("a duplicate raises the issue it is added to to priority:p0, taking off the lower priority it had", () => {
  const [added] = byAction(plan(reports, issues), "add");
  assert.deepEqual(added.priority, { add: "priority:p0", remove: ["priority:p2"] });
  const [closed] = byAction(plan(reports, issues), "closed");
  assert.deepEqual(closed.priority, { add: "priority:p0", remove: [] }, "no priority at all is lower too");
  const urgent = issues.map((issue) => ({ ...issue, labels: [{ name: "reported" }, { name: "priority:p0" }] }));
  assert.equal(byAction(plan(reports, urgent), "add")[0].priority, null, "already p0: nothing to change");
});

test("reports of one underlying bug merge into one issue that lists every Sentry short id and keeps every event's marker", () => {
  const steps = plan(reports, issues);
  const files = steps.map((step, i) => [step, i + 1]).filter(([step]) => step.action === "file");
  assert.equal(files.length, 2);
  const merged = merge(steps, files.map(([, n]) => n), { title: "One bug, reported twice" });
  assert.equal(merged.action, "file");
  assert.equal(merged.title, "One bug, reported twice");
  assert.deepEqual(merged.labels, ["reported", "priority:p0"]);
  assert.equal(merged.reports, 4);
  assert.deepEqual(merged.shortIds, ["PURLIS-APP-2", "PURLIS-APP-4", "PURLIS-APP-5"]);
  const named = markersIn(merged.body);
  assert.deepEqual([...named.sentryIssues].sort(), ["7002", "7004", "7005"]);
  assert.equal(named.events.size, 4);
  assert.ok(merged.texts.every((text) => !merged.body.includes(text)), "still never a report's words (ADR 0118)");
  const applied = [...issues, { number: 9200, title: merged.title, state: "OPEN", body: merged.body, comments: [], labels: [] }];
  assert.deepEqual(byAction(plan(reports, applied), "file"), [], "tomorrow's run knows every merged event");
  assert.throws(() => merge(steps, [files[0][1]]), /two or more/);
  const add = steps.findIndex((step) => step.action === "add") + 1;
  assert.throws(() => merge(steps, [files[0][1], add]), /step \d+ is not one to file/);
  assert.throws(() => merge(steps, [files[0][1], 99]), /no step 99/);
});

test("the milestone is the one holding the most open issues of the area, or none to say the closest is the steward's call", () => {
  const open = [
    { number: 1, milestone: { title: "0.6: Browser" } },
    { number: 2, milestone: { title: "0.5.2" } },
    { number: 3, milestone: { title: "0.6: Browser" } },
    { number: 4, milestone: null },
  ];
  assert.equal(milestoneFor(open), "0.6: Browser");
  assert.equal(milestoneFor([{ number: 4, milestone: null }]), null);
  assert.equal(milestoneFor([]), null);
  assert.equal(milestoneFor([{ milestone: { title: "B" } }, { milestone: { title: "A" } }]), "B", "a tie: the first listed, which the skill says to name");
});

test("the issues page is asked for on the intake's host, for the same window of weeks", () => {
  const run = spawnSync(process.execPath, [tool, "url", "--issues", "--weeks", "1"], { encoding: "utf8" });
  assert.equal(run.status, 0);
  assert.equal(run.stdout, "https://de.sentry.io/api/0/projects/purlis-inc-ff/purlis-app/issues/?statsPeriod=&query=lastSeen%3A-168h&limit=100\n");
  assert.deepEqual(parseArgs(["milestone", "--area", "area:browser"]), { command: "milestone", area: "area:browser" });
  assert.throws(() => parseArgs(["milestone", "--area", "browser"]), /area:/);
  assert.deepEqual(parseArgs(["merge", "--plan", "p.json", "--steps", "1,3", "--title", "T"]), { command: "merge", plan: "p.json", steps: [1, 3], title: "T" });
  assert.throws(() => parseArgs(["merge", "--plan", "p.json", "--steps", "1,x"]), /whole numbers/);
});
