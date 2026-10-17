// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The built-in themes, handed to a test's window as the app's core hands them to the real
 * one** (`purlis_core::theme::built_ins`, `src-tauri/src/windowprefs.rs`).
 *
 * The real window finds them in the page before its own code runs; a test's window is jsdom,
 * which nothing hands anything. So `test-setup.ts` imports this first, and it puts the core's
 * own built-in files on the same global, as the shape the core resolves them to. The files are
 * read from the core, so there is one copy of each; a built-in is complete (the core's tests
 * prove it), so resolving one is putting its tokens and motion where the window reads them.
 */

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";

/** Vitest's working directory is `app`. */
const BUILT_IN = join(process.cwd(), "..", "crates", "purlis-core", "src", "theme", "built-in");

type File = {
  name: string;
  appearance: string;
  tokens: Record<string, string>;
  motion: Record<string, number | number[]>;
};

function resolved(file: string) {
  const from = JSON.parse(readFileSync(join(BUILT_IN, file), "utf8")) as File;
  const durations: Record<string, number> = {};
  const easings: Record<string, number[]> = {};
  for (const [token, value] of Object.entries(from.motion)) {
    if (Array.isArray(value)) easings[token] = value;
    else durations[token] = value;
  }
  return {
    name: from.name,
    appearance: from.appearance,
    values: from.tokens,
    motion: { durations, easings },
  };
}

const GLOBAL = "__CHARTER_AT_CREATION__";
const held = globalThis as Record<string, unknown>;
held[GLOBAL] ??= { built_in: ["purlis-dark.json", "purlis-light.json"].map(resolved) };
