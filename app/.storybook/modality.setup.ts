// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every story starts from the same input modality** (#5040): whatever the story before it did,
 * its last input a key or a press, or stopped part way through, a story is drawn as it is when
 * it runs alone.
 *
 * The stories of a file run one after another on one page, and React Aria keeps two things on
 * that page from one story to the next: the modality of the last input (a key, a press, or
 * virtual), which decides whether a control that takes the focus draws its focus ring; and
 * whether an input has come since the last focus, which decides whether the next focus, a
 * dialog's answer taking it as the dialog opens, counts as virtual or keeps that modality. A
 * story that ended on a press on words left both, and the next story's dialog drew its answer
 * without its ring, so its picture failed for a reason in the story before it (#5022's
 * `PoolUnavailable`). `src/InputModality.stories.tsx` holds this file to it.
 *
 * Before each story both are put back, with React Aria's own calls and nothing of its insides:
 * a focus moved by a call, which React Aria counts as no input, so the focus that follows is the
 * page's first; then the keyboard's modality, the one a story's keyboard path leaves, where a
 * focused control draws its ring, as on a page no input has reached yet.
 */
import { beforeEach } from "vitest";
import { setInteractionModality } from "react-aria";

beforeEach(() => {
  const nowhere = document.createElement("div");
  nowhere.tabIndex = -1;
  document.body.append(nowhere);
  nowhere.focus();
  nowhere.remove();
  setInteractionModality("keyboard");
});
