// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Theme } from "./theme";

/** One built-in appearance, by its id, as the core hands it. */
export type Handed = { id: string; theme: unknown };

/** One theme a story can be drawn in, with the toolbar's title for it. */
export type StoryTheme = { title: string; theme: Theme };

/**
 * **Every built-in theme a story can be drawn in** (#3034), by the `theme` global's value: each
 * of Purlis's appearances as `purlis:<appearance>`, then each of Purlis Classic's as
 * `purlis-classic:<appearance>`. `.storybook/preview.tsx` offers them on the toolbar, and a
 * theme matrix (#2945) can walk them.
 */
export function storyThemes(
  purlis: readonly Handed[],
  classic: readonly Handed[],
): Record<string, StoryTheme> {
  const each = (id: string, title: string, list: readonly Handed[]) =>
    list.map((one) => [
      `${id}:${one.id}`,
      { title: `${title} ${one.id.split("-").join(" ")}`, theme: one.theme as Theme },
    ]);
  return Object.fromEntries([
    ...each("purlis", "Purlis", purlis),
    ...each("purlis-classic", "Purlis Classic", classic),
  ]);
}

/** The value a story is drawn in when it names none: the default's appearance, by the id the
 *  core's test wrote beside Purlis (`handed.purlis.json`'s `default`). */
export function defaultStoryTheme(defaultId: string): string {
  return `purlis:${defaultId}`;
}
