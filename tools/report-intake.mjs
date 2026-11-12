// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The steward's daily Report intake routine (#5701, spec #5683, ADR 0118): its deterministic half.
//
// The Report intake is purlis's private Sentry EU project (org `purlis-inc-ff`, project `purlis-app`).
// Nothing in it is public until a maintainer, or the steward on their behalf, promotes it to a
// purlis/purlis issue labelled `reported`. This reads the intake's new events with a read-only
// token, groups them, decides for each group whether it is a new issue or more reports of one
// already filed, and renders the day's summary. It writes nothing anywhere: it prints a plan,
// and the steward applies it after triage (`docs/agents/skills/report-intake/SKILL.md`).
//
//   purlis secret list sentry | node tools/report-intake.mjs has-token
//   node tools/report-intake.mjs url [--issues] [--weeks 4]
//   node tools/report-intake.mjs next <headers-file>
//   node tools/report-intake.mjs plan --from <dir> [--since-hours 26] [--weeks 4]
//   node tools/report-intake.mjs merge --plan <plan.json> --steps <n,n,…> [--title <title>]
//   node tools/report-intake.mjs milestone --area <area:label>
//
// **It never sees the token.** purlis hands a vault's value to no program that runs what it is
// told, `node` among them (ADR 0112, D-PE-ruling-2), so curl fetches the pages:
// `purlis secret exec sentry --env SENTRY_INTAKE_TOKEN=intake-read-token -- curl … --variable
// %SENTRY_INTAKE_TOKEN --expand-header "Authorization: Bearer {{SENTRY_INTAKE_TOKEN}}" -D
// page-<n>.headers -o page-<n>.json <url>` (curl 8.3.0 or later), the skill holding the exact
// loop. This prints each address curl is given, and only on the intake's own host, so the token
// goes nowhere else; reads each saved page's status, saying to run the Sentry setup wizard's token
// step (#5687) where the token was refused; and plans from the saved pages. `has-token` reads
// `purlis secret list sentry` and, where the token is not there, says to run the wizard and exits
// 2, so the routine is a no-op until the token exists. At most `MAX_PAGES` pages are fetched.
//
// **No store of its own.** An event is handled once an issue or comment labelled `reported`
// carries its id in a marker this writes (`<!-- purlis-intake … -->`); "new" is an event of the
// last `--since-hours` that no marker names. A report the steward judged not worth an issue is
// left to age out of that window.
//
// **Grouping**: by the device's fingerprint where the event carries one (its `fingerprint` tag,
// which Sentry also groups by), else by Sentry's own issue. Words an agent added to a sent draft
// (`follows`) join the group of the event they follow. Tested on recorded API answers in
// `tools/report-intake.test.mjs` (`node --test tools/`).
//
// **Urgent, and in a milestone.** Every issue it files carries `priority:p0` beside `reported`,
// and a duplicate raises the issue it is added to to `priority:p0` (the operator, 2026-11-12).
// `milestone` answers the milestone holding the most open issues of an `area:*` label, by
// `docs/agents/issue-tracker.md`; the steward picks the area and passes `--milestone`. `merge`
// joins file steps the steward judged one bug into one issue, listing each Sentry short id
// (`PURLIS-APP-2`), which the intake's issues page (`url --issues`, saved as `issues-1.json`)
// names.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** Where the intake is: Sentry's EU region, its org and its project. */
export const INTAKE = {
  api: "https://de.sentry.io",
  ui: "https://purlis-inc-ff.sentry.io",
  org: "purlis-inc-ff",
  project: "purlis-app",
};

/** Where the read token lives, and the variable it is handed over in. */
export const TOKEN = { vault: "sentry", key: "intake-read-token", env: "SENTRY_INTAKE_TOKEN" };

export const LABEL = "reported";
/** The priority every issue the routine files or adds to gets, and the prefix of its peers. */
export const PRIORITY = "priority:p0";
const PRIORITY_PREFIX = "priority:";
export const LABELS = [LABEL, PRIORITY];
export const REPO = "purlis/purlis";

