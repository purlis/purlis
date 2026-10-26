// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { Offer } from "./actions";
import { EndingChat, KEEP_SAYS, RUNNING_SAYS, STOP_SAYS } from "./EndingChat";

afterEach(cleanup);

/**
 * **The answer about the chats a closing chat started** (#630, DS-8): a radio group of the
 * settings set, so it is a Radix radio group in WebKit's tab sequence (`docs/ui-primitives.md`:
 * a native radio `<input>` is in WebKit's restricted one), and each answer says what it does.
 */
describe("the close question's answer about the chats still at work", () => {
  const offer = { id: "pane.close", title: "Close chat" } as unknown as Offer;

  it("is one group, named by its question, whose answers each say what they do", () => {
    render(
      <EndingChat
        offer={offer}
        running={["helper"]}
        onEnd={() => undefined}
        onSmartClose={() => undefined}
        onCancel={() => undefined}
      />,
    );
    const group = screen.getByRole("radiogroup", { name: RUNNING_SAYS(["helper"]) });
    expect(group).toBeTruthy();
    const keep = screen.getByRole("radio", { name: "Keep them running" });
    const stop = screen.getByRole("radio", { name: "Stop them" });
    expect(keep).toBeChecked();
    // A native radio of the radio group (#2907), with a written-down tabindex for WebKit.
    expect(keep.tagName).toBe("INPUT");
    expect(keep.getAttribute("tabindex")).not.toBeNull();
    expect(keep).toHaveAccessibleDescription(KEEP_SAYS);
    expect(stop).toHaveAccessibleDescription(STOP_SAYS);
  });

  it("sends the answer picked with Close", async () => {
    const onEnd = vi.fn();
    render(
      <EndingChat
        offer={offer}
        running={["helper"]}
        onEnd={onEnd}
        onSmartClose={() => undefined}
        onCancel={() => undefined}
      />,
    );
    await userEvent.click(screen.getByRole("radio", { name: "Stop them" }));
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onEnd).toHaveBeenCalledWith(true);
  });
});
