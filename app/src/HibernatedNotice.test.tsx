// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { HIBERNATION_CHANGED, HibernatedNotice, UnsentNotice } from "./HibernatedNotice";
import type { ChatHibernation, HandedBack } from "./bindings";

/**
 * A chat the person hibernated says so on its pane (SC-4a, #7630), with Resume, its restart on
 * its conversation in one press.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const BY_YOU: ChatHibernation = { by: "operator" };

/** A core holding `held` for chat 7. */
function core(held: ChatHibernation | null) {
  let now = held;
  mockIPC((cmd) => (cmd === "chat_hibernated" ? now : null), { shouldMockEvents: true });
  return {
    hibernate: (one: ChatHibernation | null) => {
      now = one;
    },
  };
}

describe("the band of a hibernated chat", () => {
  it("says the chat is hibernated, with Resume", async () => {
    core(BY_YOU);
    render(<HibernatedNotice plane={PLANE} session={7} onResume={() => {}} />);
    const said = await screen.findByRole("status", { name: "Hibernated" });
    expect(said).toHaveTextContent(
      "This chat is hibernated: its program ended and its conversation is kept. Resume starts it again on its conversation.",
    );
    expect(screen.getByRole("button", { name: "Resume" })).toBeVisible();
  });

  it("asks for the chat's restart on its conversation when Resume is pressed", async () => {
    core(BY_YOU);
    const onResume = vi.fn();
    render(<HibernatedNotice plane={PLANE} session={7} onResume={onResume} />);
    await userEvent.click(await screen.findByRole("button", { name: "Resume" }));
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it("shows nothing for a chat that is not hibernated", async () => {
    core(null);
    render(<HibernatedNotice plane={PLANE} session={7} onResume={() => {}} />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.queryByRole("status", { name: "Hibernated" })).toBeNull();
  });

  it("appears when the core says the chat was hibernated", async () => {
    const held = core(null);
    render(<HibernatedNotice plane={PLANE} session={7} onResume={() => {}} />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.queryByRole("status", { name: "Hibernated" })).toBeNull();
    held.hibernate(BY_YOU);
    await act(async () => {
      await emit(HIBERNATION_CHANGED, { plane: PLANE, session: 7 });
    });
    expect(await screen.findByRole("status", { name: "Hibernated" })).toBeVisible();
  });
});

describe("what the person typed into a hibernated chat (SC-4d, #7633)", () => {
  it("is shown on its band, unsent, and again when the core says it changed", async () => {
    const held = core(BY_YOU);
    render(<HibernatedNotice plane={PLANE} session={7} onResume={() => {}} />);
    const said = await screen.findByRole("status", { name: "Hibernated" });
    expect(said).not.toHaveTextContent("was not sent");
    held.hibernate({ by: "operator", unsent: "go on\r" });
    await act(async () => {
      await emit(HIBERNATION_CHANGED, { plane: PLANE, session: 7, hibernated: true });
    });
    expect(await screen.findByRole("status", { name: "Hibernated" })).toHaveTextContent(
      "What you typed since was not sent, and is kept until it wakes: “go on”.",
    );
  });

  it("says why typing in it did not wake it, as a sentence of its own (SC-4j, #7639)", async () => {
    core({ by: "operator", unsent: "go on\r", not_woken: "the kill switch is thrown" });
    render(<HibernatedNotice plane={PLANE} session={7} onResume={() => {}} />);
    const said = await screen.findByRole("status", { name: "Hibernated" });
    expect(said).toHaveTextContent(
      "What you typed since was not sent, and is kept until it wakes: “go on”. purlis could not wake it: the kill switch is thrown.",
    );
    expect(screen.getByRole("button", { name: "Resume" })).toBeVisible();
  });

  it("says the reason of the latest refused wake once the core says it changed", async () => {
    const held = core({
      by: "operator",
      unsent: "go on\r",
      not_woken: "the kill switch is thrown",
    });
    render(<HibernatedNotice plane={PLANE} session={7} onResume={() => {}} />);
    await screen.findByText(/the kill switch is thrown/);
    held.hibernate({
      by: "operator",
      unsent: "go on\r",
      not_woken: "its harness would not start.",
    });
    await act(async () => {
      await emit(HIBERNATION_CHANGED, { plane: PLANE, session: 7, hibernated: true });
    });
    const said = await screen.findByRole("status", { name: "Hibernated" });
    expect(said).toHaveTextContent("purlis could not wake it: its harness would not start.");
    expect(said).not.toHaveTextContent("kill switch");
  });
});

describe("what a woken chat never read, handed back (SC-4j, #7639)", () => {
  it("is shown on the pane, unsent, with why, until it is put away", async () => {
    let back: HandedBack | null = {
      text: "go on\r",
      why: "Its new run ended before it was ready to read it.",
    };
    mockIPC(
      (cmd) => {
        if (cmd !== "chat_unsent_handed_back") return null;
        // Handed over once, as the core does.
        const once = back;
        back = null;
        return once;
      },
      { shouldMockEvents: true },
    );
    render(<UnsentNotice plane={PLANE} session={8} />);
    const said = await screen.findByRole("status", { name: "Not sent" });
    expect(said).toHaveTextContent(
      "This chat was woken, but what you typed while it was hibernated was not sent: “go on”. Its new run ended before it was ready to read it. Type it again if you still want it.",
    );
    await act(async () => {
      await emit(HIBERNATION_CHANGED, { plane: PLANE, session: 8, hibernated: false });
    });
    expect(screen.getByRole("status", { name: "Not sent" })).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: /Dismiss/ }));
    expect(screen.queryByRole("status", { name: "Not sent" })).toBeNull();
  });
});