/** What the routine says when it has no token, and so does nothing. */
export const MISSING_TOKEN = [
  `No read token for the Report intake: vault \`${TOKEN.vault}\` has no \`${TOKEN.key}\`, so nothing was read and nothing was filed.`,
  `Run the Sentry setup wizard (#5687): it creates the EU project and stores the steward's read-only token there.`,
  `Then run this routine again.`,
].join("\n");

/** What it says when Sentry refuses the token. */
export const REFUSED_TOKEN = (status) =>
  `The Report intake refused the read token (HTTP ${status}). It may have been revoked or lack \`event:read\`: ` +
  `run the token step of the Sentry setup wizard (#5687) again to store a new one in vault \`${TOKEN.vault}\`.`;

/** An event's tags, as an object: Sentry's API answers them as `[{key, value}]`. */
export function tagsOf(event) {
  const tags = {};
  for (const tag of event.tags ?? []) tags[tag.key] = tag.value;
  return tags;
}

/** The fields of an event the routine reads, and nothing else. */
export function reportOf(event, shortIds = new Map()) {
  const tags = tagsOf(event);
  const release = tags.release ?? event.release?.version ?? "";
  return {
    id: event.eventID ?? event.id,
    sentryIssue: String(event.groupID),
    shortId: shortIds.get(String(event.groupID)) ?? null,
    at: event.dateCreated,
    title: event.title ?? "",
    // What it says: for the steward to read, never pasted into a public issue (ADR 0118).
    text: event.message ?? "",
    version: release.startsWith("purlis@") ? release.slice("purlis@".length) : release || "unknown",
    channel: tags.channel ?? tags.environment ?? "unknown",
    kind: tags.written_by === "agent" ? "draft" : "crash",
    reviewed: tags.reviewed_by_the_person === "true",
    fingerprint: tags.fingerprint ?? null,
    follows: tags.follows ?? null,
    repeats: event.context?.repeats ?? [],
    link: `${INTAKE.ui}/issues/${event.groupID}/events/${event.eventID ?? event.id}/`,
  };
}

/**
 * The reports grouped: one group per device fingerprint, else per Sentry issue; a follow-up
 * joins the group of the event it follows when that event is among `reports`. Oldest first.
 */
export function group(reports) {
  const byEvent = new Map(reports.map((report) => [report.id, report]));
  const keyOf = (report) => {
    const followed = report.follows && byEvent.get(report.follows);
    if (followed && followed !== report) return keyOf(followed);
    return report.fingerprint ? `fp:${report.fingerprint}` : `sentry:${report.sentryIssue}`;
  };
  const groups = new Map();
  for (const report of [...reports].sort((a, b) => a.at.localeCompare(b.at))) {
    const key = keyOf(report);
    let one = groups.get(key);
    if (!one) {
      one = { key, title: report.title, fingerprints: new Set(), sentryIssues: new Set(), reports: [] };
      groups.set(key, one);
    }
    if (report.fingerprint) one.fingerprints.add(report.fingerprint);
    one.sentryIssues.add(report.sentryIssue);
    one.reports.push(report);
  }
  return [...groups.values()];
}

/** The marker an issue or comment carries, naming the fingerprints, Sentry issues and events it covers. */
export function marker({ fingerprints = [], sentryIssues = [], events = [] }) {
  const part = (name, values) => {
    const list = [...values];
    return list.length ? ` ${name}=${list.join(",")}` : "";
  };
  return `<!-- purlis-intake${part("fingerprints", fingerprints)}${part("sentry-issues", sentryIssues)}${part("events", events)} -->`;
}

