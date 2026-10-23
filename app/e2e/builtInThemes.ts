// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * **A built-in theme's appearance, as the core resolves it** (#3034): read from the file the
 * stories read (`.storybook/built-in-themes.json`), which a test of the app's crate holds to the
 * core. For a spec that needs one the window is not handed at creation (Purlis's light: a pick of
 * it asks the core), by the theme's id and the appearance's id.
 */
export function builtInAppearance(
  theme: string,
  appearance: string,
): { values: Record<string, string> } {
  const file = join(
    dirname(fileURLToPath(import.meta.url)),
    "..",
    ".storybook",
    "built-in-themes.json",
  );
  const themes = JSON.parse(readFileSync(file, "utf8")) as Record<
    string,
    Record<string, { values: Record<string, string> }>
  >;
  const found = themes[theme]?.[appearance];
  if (found === undefined) throw new Error(`no built-in ${theme} ${appearance}`);
  return found;
}
