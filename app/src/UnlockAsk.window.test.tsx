// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { Unlock } from "./bindings";
import { UnlockAsk } from "./UnlockAsk";
import { windowCopy } from "./windowCopy";

/**
 * **Your keys locked at a start, in place in New chat** (T1a-61, R-510, R-592): W10 with
 * *Unlock*. While the OS's unlock runs, *Unlock* is busy; a cancel leaves W10 as it was, a
 * failure puts W84 under it, and neither starts the chat; an unlock starts it, with no second
 * press.
 */

afterEach(cleanup);

const ENTRY = "01JA0000000000000000000000";

/** Draws the Ask, the OS's unlock answering `answer` when let go. */
function draw() {
  let answer: (said: Unlock) => void = () => {};
  const unlock = vi.fn(
    () =>
      new Promise<Unlock>((done) => {
        answer = done;
      }),
  );
  const onUnlocked = vi.fn();
  render(<UnlockAsk entry={ENTRY} unlock={unlock} onUnlocked={onUnlocked} />);
  return { unlock, onUnlocked, answer: (said: Unlock) => answer(said) };
}

const unlockButton = () => screen.getByRole("button", { name: "Unlock" });

describe("your keys locked at a start (W10)", () => {
  it("says W10 with Unlock, and Unlock is busy while the OS's unlock runs", async () => {
    const { unlock, answer } = draw();
    expect(screen.getByText(windowCopy.W10)).toBeTruthy();
    await userEvent.click(unlockButton());
    expect(unlock).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(unlockButton().getAttribute("aria-disabled")).toBe("true"));
    // A press while it is busy sends nothing more.
    await userEvent.click(unlockButton());
    expect(unlock).toHaveBeenCalledTimes(1);
    answer("cancelled");
    await waitFor(() => expect(unlockButton().getAttribute("aria-disabled")).not.toBe("true"));
  });

  it("an unlock starts the chat with no second press", async () => {
    const { onUnlocked, answer } = draw();
    await userEvent.click(unlockButton());
    answer("unlocked");
    await waitFor(() => expect(onUnlocked).toHaveBeenCalledTimes(1));
  });

  it("a cancelled unlock leaves W10 as it was and starts nothing", async () => {
    const { onUnlocked, answer } = draw();
    await userEvent.click(unlockButton());
    answer("cancelled");
    await waitFor(() => expect(unlockButton().getAttribute("aria-disabled")).not.toBe("true"));
    expect(screen.getByText(windowCopy.W10)).toBeTruthy();
    expect(screen.queryByText(windowCopy.W84)).toBeNull();
    expect(onUnlocked).not.toHaveBeenCalled();
  });

  it("a failed unlock puts W84 under W10 and starts nothing", async () => {
    const { onUnlocked, answer } = draw();
    await userEvent.click(unlockButton());
    answer("failed");
    await waitFor(() => expect(screen.getByText(windowCopy.W84)).toBeTruthy());
    expect(screen.getByText(windowCopy.W10)).toBeTruthy();
    expect(onUnlocked).not.toHaveBeenCalled();
  });
});