/** What the markers in `text` name. */
export function markersIn(text) {
  const named = { fingerprints: new Set(), sentryIssues: new Set(), events: new Set() };
  const names = { fingerprints: "fingerprints", "sentry-issues": "sentryIssues", events: "events" };
  for (const [, body] of (text ?? "").matchAll(/<!-- purlis-intake([^>]*)-->/g)) {
    for (const [, name, values] of body.matchAll(/([a-z-]+)=([^\s]+)/g)) {
      const into = names[name];
      if (into) for (const value of values.split(",")) if (value) named[into].add(value);
    }
  }
  return named;
}

/**
 * The `reported` issues (`gh issue list --json number,title,state,stateReason,url,body,comments`),
 * each with what its body's and its comments' markers name.
 */
export function knownIssues(issues) {
  return issues.map((issue) => {
    const text = [issue.body, ...(issue.comments ?? []).map((comment) => comment.body)].join("\n");
    const labels = (issue.labels ?? []).map((label) => label.name);
    return { number: issue.number, title: issue.title, state: issue.state, url: issue.url, labels, ...markersIn(text) };
  });
}

/** What raises `issue` to {@link PRIORITY}: the label to add and the lower ones to take off, or null when it has it. */
export function raiseOf(issue) {
  if (issue.labels.includes(PRIORITY)) return null;
  return { add: PRIORITY, remove: issue.labels.filter((label) => label.startsWith(PRIORITY_PREFIX)) };
}

/** The issue that already covers `one`, by a fingerprint first and a Sentry issue second, or null. */
export function matchOf(one, known) {
  const by = (field, values) => known.find((issue) => [...values].some((value) => issue[field].has(value)));
  return by("fingerprints", one.fingerprints) ?? by("sentryIssues", one.sentryIssues) ?? null;
}

/** What `one`'s reports say, each once: the steward reads them; no issue or comment quotes them. */
const textsOf = (one) => [...new Set(one.reports.map((r) => r.text).filter(Boolean))];

/** The Sentry short ids of `reports`, each once, in order. */
const shortIdsOf = (reports) => [...new Set(reports.map((r) => r.shortId).filter(Boolean))];

/** One event's line: its short id where known, and its link. */
const eventLine = (r) => `- ${r.shortId ? `${r.shortId}: ` : ""}${r.link}`;

const versionsOf = (reports) => [...new Set(reports.map((r) => `${r.version} (${r.channel})`))].join(", ");

/** The title of a new issue for `one`. */
export function issueTitle(one) {
  const title = one.title.replace(/\s+/g, " ").trim() || "A report from the Report intake";
  return title.length > 120 ? `${title.slice(0, 117)}…` : title;
}

/** The body of a new issue for `one`: what it is, where it came from, and the links back. */
export function issueBody(one) {
  const kinds = new Set(one.reports.map((r) => r.kind));
  const what = kinds.has("crash") ? "Crash report" : "Report draft";
  const reviewed = one.reports.some((r) => r.kind === "draft" && r.reviewed);
  return [
    `Promoted from the Report intake by the steward's daily routine (#5701, ADR 0118).`,
    ``,
    `- **What:** ${what}${kinds.size > 1 ? "s and drafts" : ""}${kinds.has("draft") ? (reviewed ? ", read by the person before sending" : ", written by an agent and sent unread") : ""}`,
    `- **Reports:** ${one.reports.length}`,
    `- **Versions:** ${versionsOf(one.reports)}`,
    `- **First seen:** ${one.reports[0].at}`,
    ``,
    `Events (private to purlis's maintainers):`,
    ...one.reports.map(eventLine),
    ``,
    marker({ fingerprints: one.fingerprints, sentryIssues: one.sentryIssues, events: one.reports.map((r) => r.id) }),
  ].join("\n");
}

/** The comment that adds `one`'s reports to an issue already filed. */
export function commentBody(one, issue) {
  return [
    `${one.reports.length} more report${one.reports.length === 1 ? "" : "s"} from the Report intake, from ${versionsOf(one.reports)}${issue.state === "CLOSED" ? ", after this issue was closed" : ""}:`,
    ...one.reports.map(eventLine),
    ``,
    marker({ fingerprints: one.fingerprints, sentryIssues: one.sentryIssues, events: one.reports.map((r) => r.id) }),
  ].join("\n");
}

