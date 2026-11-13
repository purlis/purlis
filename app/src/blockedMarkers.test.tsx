// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatBlocks } from "./bindings";
import { BlockedMarker } from "./BlockedMarker";
import {
  BlockedMarkers,
  blockedSaid,
  blocksSeenKey,
  lastLook,
  markerOf,
  useBlockedMarkers,
  type BlockedMarkersHeld,
} from "./blockedMarkers";
import { useChatOnScreenSeen } from "./seen";

/**
 * **The blocked marker's rules** (R-177, R-299, R-341; T1-30): what it shows of a chat's
 * blocks, the look it counts from, and that it touches nothing of Quiet or snooze.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";

const blocks = (over: Partial<ChatBlocks>): ChatBlocks => ({
  session: 7,
  blocks: 0,
  unverified: 0,
  latest: null,
  ...over,
});

describe("the blocked marker", () => {
  it("unverified reports show a muted dot and never count on the sidebar", () => {
    // Unverified alone: a dot, no count.
    expect(markerOf(blocks({ unverified: 3, latest: "01C" }), new Set())).toEqual({
      kind: "dot",
      latest: "01C",
    });
    // Beside verified ones: the count is the verified ones alone.
    expect(markerOf(blocks({ blocks: 2, unverified: 5, latest: "01G" }), new Set())).toEqual({
      kind: "count",
      said: "2 new blocked",
      latest: "01G",
    });
    expect(blockedSaid(1)).toBe("1 new blocked");

    // Drawn: the dot is a muted mark with words for a reader, and no count.
    const held: BlockedMarkersHeld = {
      of: () => markerOf(blocks({ unverified: 2, latest: "01B" }), new Set()),
      open: vi.fn(),
    };
    render(
      <BlockedMarkers.Provider value={held}>
        <BlockedMarker session={7} />
      </BlockedMarkers.Provider>,
    );
    const dot = screen.getByRole("img", { name: "Unverified blocks, counted in its Chat log" });
    expect(dot).toHaveAttribute("data-mark", "blocked-unverified");
    expect(screen.queryByText(/new blocked/)).toBeNull();
  });

  it("nothing shows once the latest block has been looked at, nor for a chat with none", () => {
    const counted = blocks({ blocks: 2, latest: "01B" });
    expect(markerOf(counted, new Set([blocksSeenKey(7, "01B")]))).toBeUndefined();
    expect(markerOf(blocks({}), new Set())).toBeUndefined();
    expect(markerOf(undefined, new Set())).toBeUndefined();
  });

  it("the last look is the latest block looked at, kept in the seen record", () => {
    const seen = new Set([
      "ask:7:1",
      blocksSeenKey(7, "01A"),
      blocksSeenKey(7, "01C"),
      blocksSeenKey(17, "01Z"),
    ]);
    expect(lastLook(seen, 7)).toBe("01C");
    expect(lastLook(seen, 17)).toBe("01Z");
    expect(lastLook(seen, 3)).toBeNull();
  });

  it("a block neither breaks Quiet nor wakes a snooze", async () => {
    const asked: string[] = [];
    let held: ChatBlocks[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      if (cmd === "chat_blocks") return held;
      return null;
    });
    const { result } = renderHook(() => useBlockedMarkers(PLANE, [7], new Set()));
    await waitFor(() => expect(asked).toContain("chat_blocks"));

    // Blocks come: the marker reads them, and that is all it does.
    held = [blocks({ blocks: 2, unverified: 1, latest: "01C" })];
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.get(7)?.blocks).toBe(2));
    expect(new Set(asked)).toEqual(new Set(["chat_blocks"]));
    for (const touched of [
      "snooze_inbox_asks",
      "wake_inbox_asks",
      "set_quiet_now",
      "quiet_now",
      "quiet_typed",
    ])
      expect(asked).not.toContain(touched);
  });

  it("on screen means the pane is visible in the focused window", () => {
    const focus = vi.spyOn(document, "hasFocus");
    const latest = new Map([[7, "01B"]]);
    const look = (shown: boolean, focused: boolean) => {
      focus.mockReturnValue(focused);
      const see = vi.fn();
      const { unmount } = renderHook(() => useChatOnScreenSeen(see, 7, shown, [], [], latest));
      unmount();
      return see.mock.calls.flat(2);
    };

    // Visible in the focused window: its latest block is looked at.
    expect(look(true, true)).toContain(blocksSeenKey(7, "01B"));
    // Hidden, or in a window without focus: it is not.
    expect(look(false, true)).toEqual([]);
    expect(look(true, false)).toEqual([]);
  });
});
