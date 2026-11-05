// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every story starts with the page's input at rest**, whatever the story before it did. The
 * stories of a file run one after another on one page, and two things stay on it from one to the
 * next. Before each story, in this order:
 *
 * **The engine's pointer is out of the way** (#5063). The pointer stays where it last was. As a
 * story lays out under it, the engine tells the page what came under it: a real `pointerover`,
 * `:hover`, and React Aria's `data-hovered`. So a story's picture could change with where the story
 * before it pressed, and a story that walks a pointer of its own could hear the engine's as its own
 * leaving (#4098). The pointer goes to the window's far corner. `force`, because the story before
 * may still be drawn at that moment, and something of its over the corner must not fail this one on
 * the engine's check of what a pointer may land on. `src/PointerAtRest.stories.tsx` holds this to
 * it.
 *
 * **The input modality is the keyboard's** (#5040). React Aria keeps on the page the modality of
 * the last input: a key, a press, or virtual. A focused control draws its focus ring unless it is a
 * press. A story that ended on a press left it, and the next story's dialog drew its answer without
 * its ring, so its picture failed for a reason in the story before it (#5022's `PoolUnavailable`).
 * It is set with React Aria's own call. That is a choice: a page no input has reached yet has none
 * at all, which the call does not take, and draws the same rings. The keyboard's is what a story's
 * keyboard path leaves, so most stories started from it already. `src/InputModality.stories.tsx`
 * holds this to it.
 *
 * **The order is the point**, and why this is one hook and not a file for each: moving the pointer
 * is an input too, and React Aria takes it as the modality's. Moved after the modality is set, every
 * story would start from a pointer's, and a focused control draw no ring.
 *
 * **And the engine's press, for a story** (`pressWithTheEnginesPointer` in
 * `src/stories.testkit.ts`): set here, as importing `vitest/browser` in a story's file can break
 * jest-dom's matcher types across the program (#4098).
 */
import { setInteractionModality } from "react-aria";
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
  setInteractionModality("keyboard");
});
