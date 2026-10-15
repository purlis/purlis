// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// An ADR's amendment is a file of its own, `docs/adr/<NNNN>/<yyyy-mm-dd>-<slug>.md`, as a
// changelog entry is a fragment in `changes/` (#1982). Two pull requests that each amend the same
// ADR then add two files and never touch the same lines, where a dated section appended to the
// end of the ADR made every such pair conflict.
//
// This holds the rule: no ADR carries a dated amendment section (an `## ` or `### ` heading that
// starts with one of the words below and carries a date), and the refusal names the file to write
// instead; an amendments folder belongs to an ADR of its
// number, which links to it; and every file in it is named by its date and a slug and starts
// with its `# ` heading. CI runs it through `tools/adr-amendments.test.mjs`, which checks the
// repository's own `docs/adr/`. `node tools/adr-amendments.mjs` runs it by hand.
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const DATE = /\d{4}-\d{2}-\d{2}/;
const AMENDMENT =
  /^#{2,3} (?:(?:Amended|Amendment|Noted|Note|Ruled|Re-opened|Decided|Built)\b|ADR \d{4},? amended\b)/;
const ADR = /^(\d{4})-.+\.md$/;
const FOLDER = /^\d{4}$/;
const FILE = /^\d{4}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/;

const STOP = new Set(
  "a an the and of to by in is its for on or not be as at it".split(" "),
);

/** A heading's text as it reads, without its links' targets, code marks or emphasis. */
function plain(text) {
  return text
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/`/g, "")
    .replace(/\*\*|__/g, "")
    .replace(/(^|\W)\*(\S)/g, "$1$2")
    .replace(/(\S)\*(\W|$)/g, "$1$2");
}

/**
 * The file an amendment with this heading (its text after the `#`s) is written as: its first
 * date, then a slug of what follows the heading's first ": " (all of it when there is none), cut
 * at an em dash, without dates, at most 50 characters and not ending on a small word.
 */
export function amendmentFile(heading) {
  const date = DATE.exec(heading)?.[0] ?? "yyyy-mm-dd";
  const colon = heading.indexOf(": ");
  const words = plain(colon === -1 ? heading : heading.slice(colon + 2))
    .split(" — ")[0]
    .replace(new RegExp(DATE.source, "g"), "")
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean);
  while (words.join("-").length > 50) words.pop();
  while (words.length > 1 && STOP.has(words[words.length - 1])) words.pop();
  return `${date}-${words.join("-") || "amendment"}.md`;
}

/** Each `## ` and `### ` heading of `text` outside a fenced code block, with its line number. */
function sectionHeadings(text) {
  const found = [];
  let fence = null;
  text.split("\n").forEach((line, i) => {
    const marker = /^\s*(`{3,}|~{3,})/.exec(line);
    if (marker) {
      if (fence === null) fence = marker[1][0];
      else if (marker[1][0] === fence) fence = null;
      return;
    }
    if (fence === null && /^#{2,3} /.test(line)) found.push({ line: i + 1, text: line });
  });
  return found;
}

/** What breaks the rule in the ADR directory `dir`, one line each; empty when nothing does. */
export function problems(dir) {
  const entries = readdirSync(dir, { withFileTypes: true });
  const adrs = new Map();
  const out = [];
  for (const e of entries) {
    const m = e.isFile() && ADR.exec(e.name);
    if (m) adrs.set(m[1], e.name);
  }
  for (const [number, name] of [...adrs].sort()) {
    const text = readFileSync(join(dir, name), "utf8");
    for (const heading of sectionHeadings(text)) {
      if (DATE.test(heading.text) && AMENDMENT.test(heading.text)) {
        const file = amendmentFile(heading.text.replace(/^#+ /, ""));
        out.push(
          `${name}:${heading.line}: "${heading.text.slice(0, 60)}" is an amendment; move it to ` +
            `its own file, docs/adr/${number}/${file}, with its heading as the file's "# " line`,
        );
      }
    }
    if (existsSync(join(dir, number)) && !text.includes(`](${number}/)`)) {
      out.push(
        `${name}: it has amendments in ${number}/ and does not link to them, as (${number}/)`,
      );
    }
  }
  for (const e of entries.filter((e) => e.isDirectory() && FOLDER.test(e.name))) {
    if (!adrs.has(e.name)) out.push(`${e.name}/: no ADR ${e.name}-<title>.md is beside it`);
    for (const file of readdirSync(join(dir, e.name)).sort()) {
      const path = `${e.name}/${file}`;
      if (!FILE.test(file)) {
        out.push(`${path}: an amendment is named <yyyy-mm-dd>-<slug>.md, in lower case`);
      } else if (!readFileSync(join(dir, path), "utf8").startsWith("# ")) {
        out.push(`${path}: an amendment starts with its "# " heading`);
      }
    }
  }
  return out;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const found = problems(fileURLToPath(new URL("../docs/adr", import.meta.url)));
  for (const line of found) console.error(line);
  if (found.length) process.exit(1);
  console.log("every ADR amendment is a file of its own");
}
