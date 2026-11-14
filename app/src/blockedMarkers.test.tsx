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
  filterChatLog,
  lastLook,
  markerOf,
  useBlockedMarkers,
  useChatLogFilter,
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

/** Chat 7's logged id, which its look is keyed by (Q258). */
const CHAT = "01K6CHAT000000000000000007";

/** The seen record's key for a look at chat 7's block `seq`, as the core hands it. */
const look = (seq: string, chat = CHAT) => `blocks:${chat}:${seq}`;

const blocks = (over: Partial<ChatBlocks>): ChatBlocks => ({
  session: 7,
  blocks: 0,
  unverified: 0,
  key: null,
  ...over,
});

describe("the blocked marker", () => {
  it("unverified reports show a muted dot and never count on the sidebar", () => {
    // Unverified alone: a dot, no count.
    expect(markerOf(blocks({ unverified: 3, key: look("01C") }), new Set())).toEqual({
      kind: "dot",
      key: look("01C"),
    });
    // Beside verified ones: the count is the verified ones alone.
    expect(markerOf(blocks({ blocks: 2, unverified: 5, key: look("01G") }), new Set())).toEqual({
      kind: "count",
      said: "2 new blocked",
      key: look("01G"),
    });
    expect(blockedSaid(1)).toBe("1 new blocked");

    // Drawn: the dot is a muted mark with words for a reader, and no count.
    const held: BlockedMarkersHeld = {
      of: () => markerOf(blocks({ unverified: 2, key: look("01B") }), new Set()),
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
    const counted = blocks({ blocks: 2, key: look("01B") });
    expect(markerOf(counted, new Set([look("01B")]))).toBeUndefined();
    expect(markerOf(blocks({}), new Set())).toBeUndefined();
    expect(markerOf(undefined, new Set())).toBeUndefined();
  });

  it("the last look is keyed by the chat's logged id, never its session number", () => {
    const other = "01K6CHAT000000000000000017";
    const seen = new Set(["ask:7:1", "blocks:7:09", look("01C"), look("01Z", other)]);
    // The look of the chat the core's key names, whatever its session number.
    expect(lastLook(seen, look("01D"))).toBe("01C");
    expect(lastLook(seen, look("02A", other))).toBe("01Z");
    // A chat not yet counted, or never looked at, has none.
    expect(lastLook(seen, null)).toBeNull();
    expect(lastLook(seen, look("01A", "01K6CHAT000000000000000003"))).toBeNull();
  });

  it("the marker asks from the look of the chat the core named", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd !== "chat_blocks") return null;
      asked.push((args as { asked: unknown }).asked);
      return [blocks({ blocks: 1, key: look("01D") })];
    });
    renderHook(() => useBlockedMarkers(PLANE, [7], new Set([look("01C")])));
    await waitFor(() => expect(asked).toContainEqual([{ session: 7, after: "01C" }]));
  });

  it("an unchanged answer keeps the same map, so a poll redraws nothing", async () => {
    let reads = 0;
    mockIPC((cmd) => {
      if (cmd !== "chat_blocks") return null;
      reads += 1;
      return [blocks({ blocks: 2, key: look("01B") })];
    });
    const { result } = renderHook(() => useBlockedMarkers(PLANE, [7], new Set()));
    await waitFor(() => expect(result.current.get(7)?.blocks).toBe(2));
    const first = result.current;
    const before = reads;

    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });
    await waitFor(() => expect(reads).toBeGreaterThan(before));
    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current).toBe(first);
  });

  it("a chat log's filter is its project's, not another project's chat of the same number", () => {
    filterChatLog("/home/dev/one", 7, "blocks");
    const one = renderHook(() => useChatLogFilter("/home/dev/one", 7));
    const two = renderHook(() => useChatLogFilter("/home/dev/two", 7));
    expect(one.result.current[0]).toBe("blocks");
    expect(two.result.current[0]).toBe("all");
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
    held = [blocks({ blocks: 2, unverified: 1, key: look("01C") })];
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
    const latest = new Map([[7, look("01B")]]);
    const seenAt = (shown: boolean, focused: boolean) => {
      focus.mockReturnValue(focused);
      const see = vi.fn();
      const { unmount } = renderHook(() => useChatOnScreenSeen(see, 7, shown, [], [], latest));
      unmount();
      return see.mock.calls.flat(2);
    };

    // Visible in the focused window: its latest block is looked at.
    expect(seenAt(true, true)).toContain(look("01B"));
    // Hidden, or in a window without focus: it is not.
    expect(seenAt(false, true)).toEqual([]);
    expect(seenAt(true, false)).toEqual([]);
  });
});
