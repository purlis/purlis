// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { composeStories } from "@storybook/react-vite";
import { cleanup, render, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, it, vi } from "vitest";
import * as stories from "./RaisedAlert.stories";

// Whether the focus is in sight needs a layout, which jsdom has none of: the browser run holds
// that. What jsdom can hold is that the focus is somewhere, never on the page.
vi.mock(import("../../stories.testkit"), async (actual) => ({
  ...(await actual()),
  expectFocusInSight: () => {
    const at = document.activeElement;
    if (at === null || at === document.body) throw new Error("nothing has the focus");
  },
}));

/**
 * **The app-raised alert's stories, walked in jsdom** as `npm run test:stories` walks them in a
 * browser, so what their `play` holds is seen to fail and pass where no browser runs.
 */
const composed = composeStories(stories);
type Composed = (typeof composed)[keyof typeof composed];

afterEach(cleanup);

/** Draws `Story` and walks its `play`, with what a story's run hands it. */
async function walk(Story: Composed) {
  await Story.load();
  const { container } = render(<Story />);
  await Story.play?.({
    canvasElement: container,
    canvas: within(container),
    userEvent: userEvent.setup(),
  } as unknown as Parameters<NonNullable<Composed["play"]>>[0]);
}

describe("the app-raised alert's stories", () => {
  // #7847: a press with the story's `userEvent` left Accept drawn under a pointer that was not
  // there, until the engine's own pointer moved, and WebKit's moves when it will.
  it("pictures the alert raised with no press with no answer drawn under the pointer", async () => {
    await walk(composed.RaisedByTheApp);
  });

  it("walks the alert whose content changes while it is up", async () => {
    await walk(composed.ItsContentChanges);
  });
});
