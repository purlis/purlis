// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ActivityItem } from "./ActivityItem";

afterEach(cleanup);

const item = () => {
  render(<ActivityItem variant="bar" selected={false} icon={null} aria-label="Chats" />);
  return screen.getByRole("button", { name: "Chats" });
};
const pointerGone = () =>
  act(async () => void fireEvent.pointerOut(document, { relatedTarget: null }));

/** **What an item draws only while the pointer is here** goes when the pointer leaves the window,
 *  which an engine may not say on the item (#4628). */
describe("an activity item the pointer left the window from", () => {
  it("stops drawing the hover", async () => {
    const on = item();
    fireEvent.pointerEnter(on, { pointerType: "mouse" });
    expect(on.dataset.hovered).toBeDefined();
    await pointerGone();
    expect(on.dataset.hovered).toBeUndefined();
  });

  it("stops drawing the press", async () => {
    const on = item();
    fireEvent.pointerDown(on, { pointerType: "mouse" });
    expect(on.dataset.pressed).toBeDefined();
    await pointerGone();
    expect(on.dataset.pressed).toBeUndefined();
  });
});