/** The step that files `one` as a new issue, under `title`. Its `group` is what {@link merge} joins. */
function fileStep(one, title = issueTitle(one)) {
  return {
    action: "file",
    title,
    body: issueBody(one),
    labels: LABELS,
    reports: one.reports.length,
    versions: versionsOf(one.reports),
    shortIds: shortIdsOf(one.reports),
    texts: textsOf(one),
    group: {
      title: one.title,
      fingerprints: [...one.fingerprints],
      sentryIssues: [...one.sentryIssues],
      reports: one.reports,
    },
  };
}

/**
 * One file step for the plan's steps `numbers` (1 up, as the steward counts them), which the
 * steward judged the same underlying bug: their reports, oldest first, under `title`.
 */
export function merge(steps, numbers, { title } = {}) {
  if (new Set(numbers).size < 2) throw new Error("merge wants two or more distinct steps");
  const groups = numbers.map((n) => {
    const step = steps[n - 1];
    if (!step) throw new Error(`no step ${n}: the plan has ${steps.length}`);
    if (step.action !== "file" || !step.group) throw new Error(`step ${n} is not one to file: add its reports to issue #${step.issue} instead`);
    return step.group;
  });
  const one = {
    title: groups[0].title,
    fingerprints: new Set(groups.flatMap((g) => g.fingerprints)),
    sentryIssues: new Set(groups.flatMap((g) => g.sentryIssues)),
    reports: groups.flatMap((g) => g.reports).sort((a, b) => a.at.localeCompare(b.at)),
  };
  return fileStep(one, title ?? issueTitle(one));
}

/**
 * The milestone holding the most of `issues` (`gh issue list --label <area> --state open --json
 * milestone`), the first listed on a tie, or null when none has one: then the closest existing
 * milestone is the steward's call (`docs/agents/issue-tracker.md`).
 */
export function milestoneFor(issues) {
  const counts = new Map();
  for (const issue of issues) {
    const title = issue.milestone?.title;
    if (title) counts.set(title, (counts.get(title) ?? 0) + 1);
  }
  let best = null;
  for (const [title, count] of counts) if (!best || count > best.count) best = { title, count };
  return best?.title ?? null;
}

/**
 * The day's plan: each group of new reports with what to do with it. `file` is a new issue
 * labelled `reported`; `add` is a 👍 and a comment on an open issue that covers it; `closed` is
 * the same on a closed one, which the steward judges (a regression reopens it). Reports whose
 * events a marker already names are not new, and are left out.
 */
export function plan(reports, issues) {
  const known = knownIssues(issues);
  const handled = new Set(known.flatMap((issue) => [...issue.events]));
  const fresh = reports.filter((report) => !handled.has(report.id));
  return group(fresh).map((one) => {
    const issue = matchOf(one, known);
    if (!issue) return fileStep(one);
    return {
      action: issue.state === "CLOSED" ? "closed" : "add",
      issue: issue.number,
      reaction: "+1",
      priority: raiseOf(issue),
      comment: commentBody(one, issue),
      reports: one.reports.length,
      versions: versionsOf(one.reports),
      shortIds: shortIdsOf(one.reports),
      texts: textsOf(one),
    };
  });
}

/** The ISO week `at` falls in, as `2026-W45`. */
export function isoWeek(at) {
  // The week's Thursday decides its year; its week is how many weeks that Thursday is into it.
  const thursday = new Date(`${at.slice(0, 10)}T00:00:00Z`);
  thursday.setUTCDate(thursday.getUTCDate() + 3 - ((thursday.getUTCDay() + 6) % 7));
  const year = thursday.getUTCFullYear();
  const week = 1 + Math.floor((thursday - Date.UTC(year, 0, 1)) / (7 * 86_400_000));
  return `${year}-W${String(week).padStart(2, "0")}`;
}

