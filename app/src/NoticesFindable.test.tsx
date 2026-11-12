// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { composeStories } from "@storybook/react-vite";
import { cleanup, render, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, it, vi } from "vitest";
import * as stories from "./NoticesFindable.stories";

// Whether the focus is in sight needs a layout, which jsdom has none of: the browser run holds it.
vi.mock(import("./stories.testkit"), async (actual) => ({
  ...(await actual()),
  expectFocusInSight: () => undefined,
}));

/**
 * **Notices are findable** (Q182, Q183): the Notices view's stories, each one's keyboard and
 * pointer path walked here in jsdom as `npm run test:stories` walks it in a browser, so each
 * requirement's test is seen to fail and pass where there is no browser to run it in.
 */
const composed = composeStories(stories);
type Composed = (typeof composed)[keyof typeof composed];

afterEach(cleanup);

/** Draws `Story` and walks its `play`, with what a story's run hands it. */
async function walk(Story: Composed) {
  const { container } = render(<Story />);
  const context = {
    canvasElement: container,
    canvas: within(container),
    userEvent: userEvent.setup(),
  };
  await Story.play?.(context as unknown as Parameters<NonNullable<Composed["play"]>>[0]);
}

describe("the Notices view", () => {
  it("typing in the Notices search filters them (R-331)", async () => {
    await walk(composed.Searched);
  });

  it("Notices filter by kind and group with counts (R-332)", async () => {
    await walk(composed.ByKind);
  });

  it("a message that names a window control links to it focused (R-333)", async () => {
    await walk(composed.ToTheControl);
  });

  it("a dismissed Notice comes back from Dismissed (N) (R-334)", async () => {
    await walk(composed.BroughtBack);
  });
});
