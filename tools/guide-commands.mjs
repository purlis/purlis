#!/usr/bin/env node
// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The commands of `docs/cluster-pool-setup.md` (#4537), so `operator/e2e/kind.sh` runs what the
// guide says and not a copy of it. A `bash` block is either named by a `<!-- kind: <name> -->`
// line above it, which kind.sh runs by that name, or follows a visible `_Not yet run in CI: ...`
// line, which tells the reader so. `tools/guide-commands.test.mjs` refuses any other.
//
//   node tools/guide-commands.mjs <guide> <name> [--value '<placeholder>=<text>']...
//
// prints the block, each placeholder replaced. A placeholder left over is refused, so a command
// that gained one can't run with its angle brackets as a shell redirection.
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const NAMED = /^<!-- kind: ([a-z-]+) -->$/;
const NOT_RUN = /^_Not yet run in CI: .+/;

/** Every `bash` block of `markdown`: `{ name, notRun, body }`, marks read from the lines above it. */
export function blocks(markdown) {
  const lines = markdown.split("\n");
  const found = [];
  let inComment = false;
  for (let at = 0; at < lines.length; at += 1) {
    if (lines[at].includes("<!--") && !lines[at].includes("-->")) inComment = true;
    if (lines[at].includes("-->") && !lines[at].includes("<!--")) inComment = false;
    if (!lines[at].startsWith("```")) continue;
    // A block the page doesn't show would run unseen.
    if (inComment) throw new Error(`a block on line ${at + 1} is inside a comment`);
    const opening = lines[at];
    const end = lines.findIndex((line, index) => index > at && line.startsWith("```"));
    if (end < 0) throw new Error(`a block opened on line ${at + 1} never closes`);
    if (opening === "```bash") {
      let above = at - 1;
      while (above >= 0 && lines[above] === "") above -= 1;
      found.push({
        name: lines[above]?.match(NAMED)?.[1],
        notRun: NOT_RUN.test(lines[above] ?? ""),
        line: at + 1,
        body: lines.slice(at + 1, end).join("\n"),
      });
    }
    at = end;
  }
  return found;
}

/** The block called `name`, with `values` (`[placeholder, text]`) put in. */
export function commands(markdown, name, values = []) {
  const block = blocks(markdown).find((one) => one.name === name);
  if (block === undefined) throw new Error(`the guide has no block called ${name}`);
  let body = block.body;
  for (const [placeholder, text] of values) {
    if (!body.includes(placeholder)) throw new Error(`block ${name} has no ${placeholder}`);
    body = body.replaceAll(placeholder, () => text);
  }
  // `<<` opens a here-document, and `<your registry>` is a placeholder.
  const left = body.match(/<[^<>\s'"][^<>\n]*>/);
  if (left !== null) throw new Error(`block ${name} still holds ${left[0]}`);
  return body;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [guide, name, ...rest] = process.argv.slice(2);
  const values = [];
  for (let at = 0; at < rest.length; at += 2) {
    const given = rest[at + 1] ?? "";
    const split = given.indexOf("=");
    if (rest[at] !== "--value" || split < 0) {
      throw new Error(`expected --value '<placeholder>=<text>', got ${rest[at]} ${given}`);
    }
    values.push([given.slice(0, split), given.slice(split + 1)]);
  }
  process.stdout.write(`${commands(readFileSync(guide, "utf8"), name, values)}\n`);
}
