// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every story starts with nothing under the engine's pointer** (#5063): wherever the story before
 * it left it, a control is hovered only if the story itself puts the pointer there.
 *
 * The stories of a file run one after another on one page, and the engine's own pointer stays where
 * it last was. As a story lays out under it, the engine tells the page what came under it: a real
 * `pointerover`, `:hover`, and React Aria's `data-hovered`. So a story's picture could change with
 * where the story before it pressed, and a story that walks a pointer of its own could hear the
 * engine's as its own leaving (#4098). `src/PointerAtRest.stories.tsx` holds this file to it.
 *
 * Before each story the pointer goes to the frame's far corner. `force`, because the story before
 * may still be drawn at that moment, and something of its over the corner must not fail this one on
 * the engine's check of what a pointer may land on.
 *
 * **And the engine's press, for a story** (`pressWithTheEnginesPointer` in
 * `src/stories.testkit.ts`): set here, where `vitest/browser` is imported, and not in a story.
 */
import { beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { ENGINE_PRESS_GLOBAL } from "../src/stories.testkit";

(globalThis as Record<string, unknown>)[ENGINE_PRESS_GLOBAL] = (element: Element) =>
  userEvent.click(element);

beforeEach(async () => {
  const root = document.documentElement;
  await userEvent.hover(root, {
    position: { x: root.clientWidth - 1, y: root.clientHeight - 1 },
    force: true,
  });
});
