// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **The purlis UI skill says what the repository has** (#2928, spec #2897 story 76).
 *
 * `building-purlis-ui` is the skill an agent loads to build the window, and it points into the
 * docs rather than restating them. A pointer that has gone stale sends the agent nowhere, so
 * every one is held here, as the purlis-theme skill's commands are held by `purlis theme --help`:
 *
 * - each repository path it names in code exists, and each `#anchor` after a doc is a heading of
 *   that doc;
 * - each `npm run <script>` is a script of `app/package.json`;
 * - its closed state list is the theme engine's `STATES`, in order;
 * - the `NEXT` it tells a family to write is the manifest's.
 */
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const APP = process.cwd();
const REPO = join(APP, "..");
const SKILL = readFileSync(
  join(APP, "src-tauri", "plugin", "skills", "building-purlis-ui", "SKILL.md"),
  "utf8",
);
const PARTS = readFileSync(join(REPO, "crates", "purlis-core", "src", "theme", "parts.rs"), "utf8");

/** Every piece of code in the skill: its code spans, and each line of a fenced block. */
function code(page: string): string[] {
  const out: string[] = [];
  let fenced = false;
  for (const line of page.split("\n")) {
    if (line.trimStart().startsWith("```")) {
      fenced = !fenced;
      continue;
    }
    if (fenced) out.push(line);
    else out.push(...line.split("`").filter((_, i) => i % 2 === 1));
  }
  return out;
}

/** GitHub's anchor for a heading: lower case, punctuation dropped, spaces as dashes. */
function slug(heading: string): string {
  return heading
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");
}

function anchors(doc: string): Set<string> {
  return new Set(
    readFileSync(join(REPO, doc), "utf8")
      .split("\n")
      .filter((line) => /^#{1,6} /.test(line))
      .map((line) => slug(line.replace(/^#+ /, "").trim())),
  );
}

/** The first group `pattern` finds in `text`, or a failure naming what was looked for. */
function first(pattern: RegExp, text: string, what: string): string {
  const hit = pattern.exec(text);
  if (!hit) throw new Error(`no ${what}`);
  return hit[1];
}

/** A path from the repository's root, with no placeholder in it. */
const ROOTS = /^(app|crates|docs|tools)\//;

describe("the purlis UI skill", () => {
  it("names only paths the repository has, and headings its docs have", () => {
    const paths = code(SKILL)
      .flatMap((piece) => piece.split(/\s+/))
      .filter((word) => ROOTS.test(word) && !word.includes("<"));
    expect(paths.length).toBeGreaterThan(10);
    const missing = paths.filter((path) => {
      const [file, anchor] = path.split("#");
      if (!existsSync(join(REPO, file))) return true;
      return anchor !== undefined && !anchors(file).has(anchor);
    });
    expect(missing).toEqual([]);
  });

  it("runs only npm scripts the app has", () => {
    const scripts = Object.keys(
      (JSON.parse(readFileSync(join(APP, "package.json"), "utf8")) as { scripts: object }).scripts,
    );
    const named = code(SKILL).flatMap((piece) =>
      [...piece.matchAll(/npm run ([\w:-]+)/g)].map((hit) => hit[1]),
    );
    expect(named.length).toBeGreaterThan(0);
    expect(named.filter((script) => !scripts.includes(script))).toEqual([]);
  });

  it("lists the manifest's closed states, in its order", () => {
    const block = first(/pub const STATES: \[State; \d+\] = \[([^\]]*)\]/, PARTS, "STATES");
    const names = [...block.matchAll(/State::(\w+)/g)].map((hit) =>
      first(new RegExp(`Self::${hit[1]} => "([a-z-]+)"`), PARTS, `name of State::${hit[1]}`),
    );
    const listed = first(/\*\*The states are a closed list\*\*:([^.]*)\./s, SKILL, "state list");
    const said = [...listed.matchAll(/`([a-z-]+)`/g)].map((hit) => hit[1]);
    expect(said).toEqual(names);
  });

  it("tells a family to write the manifest's NEXT", () => {
    expect(PARTS).toMatch(/pub const NEXT: u32 = VERSION \+ 1;/);
    expect(SKILL).toContain("`super::NEXT`");
    expect(SKILL).toContain("`parts::NEXT`");
  });
});
