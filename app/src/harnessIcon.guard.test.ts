// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **A profile's icon is never drawn where purlis asks for consent** (#2922). A profile's icon is
 * the person's to choose, a glyph or an SVG of their own, and a chat can write the file that
 * names it. So `HarnessIcon` is drawn only where a profile is listed beside its name, and never
 * on a surface that asks, answers, approves, allows, accepts, confirms, trusts or grants: a mark
 * there could be taken for purlis's own. This guard holds that by file: no file named for such
 * a surface reaches the component, by its own import or through a module that re-exports it.
 */

const SRC = join(process.cwd(), "src");

/** The component's own module. */
const HARNESS_ICON = join(SRC, "ui/harness-icon/HarnessIcon.tsx");

/** A source file's name that says it is an ask, an answer, an approval, a grant or a consent. */
const ASKING =
  /Ask|Answer|Approv|Grant|Consent|Allow|Permission|Trust|Confirm|Accept|SandboxChoose|SandboxBlock/;

/** Every source file the window is drawn from, tests and stories aside. */
function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === "node_modules" ? [] : sources(path);
    return /\.tsx?$/.test(entry.name) && !/\.(test|stories)\.tsx?$/.test(entry.name) ? [path] : [];
  });
}

/** Every module specifier `text` imports or re-exports from, each marked if it re-exports. */
function specifiers(text: string): { from: string; reexports: boolean }[] {
  const found: { from: string; reexports: boolean }[] = [];
  const statement =
    /\b(import|export)\b[^;'"]*?\bfrom\s*["']([^"']+)["']|\bimport\s*["']([^"']+)["']|\bimport\(\s*["']([^"']+)["']\s*\)/g;
  for (const hit of text.matchAll(statement)) {
    const from = hit[2] ?? hit[3] ?? hit[4];
    if (from !== undefined) found.push({ from, reexports: hit[1] === "export" });
  }
  return found;
}

/** The file `from`, imported by `file`, names: as written, or with the extension or index file
 *  the bundler adds. Nothing for a package. */
function resolved(file: string, from: string): string | undefined {
  if (!from.startsWith(".")) return undefined;
  const bare = resolve(dirname(file), from.replace(/\.js$/, ""));
  for (const candidate of [
    bare,
    `${bare}.ts`,
    `${bare}.tsx`,
    join(bare, "index.ts"),
    join(bare, "index.tsx"),
  ])
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
  return undefined;
}

/** Every module that is the component or re-exports it, however many re-exports away. */
function reachingIt(files: readonly string[]): Set<string> {
  const reaching = new Set([HARNESS_ICON]);
  for (let grew = true; grew;) {
    grew = false;
    for (const file of files) {
      if (reaching.has(file)) continue;
      const passesItOn = specifiers(readFileSync(file, "utf8")).some(
        ({ from, reexports }) => reexports && reaching.has(resolved(file, from) ?? ""),
      );
      if (passesItOn) {
        reaching.add(file);
        grew = true;
      }
    }
  }
  return reaching;
}

/** Whether `file` reaches the component: imports it, or a module that re-exports it. */
function draws(file: string, reaching: ReadonlySet<string>): boolean {
  return specifiers(readFileSync(file, "utf8")).some(({ from }) =>
    reaching.has(resolved(file, from) ?? ""),
  );
}

describe("a profile's icon", () => {
  it("is drawn by no ask, answer, approval, grant or consent surface", () => {
    const files = sources(SRC);
    const reaching = reachingIt(files);
    const found = files
      .filter((path) => ASKING.test(relative(SRC, path)))
      .filter((path) => draws(path, reaching))
      .map((path) => relative(SRC, path));
    expect(found).toEqual([]);
  });

  it("names the surfaces it keeps the icon from", () => {
    // The pattern reads what it should: the asks and grants by name, and not a task's files.
    for (const asking of [
      "AskPersona.tsx",
      "ChatAsk.tsx",
      "AnswerQuestion.tsx",
      "ApprovePlane.tsx",
      "ProfileApproval.tsx",
      "DispatchGrantNotice.tsx",
      "settings/GrantedList.tsx",
      "SandboxChoose.tsx",
      "TrustProject.tsx",
      "ConfirmRemove.tsx",
      "AcceptChange.tsx",
    ])
      expect(ASKING.test(asking), asking).toBe(true);
    for (const listing of ["settings/profiles.tsx", "TaskAway.tsx", "StartChat.tsx"])
      expect(ASKING.test(listing), listing).toBe(false);
  });

  it("follows an import with or without its extension, and a re-export", () => {
    const here = join(SRC, "settings/profiles.tsx");
    expect(resolved(here, "../ui/harness-icon/HarnessIcon")).toBe(HARNESS_ICON);
    expect(resolved(here, "../ui/harness-icon/HarnessIcon.tsx")).toBe(HARNESS_ICON);
    expect(resolved(here, "../ui/harness-icon/HarnessIcon.js")).toBe(HARNESS_ICON);
    expect(resolved(here, "react")).toBeUndefined();
    expect(specifiers('export { HarnessIcon } from "./HarnessIcon";')).toEqual([
      { from: "./HarnessIcon", reexports: true },
    ]);
    expect(specifiers('import { HarnessIcon as Mark } from "./x";')).toEqual([
      { from: "./x", reexports: false },
    ]);
    // The one window file that draws it today is found drawing it.
    expect(draws(here, reachingIt(sources(SRC)))).toBe(true);
  });
});
