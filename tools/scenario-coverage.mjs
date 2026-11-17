#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The matrix coverage list, generated once and then owned** (M74.1, T1.1-08, #6855).
//
// `tests/scenarios/coverage.json` names a cover for every row × dialect × OS of the behaviour
// matrix (test-matrix-phase-1.md §3.22, with T1.1-43's `tests/scenarios/rows.json`), every row
// §3.20 leaves to fixtures, each dialect's real-binary network and file probes, and every escape
// × protection of `tests/scenarios/escapes.json`. `cargo test -p purlis-scenarios --test
// coverage` is the gate over it.
//
// - `node tools/scenario-coverage.mjs generate <test-matrix-phase-1.md>` writes the list from the
//   matrix, every cover `pending: <the ticket that ports it>`, and each cap's seed from the count
//   ticket 43's cover-kind column gives. Run once; after that the list is edited, not generated.
// - `node tools/scenario-coverage.mjs diff <test-matrix-phase-1.md>` is the orchestrator's check
//   at every re-carry (tickets 32, 42, 43, 52) and at the gate: the matrix's row ids and the
//   cover kind it carries for each, against the list. It exits 1 on any difference. The rows'
//   dialect and OS applicability and their Log/Window/Chat flags are `rows.json`'s, which the
//   gate's `the_list_matches_the_matrix_s_row_table` holds the list to.
//
// - `node tools/scenario-coverage.mjs issues` refreshes `tests/scenarios/issues.json`, the
//   cache of every issue a cell's `expected_fail` names and whether it is open, from GitHub
//   (`gh`). A pull request's gate reads the cache; the nightly refreshes it first, so a mark on
//   an issue closed since fails there.
//
// The matrix lives in the plane (`work/who-judges/`), not in this repo, so its path is given.

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const SCENARIOS = join(dirname(fileURLToPath(import.meta.url)), "..", "tests", "scenarios");
const DIALECTS = ["claude", "codex", "opencode"];
const OSES = ["linux", "macos"];

/** The suite ticket that ports each §3 section (the tickets' titles, tickets/phase-1-proven). */
export const SECTION_TICKET = {
  "3.0": 11, "3.1": 11, "3.2": 12, "3.3": 13, "3.4": 13, "3.5": 14, "3.6": 15, "3.7": 15,
  "3.8": 16, "3.9": 17, "3.10": 17, "3.11": 49, "3.12": 49, "3.13": 49, "3.14": 18, "3.15": 18,
  "3.16": 18, "3.17": 48, "3.18": 48, "3.19": 48, "3.20": 19,
};

/** §3.21's volume rows land with the fixes whose bounded rule they test: 28 (the proxy), 32 (files and dials). */
const VOLUME_TICKET = { "M-VOL-02": 28 };

/**
 * §3.20's "why not live", as the closed list of fixture reasons (T1.1-08): a fault, a race,
 * time, a third-party service §3.20 names, machine-wide state, an OS account or privilege, an
 * internal state or a unit-only R-row, or no inert probe.
 */
export const FIXTURE_REASON = {
  "M-WAY-04": "race", "M-LOOP-11": "third_party_service", "M-HC-02": "third_party_service",
  "M-HC-07": "internal_state", "M-ASK-03": "third_party_service", "M-ASK-12": "third_party_service",
  "M-GRD-08": "no_inert_probe", "M-GRD-10": "no_inert_probe", "M-ATT-01": "internal_state",
  "M-ATT-02": "internal_state", "M-ATT-03": "internal_state", "M-ATT-04": "internal_state",
  "M-ATT-07": "internal_state", "M-ATT-08": "no_inert_probe", "M-RST-05": "internal_state",
  "M-HOLD-11": "third_party_service", "M-HOLD-14": "fault", "M-BH-02": "fault", "M-BH-07": "fault",
  "M-BH-08b": "race", "M-OBS-07": "third_party_service", "M-OBS-11": "time",
  "M-SET-05": "third_party_service",
};

/** Ticket 43's cover-kind column (§3.22), as a kind, the binaries it needs and a reason. */
export function carried(cover) {
  if (cover.startsWith("e2e:")) return { kind: "e2e", reason: "window" };
  if (cover.startsWith("live:")) return { kind: "live", reason: "production_os_check" };
  if (cover.startsWith("scenario on ticket 10's leg")) return { kind: "scenario", binary: "real" };
  if (cover.startsWith("scenario, and ticket 53's real-binary leg")) return { kind: "scenario", binary: "both" };
  if (cover.startsWith("scenario")) return { kind: "scenario" };
  throw new Error(`a cover kind the generator does not know: ${cover}`);
}

