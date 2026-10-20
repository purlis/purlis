// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatsReconnecting } from "./bindings";
import { RECONNECTING } from "./reconnecting";
import { RunnerKeyChangedNotice } from "./RunnerKeyChangedNotice";

/**
 * **A runner that answered a relayed link with another key is said in a window Notice**
 * (#2752): once for the project, with Open Pools settings, and gone once its chats are reconnected.
 */

const PLANE = "/planes/one";

afterEach(() => {
  cleanup();
  clearMocks();
});

const told = (said: Partial<ChatsReconnecting>) =>
  act(async () => {
    await emit(RECONNECTING, {
      plane: PLANE,
      sessions: [1, 2],
      reconnection: "key_changed",
      ...said,
    } satisfies ChatsReconnecting);
  });

const notice = () => document.querySelector(`[data-cause="runner-key-changed:${PLANE}"]`);

describe("a runner whose key changed", () => {
  it("is said once in the window, opens Pools, and goes once its chats are reconnected", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const onOpenPools = vi.fn();
    render(<RunnerKeyChangedNotice plane={PLANE} onOpenPools={onOpenPools} />);
    // A link that is only down says nothing here: its panes say Reconnecting….
    await told({ reconnection: "reconnecting" });
    expect(notice()).toBeNull();

    await told({});
    await waitFor(() => expect(notice()).not.toBeNull());
    expect(screen.getByText(/answered with another key/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Open Pools settings" }));
    expect(onOpenPools).toHaveBeenCalledOnce();

    // Another project's runner is not this one's.
    await told({ plane: "/planes/two", reconnection: "reconnected" });
    expect(notice()).not.toBeNull();

    await told({ reconnection: "reconnected" });
    await waitFor(() => expect(notice()).toBeNull());
  });

  it("is put away by Dismiss until the key changes again", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    render(<RunnerKeyChangedNotice plane={PLANE} onOpenPools={() => {}} />);
    await told({});
    await waitFor(() => expect(notice()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(notice()).toBeNull());
    await told({ reconnection: "reconnected" });
    await told({});
    await waitFor(() => expect(notice()).not.toBeNull());
  });
});
