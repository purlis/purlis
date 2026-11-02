// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { InputRefusedNotice } from "./InputRefusedNotice";
import { forgetInputRefused } from "./inputRefused";
import type { InputRefused } from "./bindings";

/**
 * What a pane sends its chat waits on the session host off the window's thread, in the chat's
 * own lane (#4734). A send the host refuses, or one to a chat whose link is down, is said on the
 * chat's pane as a Notice, once, with why and Dismiss: the window never froze waiting for it.
 */

const PLANE = "/home/dev/plane";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInputRefused(PLANE, 7);
  forgetInputRefused(PLANE, 8);
});

const RECONNECTING: InputRefused = {
  plane: PLANE,
  session: 7,
  why: "this chat's runner is reconnecting",
};

const notice = () => screen.queryByRole("status", { name: "Not sent" });

describe("the Notice of input a chat was not sent", () => {
  it("says nothing until the core says a send to this chat was refused, then says why", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    render(<InputRefusedNotice plane={PLANE} session={7} />);
    expect(notice()).toBeNull();

    await act(async () => {
      await emit("chat-input-refused", RECONNECTING);
    });

    await waitFor(() => expect(notice()).not.toBeNull());
    expect(notice()).toHaveTextContent(
      "purlis could not send this chat what you typed, or anything after it: this chat's runner is reconnecting. Type it again once the chat answers.",
    );
  });

  it("leaves a refusal of another chat to that chat's pane", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    render(<InputRefusedNotice plane={PLANE} session={7} />);

    await act(async () => {
      await emit("chat-input-refused", { ...RECONNECTING, session: 8 });
      await emit("chat-input-refused", { ...RECONNECTING, plane: "/home/dev/other" });
    });

    expect(notice()).toBeNull();
  });

  it("goes on Dismiss", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    render(<InputRefusedNotice plane={PLANE} session={7} />);
    await act(async () => {
      await emit("chat-input-refused", RECONNECTING);
    });
    await waitFor(() => expect(notice()).not.toBeNull());

    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));

    await waitFor(() => expect(notice()).toBeNull());
  });
});
