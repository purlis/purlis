// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every story starts from the same input modality** (#5040): whether the story before it
 * ended on a key, a press, or a failure part way through, a focused control draws its ring as it
 * does when the story runs alone.
 *
 * The stories of a file run one after another on one page, and React Aria keeps on that page the
 * modality of the last input: a key, a press, or virtual. A focused control draws its focus ring
 * unless it is a press. A story that ended on a press left it, and the next story's dialog drew
 * its answer without its ring, so its picture failed for a reason in the story before it
 * (#5022's `PoolUnavailable`). `src/InputModality.stories.tsx` holds this file to it.
 *
 * Before each story the modality is set to the keyboard's, with React Aria's own call. That is a
 * choice: a page no input has reached yet has none at all, which the call does not take, and
 * draws the same rings. The keyboard's is what a story's keyboard path leaves, so most stories
 * started from it already.
 */
import { beforeEach } from "vitest";
import { setInteractionModality } from "react-aria";

beforeEach(() => {
  setInteractionModality("keyboard");
});
