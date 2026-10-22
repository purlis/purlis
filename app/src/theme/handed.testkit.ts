// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The built-in appearances, handed to a test's window as the app's core hands them to the
 * real one** (`purlis_core::theme::built_ins`, `src-tauri/src/windowprefs.rs`).
 *
 * The real window finds them in the page before its own code runs; a test's window is jsdom,
 * which nothing hands anything. So `test-setup.ts` imports this first, and it puts Purlis
 * Classic's appearances, read from the core's own theme folder, on the same global, in the shape
 * the core resolves them to: each named by the id a pick names it by, dark first. The folder is
 * read from the core, so there is one copy; Classic is complete (the core's tests prove it), so
 * resolving an appearance is putting its tokens and motion where the window reads them.
 */

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";

/** Vitest's working directory is `app`. */
const CLASSIC = join(
  process.cwd(),
  "..",
  "crates",
  "purlis-core",
  "src",
  "theme",
  "built-in",
  "purlis-classic",
  "theme.json",
);

type Appearance = {
  appearance: string;
  tokens: Record<string, string | boolean | number>;
  motion: Record<string, number | number[]>;
};

/** The ids a pick names Classic's appearances by, in its order (`purlis_core::names`). */
const PICKS = ["purlis-dark", "purlis-light"];

function resolved(from: Appearance, at: number) {
  const durations: Record<string, number> = {};
  const easings: Record<string, number[]> = {};
  for (const [token, value] of Object.entries(from.motion)) {
    if (Array.isArray(value)) easings[token] = value;
    else durations[token] = value;
  }
  // A token is a colour, or one that is not (#2327): a flag, the cursor's shape by name, or a
  // number, as `purlis_core::theme::Typed` holds them.
  const values: Record<string, string> = {};
  const typed = {
    flags: {} as Record<string, boolean>,
    choices: {} as Record<string, string>,
    numbers: {} as Record<string, number>,
  };
  for (const [token, value] of Object.entries(from.tokens)) {
    if (typeof value === "boolean") typed.flags[token] = value;
    else if (typeof value === "number") typed.numbers[token] = value;
    // A colour, or "platform" for what the platform draws.
    else if (value.startsWith("#") || value === "platform") values[token] = value;
    else typed.choices[token] = value;
  }
  return {
    name: PICKS[at],
    appearance: from.appearance,
    values,
    typed,
    motion: { durations, easings },
    // Classic names no icon set, so the rails draw the window's own marks (#2337).
    icons: {},
    tint,
  };
}

/** The recipe the core resolves for Classic (`purlis_core::theme::Tint`), as the shared tint
 *  fixture holds it and the core's tests hold the fixture to the engine. */
const TINT_CASES = join(
  process.cwd(),
  "..",
  "crates",
  "purlis-core",
  "src",
  "theme",
  "tint-cases.json",
);
const tint = (JSON.parse(readFileSync(TINT_CASES, "utf8")) as { classic: unknown }).classic;

const GLOBAL = "__CHARTER_AT_CREATION__";
const held = globalThis as Record<string, unknown>;
const classic = JSON.parse(readFileSync(CLASSIC, "utf8")) as { appearances: Appearance[] };

/** What the core hands a window as it is created, as far as a test's window needs it: the
 *  built-in appearances. A story's page is handed what the core itself resolves for every
 *  built-in theme instead (`.storybook/handed.plugin.ts`, #2945). */
export function handedAtCreation() {
  return { built_in: classic.appearances.map(resolved) };
}

held[GLOBAL] ??= handedAtCreation();
