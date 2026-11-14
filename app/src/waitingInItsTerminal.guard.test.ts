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
    places: (source) => functionsNamed(source, "held_told_or_given_up"),
    at_least: 1,
  },
  // The board: a give-up is a wait on its harness's permission prompt, kept by its asker, and
  // every place that raises or withdraws a wait on a prompt (Q266): the chat's and the board's.
  {
    file: "crates/purlis-core/src/state.rs",
    places: (source) =>
      [
        "hook_gave_up",
        "waits_in_its_terminal",
        "asked",
        "tool_said",
        "child_tool_said",
        "answered_by",
        "waits_on_its_prompt",
        "helper_gone",
        "helper_tool_said",
      ].flatMap((name) => functionsNamed(source, name)),
    at_least: 14,
  },
  // The app: what it tells the board of a give-up.
  {
    file: "app/src-tauri/src/hooks.rs",
    places: (source) => functionsNamed(source, "told_it_gave_up"),
    at_least: 1,
  },
  // Asks: which chats it lists as waiting in their terminal, and the whole of what it lists.
  {
    file: "app/src-tauri/src/asking.rs",
    places: (source) => [
      ...linesNaming(source, "in_its_terminal"),
      ...functionsNamed(source, "derive").map((place) => shownAlone(place)),
    ],
    at_least: 4,
  },
];

/**
 * **What `derive` shows and never decides by**: a question's text is what the chat's turn last
 * ended on (#4550), read only where the chat is not at its prompt (`None => question(…)`). It is
 * the row's words, never why the row is there, so it alone is taken out before the place is read.
 * A place that no longer reads it fails, so this cannot hide anything else.
 */
const SHOWN_NOT_DECIDED = "(waiting.last_message)(session)";

function shownAlone(place: string): string {
  expect(place, "derive shows a question's last message").toContain(SHOWN_NOT_DECIDED);
  return place.replace(SHOWN_NOT_DECIDED, "");
}

/**
 * What the terminal shows or the chat printed: none of it is a cause (ADR 0076). A word is found
 * inside any name, in any case: `read_screen`, `readScreen`, `ReadScreen` and `READ_SCREEN` alike.
 * Only a lowercase start wants no letter before it, so `pty` is found in `read_pty` and `readPty`
 * but not in `is_empty`.
 */
const TERMINAL_WORDS = [
  "pty",
  "screen",
  "scrollback",
  "vt100",
  "transcript",
  "keystroke",
  "output",
  "last_said",
  "last_message",
  "said_before",
  "read_only_terminal",
  "readonlyterminal",
];
const TERMINAL_OUTPUT = new RegExp(
  [
    ...TERMINAL_WORDS.flatMap((word) => {
      const camel = word.replace(/_(.)/g, (_, next: string) => next.toUpperCase());
      const cap = camel[0].toUpperCase() + camel.slice(1);
      return [`(?<![a-zA-Z])${word}`, `(?<![a-zA-Z])${camel}`, cap, word.toUpperCase()];
    }),
    "chat-screens",
  ].join("|"),
);

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
