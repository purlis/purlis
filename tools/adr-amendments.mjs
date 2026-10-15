// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// An ADR's amendment is a file of its own, `docs/adr/<NNNN>/<yyyy-mm-dd>-<slug>.md`, as a
// changelog entry is a fragment in `changes/` (#1982). Two pull requests that each amend the same
// ADR then add two files and never touch the same lines, where a dated section appended to the
// end of the ADR made every such pair conflict.
//
// This holds the rule: no ADR carries a dated amendment section (an `## ` heading that starts
// with one of the words below and carries a date); an amendments folder belongs to an ADR of its
// number, which links to it; and every file in it is named by its date and a slug and starts
// with its `# ` heading. CI runs it through `tools/adr-amendments.test.mjs`, which checks the
// repository's own `docs/adr/`. `node tools/adr-amendments.mjs` runs it by hand.
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const DATE = /\d{4}-\d{2}-\d{2}/;
const AMENDMENT =
  /^## (?:(?:Amended|Amendment|Noted|Note|Ruled|Re-opened|Decided|Built)\b|ADR \d{4},? amended\b)/;
const ADR = /^(\d{4})-.+\.md$/;
const FOLDER = /^\d{4}$/;
const FILE = /^\d{4}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/;

/** Each `## ` heading of `text` outside a fenced code block, with its 1-based line number. */
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
    if (fence === null && line.startsWith("## ")) found.push({ line: i + 1, text: line });
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
      const date = DATE.exec(heading.text);
      if (date && AMENDMENT.test(heading.text)) {
        out.push(
          `${name}:${heading.line}: "${heading.text.slice(0, 60)}" is an amendment; write it as ` +
            `its own file, docs/adr/${number}/${date[0]}-<slug>.md, not as a section of the ADR`,
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