/**
 * Reports per week by version and channel, and how many of them are distinct (groups), newest
 * week first. Repeats a device counted and sent as a number ride in `repeats`.
 */
export function weekly(reports) {
  const rows = new Map();
  const groupOf = new Map();
  for (const one of group(reports)) for (const report of one.reports) groupOf.set(report.id, one.key);
  for (const report of reports) {
    const key = `${isoWeek(report.at)}\0${report.version}\0${report.channel}`;
    let row = rows.get(key);
    if (!row) {
      row = { week: isoWeek(report.at), version: report.version, channel: report.channel, reports: 0, repeats: 0, groups: new Set() };
      rows.set(key, row);
    }
    row.reports += 1;
    row.repeats += report.repeats.reduce((sum, repeat) => sum + (Number(repeat.count) || 0), 0);
    row.groups.add(groupOf.get(report.id));
  }
  return [...rows.values()]
    .map(({ groups, ...row }) => ({ ...row, distinct: groups.size }))
    .sort((a, b) => b.week.localeCompare(a.week) || b.reports - a.reports || a.version.localeCompare(b.version));
}

/** The day's summary, in Markdown: what the plan does, then the weekly table. */
export function summary(steps, recent, { day }) {
  const count = (action) => steps.filter((step) => step.action === action).length;
  const reports = steps.reduce((sum, step) => sum + step.reports, 0);
  const lines = [`## Report intake, ${day}`, ``];
  if (steps.length === 0) {
    lines.push(`No new reports since the last run.`);
  } else {
    lines.push(
      `${reports} new report${reports === 1 ? "" : "s"} in ${steps.length} distinct group${steps.length === 1 ? "" : "s"}: ` +
        `${count("file")} to file, ${count("add")} added to open issues, ${count("closed")} on closed issues.`,
    );
  }
  const rows = weekly(recent);
  lines.push(``, `### Reports per week`, ``);
  if (rows.length === 0) {
    lines.push(`No reports in the window.`);
  } else {
    lines.push(`| Week | Version | Channel | Reports | Distinct | Repeats counted on devices |`, `|---|---|---|---:|---:|---:|`);
    for (const row of rows) {
      lines.push(`| ${row.week} | ${row.version} | ${row.channel} | ${row.reports} | ${row.distinct} | ${row.repeats} |`);
    }
  }
  return lines.join("\n");
}

/** The next page's address from a Sentry `Link` header, or null when there are no more results. */
export function nextPage(link) {
  for (const part of (link ?? "").split(",")) {
    if (/rel="next"/.test(part) && /results="true"/.test(part)) return part.match(/<([^>]+)>/)?.[1] ?? null;
  }
  return null;
}

/** The most pages fetched in one run: a flood stops here rather than running the routine for an hour. */
export const MAX_PAGES = 20;

/** `url` when it is on the intake's own host, the only one the token is sent to; else it throws. */
export function onIntakeHost(url) {
  const origin = new URL(url).origin;
  if (origin !== INTAKE.api) throw new Error(`refused to send the token to ${origin}`);
  return url;
}

/** The first page of the project's events of the last `hours`. */
export function firstPage(hours) {
  return `${INTAKE.api}/api/0/projects/${INTAKE.org}/${INTAKE.project}/events/?full=true&statsPeriod=${hours}h`;
}

/**
 * The intake's issues of the last `hours`, for their short ids. One page of 100: an event whose
 * issue is not on it is listed by its link alone.
 */
export function issuesPage(hours) {
  return `${INTAKE.api}/api/0/projects/${INTAKE.org}/${INTAKE.project}/issues/?statsPeriod=&query=lastSeen%3A-${hours}h&limit=100`;
}