/** Each §3 table row's id and section, §3.22's cover column, and §3.20's fixture rows. */
export function readMatrix(text) {
  const section = {};
  const cover = {};
  const fixtures = [];
  let at = null;
  for (const line of text.split("\n")) {
    const heading = line.match(/^### (3\.\d+) /);
    if (heading) {
      at = heading[1];
      continue;
    }
    if (line.startsWith("## ") && !line.startsWith("## 3.")) at = null;
    if (!at || !line.startsWith("| ")) continue;
    const cells = line.split(" | ");
    const id = cells[0].slice(2).trim();
    if (at === "3.22") {
      if (/^M-[A-Z0-9]+-\d/.test(id)) cover[id] = cells[5].trim();
      continue;
    }
    if (at === "3.20") {
      const range = id.match(/^(M-[A-Z0-9]+-)(\d+)…(\d+)$/);
      if (range) {
        for (let n = Number(range[2]); n <= Number(range[3]); n++) {
          fixtures.push(`${range[1]}${String(n).padStart(2, "0")}`);
        }
      } else if (/^M-[A-Z0-9]+-\d/.test(id)) {
        fixtures.push(id);
      } else if (id === "—") {
        fixtures.push(...cells[1].split(",").map((r) => r.trim()));
      }
      continue;
    }
    if (/^M-[A-Z0-9]+-\d/.test(id) && !(id in section)) section[id] = at;
  }
  return { section, cover, fixtures };
}

function ticketOf(row, section) {
  if (row in VOLUME_TICKET) return VOLUME_TICKET[row];
  if (section[row] === "3.21") return 32;
  const ticket = SECTION_TICKET[section[row]];
  if (!ticket) throw new Error(`${row}: no suite ticket for §${section[row]}`);
  return ticket;
}

function readJson(name) {
  return JSON.parse(readFileSync(join(SCENARIOS, name), "utf8"));
}

export function generate(matrix, rows, escapes) {
  const { section, cover, fixtures } = readMatrix(matrix);
  const list = { rows: [], fixtures: [], real_binary: [], escapes: [] };
  for (const r of rows) {
    const text = cover[r.row];
    if (!text) throw new Error(`${r.row} is in rows.json but has no cover in §3.22`);
    const c = carried(text);
    const entry = { row: r.row, dialect: r.dialect, os: r.os, log: r.log, window: r.window, chat: r.chat, carried: c.kind, kind: c.kind };
    if (c.reason) entry.reason = c.reason;
    if (c.binary) entry.binary = c.binary;
    entry.cover = `pending: ${c.kind === "live" ? 37 : ticketOf(r.row, section)}`;
    list.rows.push(entry);
  }
  for (const row of fixtures) {
    const reason = row.startsWith("R-") ? "internal_state" : FIXTURE_REASON[row];
    if (!reason) throw new Error(`${row}: no fixture reason`);
    list.fixtures.push({ row, os: "linux", reason, cover: "pending: 19" });
  }
  for (const dialect of DIALECTS) {
    for (const os of OSES) {
      for (const probe of ["network", "file"]) {
        for (const protection of ["full", "basic"]) {
          const ticket = dialect === "claude" && os === "macos" ? 10 : 53;
          list.real_binary.push({ probe, dialect, os, protection, cover: `pending: ${ticket}` });
        }
      }
    }
  }
  const protection = { Full: "full", Basic: "basic", off: "none" };
  for (const e of escapes.rows) {
    const covers = {};
    for (const [column, name] of Object.entries(protection)) {
      if (!e[column].startsWith("n/a")) covers[name] = `pending: ${e.ticket}`;
    }
    list.escapes.push({ id: e.id, covers, control: `pending: ${e.ticket}` });
  }
  return { list, seeds: seeds(list) };
}

/** Each reason's seed: how many rows ticket 43's column gives it. */
function seeds(list) {
  const rows = { live: {}, fixture: {}, e2e: {} };
  const add = (kind, reason, row) => (rows[kind][reason] ??= new Set()).add(row);
  for (const r of list.rows) if (r.reason) add(r.kind, r.reason, r.row);
  for (const f of list.fixtures) add("fixture", f.reason, f.row);
  const caps = {};
  for (const [kind, reasons] of Object.entries(rows)) {
    caps[kind] = {};
    for (const [reason, set] of Object.entries(reasons).sort()) {
      caps[kind][reason] = { seed: set.size, cap: set.size };
    }
  }
  return caps;
}

/** JSON with one entry of each list on a line, as rows.json is written, so a diff reads by row. */
export function format(doc) {
  const out = ["{"];
  const keys = Object.keys(doc);
  keys.forEach((key, i) => {
    const comma = i < keys.length - 1 ? "," : "";
    const value = doc[key];
    if (Array.isArray(value) && value.length > 0) {
      out.push(`  ${JSON.stringify(key)}: [`);
      value.forEach((v, j) => out.push(`    ${JSON.stringify(v)}${j < value.length - 1 ? "," : ""}`));
      out.push(`  ]${comma}`);
    } else if (key === "caps") {
      out.push(`  "caps": {`);
      const kinds = Object.keys(value);
      kinds.forEach((kind, k) => {
        out.push(`    ${JSON.stringify(kind)}: {`);
        const reasons = Object.keys(value[kind]);
        reasons.forEach((reason, j) => out.push(`      ${JSON.stringify(reason)}: ${JSON.stringify(value[kind][reason])}${j < reasons.length - 1 ? "," : ""}`));
        out.push(`    }${k < kinds.length - 1 ? "," : ""}`);
      });
      out.push(`  }${comma}`);
    } else {
      out.push(`  ${JSON.stringify(key)}: ${JSON.stringify(value)}${comma}`);
    }
  });
  out.push("}");
  return `${out.join("\n")}\n`;
}

/** The matrix's row ids and carried cover kinds that the list does not have, or has otherwise. */
export function diff(matrix, list) {
  const { cover, fixtures } = readMatrix(matrix);
  const problems = [];
  const listed = new Map();
  for (const r of list.rows) listed.set(r.row, r.carried);
  for (const [row, text] of Object.entries(cover)) {
    if (!listed.has(row)) problems.push(`${row}: in the matrix's §3.22, not in the list`);
    else if (listed.get(row) !== carried(text).kind) problems.push(`${row}: the matrix carries ${carried(text).kind}, the list ${listed.get(row)}`);
  }
  for (const row of listed.keys()) if (!(row in cover)) problems.push(`${row}: in the list, not in the matrix's §3.22`);
  const fixtureRows = new Set(list.fixtures.map((f) => f.row));
  for (const row of fixtures) if (!fixtureRows.has(row)) problems.push(`${row}: in the matrix's §3.20, not in the list's fixtures`);
  for (const row of fixtureRows) if (!fixtures.includes(row)) problems.push(`${row}: in the list's fixtures, not in the matrix's §3.20`);
  return problems;
}

/** Every issue a cells file's `expected_fail` names. */
export function markedIssues() {
  const issues = new Set();
  for (const name of readdirSync(join(SCENARIOS, "cells")).filter((n) => n.endsWith(".json"))) {
    for (const cell of readJson(join("cells", name)).cells) {
      for (const mark of cell.expected_fail ?? []) issues.add(mark.issue);
    }
  }
  return [...issues].sort((a, b) => a - b);
}

function refreshIssues(repo) {
  const issues = {};
  for (const number of markedIssues()) {
    const state = execFileSync("gh", ["issue", "view", String(number), "-R", repo, "--json", "state", "-q", ".state"], { encoding: "utf8" }).trim();
    issues[number] = state === "OPEN" ? "open" : "closed";
  }
  const doc = {
    $comment: "The issues the cells' expected_fail marks name, and whether each is open (T1.1-08): read by the gate on a pull request, refreshed from GitHub nightly by `node tools/scenario-coverage.mjs issues`.",
    fetched: new Date().toISOString().slice(0, 10),
    issues,
  };
  writeFileSync(join(SCENARIOS, "issues.json"), `${JSON.stringify(doc, null, 2)}\n`);
  const closed = Object.entries(issues).filter(([, s]) => s === "closed").map(([n]) => `#${n}`);
  console.log(`${Object.keys(issues).length} issues${closed.length ? `; closed: ${closed.join(", ")}` : ", all open"}`);
  return 0;
}

function main([command, matrixPath]) {
  if (command === "issues") return refreshIssues(process.env.GITHUB_REPOSITORY || "purlis/purlis");
  if (!["generate", "diff"].includes(command) || !matrixPath) {
    console.error("usage: node tools/scenario-coverage.mjs generate|diff <test-matrix-phase-1.md> | issues");
    return 2;
  }
  const matrix = readFileSync(matrixPath, "utf8");
  if (command === "diff") {
    const problems = diff(matrix, readJson("coverage.json"));
    for (const p of problems) console.log(p);
    console.log(problems.length === 0 ? "the list matches the matrix" : `${problems.length} differences: reconcile the list before the gate`);
    return problems.length === 0 ? 0 : 1;
  }
  const { list, seeds: caps } = generate(matrix, readJson("rows.json"), readJson("escapes.json"));
  const doc = {
    $comment: "The matrix coverage list (T1.1-08, #6855): a cover for every row x dialect x OS of test-matrix-phase-1.md's 3.22 (rows.json), every row 3.20 leaves to fixtures, each dialect's real-binary probes, and every escape x protection of escapes.json. Generated once by tools/scenario-coverage.mjs from matrix v5 (Q321), then owned here: a suite replaces a row's `pending: <ticket>` with its cover. `cargo test -p purlis-scenarios --test coverage` is the gate.",
    version: 1,
    caps,
    approved_marks: [],
    gaps: [
      { rows: ["M-RST-12", "M-RST-13", "M-RST-14"], os: "macos", reason: "host_killed cells run on Linux only: the fixture has no killable host process on macOS (T1.1-06)" },
    ],
    ...list,
  };
  writeFileSync(join(SCENARIOS, "coverage.json"), format(doc));
  console.log(`wrote ${list.rows.length} row covers, ${list.fixtures.length} fixture rows, ${list.real_binary.length} real-binary probes and ${list.escapes.length} escapes`);
  return 0;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
