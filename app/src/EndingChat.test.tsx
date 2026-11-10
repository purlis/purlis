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
 * settings set, the choice family's on React Aria's `RadioGroup`, so it is one stop in WebKit's
 * tab sequence: React Aria writes `tabindex="0"` on the picked radio's `<input>` (`useRadio`),
 * and a native radio is in WebKit's restricted sequence only with one written down
 * (`docs/ui-primitives.md`). Each answer says what it does.
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

/**
 * **End… on a shell asks only Cancel or End** (#4761, ADR 0062 as amended 2026-11-02): a shell
 * runs no harness, so there is no Smart close and nothing it asked for to ask about.
 */
describe("a shell's End…", () => {
  const offer = { id: "chat.end:6", title: "End chat shell 6…" } as unknown as Offer;

  it("has two answers, Cancel and End, and no Smart close", () => {
    render(
      <EndingChat
        offer={offer}
        shell
        smart={{ available: true, why: null, close_first: false }}
        onEnd={() => undefined}
        onSmartClose={() => undefined}
        onCancel={() => undefined}
      />,
    );
    const dialog = screen.getByRole("alertdialog");
    const answers = Array.from(dialog.querySelectorAll("button")).map((one) => one.textContent);
    expect(answers).toEqual(["Cancel", "End"]);
    expect(screen.queryByRole("button", { name: "Smart close" })).toBeNull();
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
  });

  it("ends the shell on End, asking nothing about chats below it", async () => {
    const onEnd = vi.fn();
    render(
      <EndingChat
        offer={offer}
        shell
        onEnd={onEnd}
        onSmartClose={() => undefined}
        onCancel={() => undefined}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "End" }));
    expect(onEnd).toHaveBeenCalledWith(false);
  });
});