/**
 * A response's status and `Link` header from what `curl -D` saved. curl saves every response it
 * read (a proxy's `CONNECT` answer first), so the last one is the server's.
 */
export function responseOf(headers) {
  const blocks = headers.replace(/\r/g, "").split(/\n(?=HTTP\/)/);
  const last = blocks[blocks.length - 1];
  const status = Number(last.match(/^HTTP\/[\d.]+ (\d{3})/)?.[1]);
  if (!status) throw new Error("no HTTP status in the saved headers: was the page fetched?");
  const link = last.match(/^link:[ \t]*(.*)$/im)?.[1] ?? null;
  return { status, link };
}

/** Throws what to tell the steward when the intake did not answer a page with its events. */
function checkStatus(status) {
  if (status === 401 || status === 403) throw new Error(REFUSED_TOKEN(status));
  if (status < 200 || status > 299) throw new Error(`The Report intake answered HTTP ${status} for its events.`);
}

/**
 * The next page to fetch after the one whose saved headers are `headers`, on the intake's host
 * only, or null when that page was the last. A refused token throws {@link REFUSED_TOKEN}.
 */
export function nextPageOf(headers) {
  const { status, link } = responseOf(headers);
  checkStatus(status);
  const next = nextPage(link);
  return next && onIntakeHost(next);
}

/**
 * The events of the pages the steward saved in `dir`: `page-<n>.headers` (curl's `-D`) and
 * `page-<n>.json` (its `-o`), from 1 up, in order. A refused token throws {@link REFUSED_TOKEN}.
 */
export function readPages(dir) {
  if (!existsSync(join(dir, "page-1.headers"))) throw new Error(`no fetched pages in ${dir}: page-1.headers is missing`);
  const events = [];
  for (let n = 1; existsSync(join(dir, `page-${n}.headers`)); n += 1) {
    checkStatus(responseOf(readFileSync(join(dir, `page-${n}.headers`), "utf8")).status);
    events.push(...JSON.parse(readFileSync(join(dir, `page-${n}.json`), "utf8")));
  }
  return events;
}

/**
 * Each Sentry issue's short id, by its id, from the `issues-1.json` saved in `dir` beside its
 * `issues-1.headers`. Empty where it was not saved; a refused token throws {@link REFUSED_TOKEN}.
 */
export function readShortIds(dir) {
  const ids = new Map();
  if (!existsSync(join(dir, "issues-1.headers"))) return ids;
  checkStatus(responseOf(readFileSync(join(dir, "issues-1.headers"), "utf8")).status);
  for (const issue of JSON.parse(readFileSync(join(dir, "issues-1.json"), "utf8"))) {
    if (issue.shortId) ids.set(String(issue.id), issue.shortId);
  }
  return ids;
}

/** Whether `purlis secret list <vault>`'s answer, a key a line, lists the read token. */
export function hasToken(listed) {
  return listed.split(/\r?\n/).some((line) => line.trim() === TOKEN.key);
}

const USAGE = [
  "usage: node tools/report-intake.mjs has-token < <purlis secret list sentry>",
  "       node tools/report-intake.mjs url [--issues] [--weeks 4]",
  "       node tools/report-intake.mjs next <headers-file>",
  "       node tools/report-intake.mjs plan --from <dir> [--since-hours 26] [--weeks 4]",
  "       node tools/report-intake.mjs merge --plan <plan.json> --steps <n,n,…> [--title <title>]",
  "       node tools/report-intake.mjs milestone --area <area:label>",
].join("\n");

