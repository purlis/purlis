// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";

/**
 * **The built-in themes as the core hands them to a window**, read from `file`, this folder's
 * `built-in-themes.json` (`docs/component-stories.md`), which each reader finds from where it runs.
 * The file keeps each appearance's parts stylesheet a rule a line (`storythemes.rs`, operator
 * ruling 2026-10-26), so two pull requests that change two families' parts change two lines of
 * it; this joins them again, a newline after each, into the stylesheet the engine wrote.
 */
export function readBuiltInThemes<Theme = Record<string, unknown>>(
  file: string,
): Record<string, Record<string, Theme & { parts: string }>> {
  const kept = JSON.parse(readFileSync(file, "utf8")) as Record<
    string,
    Record<string, Record<string, unknown>>
  >;
  return Object.fromEntries(
    Object.entries(kept).map(([theme, appearances]) => [
      theme,
      Object.fromEntries(
        Object.entries(appearances).map(([id, drawn]) => [
          id,
          { ...drawn, parts: joined(drawn.parts) } as Theme & { parts: string },
        ]),
      ),
    ]),
  );
}

/** A stylesheet kept a rule a line, as the engine wrote it. */
export function joined(parts: unknown): string {
  if (typeof parts === "string") return parts;
  if (!Array.isArray(parts)) return "";
  return parts.map((line) => `${String(line)}\n`).join("");
}
