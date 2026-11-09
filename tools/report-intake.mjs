// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The steward's daily Report intake routine (#5701, spec #5683, ADR 0118): its deterministic half.
//
// The Report intake is purlis's private Sentry EU project (org `purlis`, project `purlis-app`).
// Nothing in it is public until a maintainer, or the steward on their behalf, promotes it to a
// purlis/purlis issue labelled `reported`. This reads the intake's new events with a read-only
// token, groups them, decides for each group whether it is a new issue or more reports of one
// already filed, and renders the day's summary. It writes nothing anywhere: it prints a plan,
// and the steward applies it after triage (`docs/agents/skills/report-intake/SKILL.md`).
//
//   node tools/report-intake.mjs plan [--since-hours 26] [--weeks 4]
//
// **The token** is `SENTRY_INTAKE_TOKEN`, put there by
// `purlis secret exec sentry --env SENTRY_INTAKE_TOKEN=intake-read-token -- …`. It is never
// printed, never written, and sent only to the intake's own host. Without it, this says to run
// the Sentry setup wizard (#5687) and exits 2, so the routine is a no-op until the token exists.
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

import { execFileSync } from "node:child_process";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** Where the intake is: Sentry's EU region, its org and its project. */
export const INTAKE = {
  api: "https://de.sentry.io",
  ui: "https://purlis.sentry.io",
  org: "purlis",
  project: "purlis-app",
};

/** Where the read token lives, and the variable it is handed over in. */
export const TOKEN = { vault: "sentry", key: "intake-read-token", env: "SENTRY_INTAKE_TOKEN" };

export const LABEL = "reported";
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
export function reportOf(event) {
  const tags = tagsOf(event);
  const release = tags.release ?? event.release?.version ?? "";
  return {
    id: event.eventID ?? event.id,
    sentryIssue: String(event.groupID),
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
    return { number: issue.number, title: issue.title, state: issue.state, url: issue.url, ...markersIn(text) };
  });
}

/** The issue that already covers `one`, by a fingerprint first and a Sentry issue second, or null. */
export function matchOf(one, known) {
  const by = (field, values) => known.find((issue) => [...values].some((value) => issue[field].has(value)));
  return by("fingerprints", one.fingerprints) ?? by("sentryIssues", one.sentryIssues) ?? null;
}

/** What `one`'s reports say, each once: the steward reads them; no issue or comment quotes them. */
const textsOf = (one) => [...new Set(one.reports.map((r) => r.text).filter(Boolean))];

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
    ...one.reports.map((r) => `- ${r.link}`),
    ``,
    marker({ fingerprints: one.fingerprints, sentryIssues: one.sentryIssues, events: one.reports.map((r) => r.id) }),
  ].join("\n");
}

/** The comment that adds `one`'s reports to an issue already filed. */
export function commentBody(one, issue) {
  return [
    `${one.reports.length} more report${one.reports.length === 1 ? "" : "s"} from the Report intake, from ${versionsOf(one.reports)}${issue.state === "CLOSED" ? ", after this issue was closed" : ""}:`,
    ...one.reports.map((r) => `- ${r.link}`),
    ``,
    marker({ fingerprints: one.fingerprints, sentryIssues: one.sentryIssues, events: one.reports.map((r) => r.id) }),
  ].join("\n");
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
    const versions = versionsOf(one.reports);
    if (!issue) {
      return { action: "file", title: issueTitle(one), body: issueBody(one), label: LABEL, reports: one.reports.length, versions, texts: textsOf(one) };
    }
    return {
      action: issue.state === "CLOSED" ? "closed" : "add",
      issue: issue.number,
      reaction: "+1",
      comment: commentBody(one, issue),
      reports: one.reports.length,
      versions,
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

/** The most pages read in one run: a flood stops here rather than running the routine for an hour. */
export const MAX_PAGES = 20;

/**
 * The project's events of the last `hours`, every page, through `fetcher` (a `fetch`). The token
 * goes only to the intake's own host. A refused token throws {@link REFUSED_TOKEN}.
 */
export async function fetchEvents({ token, hours, fetcher = fetch }) {
  let url = `${INTAKE.api}/api/0/projects/${INTAKE.org}/${INTAKE.project}/events/?full=true&statsPeriod=${hours}h`;
  const events = [];
  for (let page = 0; url && page < MAX_PAGES; page += 1) {
    if (new URL(url).origin !== INTAKE.api) throw new Error(`refused to send the token to ${new URL(url).origin}`);
    const response = await fetcher(url, { headers: { Authorization: `Bearer ${token}` } });
    if (response.status === 401 || response.status === 403) throw new Error(REFUSED_TOKEN(response.status));
    if (!response.ok) throw new Error(`The Report intake answered HTTP ${response.status} for its events.`);
    events.push(...(await response.json()));
    url = nextPage(response.headers.get("link"));
  }
  return events;
}

/** `argv` as options: `--since-hours` (default 26) and `--weeks` (default 4), both whole and positive. */
export function parseArgs(argv) {
  const options = { command: argv[0], sinceHours: 26, weeks: 4 };
  for (let i = 1; i < argv.length; i += 2) {
    const value = Number(argv[i + 1]);
    if (!Number.isInteger(value) || value <= 0) throw new Error(`${argv[i]} wants a whole number above 0`);
    if (argv[i] === "--since-hours") options.sinceHours = value;
    else if (argv[i] === "--weeks") options.weeks = value;
    else throw new Error(`unknown option ${argv[i]}`);
  }
  if (options.command !== "plan") throw new Error("usage: node tools/report-intake.mjs plan [--since-hours 26] [--weeks 4]");
  return options;
}

/** The `reported` issues on purlis/purlis, with their comments, through `gh`. */
function reportedIssues() {
  const out = execFileSync(
    "gh",
    ["issue", "list", "-R", REPO, "--label", LABEL, "--state", "all", "--limit", "1000", "--json", "number,title,state,url,body,comments"],
    { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  return JSON.parse(out);
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const token = process.env[TOKEN.env];
  if (!token) {
    process.stderr.write(`${MISSING_TOKEN}\n`);
    process.exit(2);
  }
  const now = new Date();
  const recent = (await fetchEvents({ token, hours: options.weeks * 7 * 24 })).map(reportOf);
  const since = now.getTime() - options.sinceHours * 3_600_000;
  const fresh = recent.filter((report) => Date.parse(report.at) >= since);
  const steps = plan(fresh, reportedIssues());
  process.stdout.write(`${JSON.stringify({ steps, summary: summary(steps, recent, { day: now.toISOString().slice(0, 10) }) }, null, 2)}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  });
}