/** `argv` as options: `--since-hours` (default 26) and `--weeks` (default 4), both whole and positive, and `--from`. */
export function parseArgs(argv) {
  const [command, ...rest] = argv;
  const options = { command, sinceHours: 26, weeks: 4 };
  if (command === "next") {
    if (rest.length !== 1) throw new Error(USAGE);
    return { command, headers: rest[0] };
  }
  if (command === "has-token" && rest.length === 0) return { command };
  if (command === "milestone") {
    if (rest.length !== 2 || rest[0] !== "--area") throw new Error(USAGE);
    if (!/^area:[a-z0-9-]+$/.test(rest[1])) throw new Error(`--area wants an area:* label, not ${rest[1]}`);
    return { command, area: rest[1] };
  }
  if (command === "merge") {
    const merged = { command };
    for (let i = 0; i < rest.length; i += 2) {
      const [flag, value] = [rest[i], rest[i + 1]];
      if (!value) throw new Error(USAGE);
      if (flag === "--plan") merged.plan = value;
      else if (flag === "--title") merged.title = value;
      else if (flag === "--steps") {
        if (!/^\d+(,\d+)*$/.test(value)) throw new Error("--steps wants whole numbers joined by commas, as 1,3");
        merged.steps = value.split(",").map(Number);
      } else throw new Error(`unknown option ${flag}`);
    }
    if (!merged.plan || !merged.steps) throw new Error(USAGE);
    return merged;
  }
  if (command !== "plan" && command !== "url") throw new Error(USAGE);
  for (let i = 0; i < rest.length; i += 2) {
    if (rest[i] === "--issues" && command === "url") {
      options.issues = true;
      i -= 1;
      continue;
    }
    if (rest[i] === "--from" && command === "plan") {
      if (!rest[i + 1]) throw new Error("--from wants the folder the pages were saved in");
      options.from = rest[i + 1];
      continue;
    }
    const value = Number(rest[i + 1]);
    if (rest[i] === "--weeks" || (rest[i] === "--since-hours" && command === "plan")) {
      if (!Number.isInteger(value) || value <= 0) throw new Error(`${rest[i]} wants a whole number above 0`);
      options[rest[i] === "--weeks" ? "weeks" : "sinceHours"] = value;
    } else {
      throw new Error(`unknown option ${rest[i]}`);
    }
  }
  if (command === "plan" && !options.from) throw new Error(USAGE);
  return options;
}

/** Issues on purlis/purlis with `label` in `state`, as `gh issue list --json <fields>` answers. */
function issuesLabelled(label, state, fields) {
  const out = execFileSync(
    "gh",
    ["issue", "list", "-R", REPO, "--label", label, "--state", state, "--limit", "1000", "--json", fields],
    { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  return JSON.parse(out);
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.command === "has-token") {
    if (hasToken(readFileSync(0, "utf8"))) return;
    process.stderr.write(`${MISSING_TOKEN}\n`);
    process.exit(2);
  }
  if (options.command === "url") {
    const hours = options.weeks * 7 * 24;
    process.stdout.write(`${options.issues ? issuesPage(hours) : firstPage(hours)}\n`);
    return;
  }
  if (options.command === "merge") {
    const { steps } = JSON.parse(readFileSync(options.plan, "utf8"));
    process.stdout.write(`${JSON.stringify(merge(steps, options.steps, { title: options.title }), null, 2)}\n`);
    return;
  }
  if (options.command === "milestone") {
    const milestone = milestoneFor(issuesLabelled(options.area, "open", "number,milestone"));
    if (milestone) process.stdout.write(`${milestone}\n`);
    return;
  }
  if (options.command === "next") {
    const next = nextPageOf(readFileSync(options.headers, "utf8"));
    if (next) process.stdout.write(`${next}\n`);
    return;
  }
  const now = new Date();
  const shortIds = readShortIds(options.from);
  const recent = readPages(options.from).map((event) => reportOf(event, shortIds));
  const since = now.getTime() - options.sinceHours * 3_600_000;
  const fresh = recent.filter((report) => Date.parse(report.at) >= since);
  const steps = plan(fresh, issuesLabelled(LABEL, "all", "number,title,state,url,body,comments,labels"));
  process.stdout.write(`${JSON.stringify({ steps, summary: summary(steps, recent, { day: now.toISOString().slice(0, 10) }) }, null, 2)}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}
