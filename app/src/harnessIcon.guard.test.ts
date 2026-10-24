// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **A profile's icon is never drawn where purlis asks for consent** (#2922). A profile's icon is
 * the person's to choose, a glyph or an SVG of their own, and a chat can write the file that
 * names it. So `HarnessIcon` is drawn only where a profile is listed beside its name, and never
 * on a surface that asks, answers, approves, allows or grants: a mark there could be taken for
 * purlis's own. This guard holds that by file: no file named for such a surface draws it.
 */

const SRC = join(process.cwd(), "src");

/** A source file's name that says it is an ask, an answer, an approval, a grant or a consent. */
const ASKING = /Ask|Answer|Approv|Grant|Consent|Allow|Permission|SandboxChoose|SandboxBlock/;

/** Every source file the window is drawn from, tests and stories aside. */
function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === "node_modules" ? [] : sources(path);
    return /\.tsx?$/.test(entry.name) && !/\.(test|stories)\.tsx?$/.test(entry.name) ? [path] : [];
  });
}

/** Whether `text` draws the harness icon: imports the component's module. */
const drawsHarnessIcon = (text: string) =>
  /from\s+["'][^"']*harness-icon\/HarnessIcon["']/.test(text);

describe("a profile's icon", () => {
  it("is drawn by no ask, answer, approval, grant or consent surface", () => {
    const found = sources(SRC)
      .filter((path) => ASKING.test(relative(SRC, path)))
      .filter((path) => drawsHarnessIcon(readFileSync(path, "utf8")))
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
    ])
      expect(ASKING.test(asking), asking).toBe(true);
    for (const listing of ["settings/profiles.tsx", "TaskAway.tsx", "StartChat.tsx"])
      expect(ASKING.test(listing), listing).toBe(false);
  });

  it("is found by how a file imports it", () => {
    expect(drawsHarnessIcon('import { HarnessIcon } from "../ui/harness-icon/HarnessIcon";')).toBe(
      true,
    );
    expect(drawsHarnessIcon('import { Button } from "./ui/button/Button";')).toBe(false);
  });
});
