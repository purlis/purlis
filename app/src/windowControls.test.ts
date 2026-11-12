// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { CLONE_VOUCH } from "./CloneVouchNotice";
import { familyOf } from "./Notice";
import { NOTICE_KINDS } from "./noticeKinds";
import { WINDOW_CONTROLS, controlsNamedIn, goToSays } from "./windowControls";

/**
 * **Every message that names a window control links to it** (Q182, R-333), held as a string
 * test over the core's sources: a message that tells the person to press a control on a Notice
 * in the window names one the window can go to, so the result that carries it has its Go to.
 */
const CRATES = join(__dirname, "..", "..", "crates");

/** Every Rust source of the core and its crates, but their tests. */
function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === "tests" ? [] : sources(path);
    return entry.name.endsWith(".rs") && !entry.name.endsWith("tests.rs") ? [path] : [];
  });
}

/** A message's words, its string literals joined across Rust's line continuations. */
const unwrapped = (text: string) => text.replace(/\\\n\s*/g, "");

/** "press <control> on its notice in the window", "… on a workspace's clone made outside
 *  purlis": a control that stands on a Notice of the window's Notices. */
const ON_A_NOTICE =
  /[Pp]ress ([A-Z][\w ]*?) on (?:its notice in the window|a workspace's clone made outside purlis)/g;

describe("the window controls the core's messages name", () => {
  const named = sources(CRATES).flatMap((path) =>
    [...unwrapped(readFileSync(path, "utf8")).matchAll(ON_A_NOTICE)].map((found) => ({
      path,
      words: found[1],
    })),
  );

  it("finds the messages it holds the window to", () => {
    expect(named.map((one) => one.words)).toContain("Trust this clone");
  });

  it("goes to every control a core message names on a Notice", () => {
    const listed = WINDOW_CONTROLS.map((control) => control.words);
    for (const one of named) expect(listed, one.path).toContain(one.words);
  });

  it("goes to Trust this clone on a clone made outside purlis", () => {
    expect(WINDOW_CONTROLS.find((one) => one.words === "Trust this clone")?.family).toBe(
      CLONE_VOUCH,
    );
  });

  it("names each control's Notice by a kind the Notices view filters by", () => {
    for (const control of WINDOW_CONTROLS)
      expect(NOTICE_KINDS[familyOf(`${control.family}:x`)], control.words).toBeDefined();
  });

  it("links every message naming a control to it, by its words", () => {
    const message =
      "If you made this clone yourself, press Trust this clone on its notice in the window";
    expect(controlsNamedIn(message).map(goToSays)).toEqual(["Go to Trust this clone"]);
    expect(controlsNamedIn("Session saved")).toEqual([]);
  });
});
