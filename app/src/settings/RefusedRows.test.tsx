// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { BlockedLately } from "../bindings";
import { BlockedMarkers } from "../blockedMarkers";
import { RefusedRows } from "./NetworkLists";

afterEach(() => {
  cleanup();
  clearMocks();
});

const ROW: BlockedLately = {
  target: "db.example.com:5432",
  looked_up: null,
  said: "a connection to an internet host this project does not allow",
  chat: "fix the build",
  session: 4,
  at: 1_790_000_000,
  times: 3,
  reached: false,
  levels: [],
};

function drawn(row: BlockedLately) {
  mockIPC(() => null);
  const open = vi.fn();
  render(
    <BlockedMarkers.Provider value={{ of: () => undefined, open }}>
      <RefusedRows plane="/home/dev/plane" rows={[row]} label="Blocked log" />
    </BlockedMarkers.Provider>,
  );
  return open;
}

describe("a Blocked log row's way to its chat's Chat log (R-226)", () => {
  it("opens the chat log of the chat it names by its number here", async () => {
    const open = drawn(ROW);
    await userEvent.click(
      screen.getByRole("button", { name: "Open the chat log of fix the build" }),
    );
    expect(open).toHaveBeenCalledWith(4, "blocks");
  });

  it("offers no chat log where an older core names no number for its chat", () => {
    // An older build's answer has no `session` at all: undefined, not null.
    const older: Partial<BlockedLately> = { ...ROW };
    delete older.session;
    drawn(older as BlockedLately);
    expect(screen.queryByRole("button", { name: /^Open the chat log/ })).toBeNull();
  });
});
