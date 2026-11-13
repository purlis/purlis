// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * **Waiting in its terminal is told by a named cause, never by the terminal** (R-204, R-351,
 * T1-35, ADR 0076). A chat is listed as waiting in its terminal because its permission hook gave
 * up (`NotRaised`), and its row goes because the pane's answer moved its tools on. This reads the
 * Rust that decides it, from where the hook's give-up is heard to where Asks lists it, and fails
 * where any of it takes the terminal's output: its screen, its scrollback, its pty, what the chat
 * printed or said. A place that matches nothing fails too, so a rename cannot drop it out.
 */

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const read = (path: string) => readFileSync(resolve(ROOT, path), "utf8");

/** Every Rust function named `name` in `source`: its signature and body, braces matched. */
function functionsNamed(source: string, name: string): string[] {
  const found: string[] = [];
  const head = new RegExp(`\\bfn ${name}\\b`, "g");
  for (let at = head.exec(source); at !== null; at = head.exec(source)) {
    const open = source.indexOf("{", at.index);
    let depth = 0;
    let end = open;
    for (; end < source.length; end++) {
      if (source[end] === "{") depth++;
      if (source[end] === "}" && --depth === 0) break;
    }
    found.push(source.slice(at.index, end + 1));
  }
  return found;
}

/** The lines of `source` that name `word`. */
const linesNaming = (source: string, word: string) =>
  source.split("\n").filter((line) => line.includes(word));

/** Where the decision is made, each with how many places it has there. */
const DECIDES: { file: string; places: (source: string) => string[]; at_least: number }[] = [
  // The hook channel's hearer: an ask not held is told as a give-up, by its `NotRaised`.
  {
    file: "crates/purlis-core/src/hookwire/permission.rs",
    places: (source) => functionsNamed(source, "held_or_given_up"),
    at_least: 1,
  },
  // The board: a give-up is a wait on its harness's permission prompt, the chat's and the board's.
  {
    file: "crates/purlis-core/src/state.rs",
    places: (source) => [
      ...functionsNamed(source, "hook_gave_up"),
      ...functionsNamed(source, "waits_in_its_terminal"),
    ],
    at_least: 4,
  },
  // The app: what it tells the board of a give-up.
  {
    file: "app/src-tauri/src/hooks.rs",
    places: (source) => functionsNamed(source, "told_it_gave_up"),
    at_least: 1,
  },
  // Asks: which chats it lists as waiting in their terminal.
  {
    file: "app/src-tauri/src/asking.rs",
    places: (source) => linesNaming(source, "in_its_terminal"),
    at_least: 3,
  },
];

/** What the terminal shows or the chat printed: none of it is a cause (ADR 0076). */
const TERMINAL_OUTPUT =
  /\b(pty|Pty|screen|Screen|scrollback|Scrollback|vt100|transcript|Transcript|keystroke|output|Output|last_said|last_message|said_before|ReadOnlyTerminal)\w*|chat-screens/;

describe("nothing_reads_terminal_output_to_tell_waiting", () => {
  it.each(DECIDES)("$file decides from a named cause alone", ({ file, places, at_least }) => {
    const found = places(read(file));
    expect(found.length, `${file}: the decision is where this reads it`).toBeGreaterThanOrEqual(
      at_least,
    );
    for (const place of found) {
      expect(place.match(TERMINAL_OUTPUT)?.[0], `${file} reads the terminal:\n${place}`).toBe(
        undefined,
      );
    }
  });
});
