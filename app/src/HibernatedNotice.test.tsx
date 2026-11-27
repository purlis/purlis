// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  HIBERNATION_CHANGED,
  HibernatedNotice,
  UnsentNotice,
  unsentHandedBack,
} from "./HibernatedNotice";
import type { ChatHibernation } from "./bindings";

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
      "What you typed since was not sent, and is kept until you resume it: go on",
    );
  });

  it("survives its Resume: the woken pane shows it, unsent, until it is put away", async () => {
    core(null);
    render(<UnsentNotice plane={PLANE} session={8} />);
    expect(screen.queryByRole("status", { name: "Not sent" })).toBeNull();
    // What the core hands back with the woken run, chat 8 in chat 7's place.
    act(() => unsentHandedBack(PLANE, 8, "go on\r"));
    const said = await screen.findByRole("status", { name: "Not sent" });
    expect(said).toHaveTextContent(
      "This chat was hibernated when you typed this, so it was not sent: go on Type it again if you still want it.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("status", { name: "Not sent" })).toBeNull();
  });

  it("is shown on no other chat's pane", async () => {
    core(null);
    render(<UnsentNotice plane={PLANE} session={9} />);
    act(() => unsentHandedBack(PLANE, 10, "elsewhere"));
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.queryByRole("status", { name: "Not sent" })).toBeNull();
  });
});
