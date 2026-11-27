// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { composeStories } from "@storybook/react-vite";
import { mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as stories from "./ConnectStep.stories";

// Whether the focus is in sight needs a layout, which jsdom has none of: the browser run holds
// that. What jsdom can hold is that the focus is somewhere, never on the page.
vi.mock(import("../stories.testkit"), async (actual) => ({
  ...(await actual()),
  expectFocusInSight: () => {
    const at = document.activeElement;
    if (at === null || at === document.body) throw new Error("nothing has the focus");
  },
}));

/**
 * **Pool setup's Connect stories, walked in jsdom** as `npm run test:stories` walks them in a
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

describe("pool setup's Connect stories", () => {
  it("pictures the add that was not taken with the focus and its ring kept on the button pressed, and no hover", async () => {
    await walk(composed.TheAddWasNotTaken);
  });

  // A button natively `disabled` while it has the focus loses it in WebKit, when WebKit next
  // lays the page out, so the add's picture was of the button with its ring or without (#7853's
  // run 45934794896). Made unavailable by hand, it keeps the focus (#3522).
  it("keeps Trust and create the pool focused while the add runs", async () => {
    await composed.TheAddWasNotTaken.load();
    // The add never answers, so the press is seen while it runs.
    mockIPC((cmd) =>
      cmd === "pool_setup_trust_and_create"
        ? new Promise(() => undefined)
        : cmd === "runner_image_of_this_version"
          ? { state: "ours", image: "ghcr.io/purlis/runner@sha256:aaaa" }
          : null,
    );
    render(<composed.TheAddWasNotTaken />);
    const add = screen.getByRole("button", { name: "Trust and create the pool" });
    await userEvent.setup().click(add);
    await waitFor(() => expect(add).toHaveAttribute("aria-busy", "true"));
    expect(add).not.toBeDisabled();
    expect(add).toHaveAttribute("aria-disabled", "true");
    expect(add).toHaveFocus();
  });
});
