// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { aWindow, rootOf, settled, windowCommits, type Asked } from "./projectSwitch.testkit";
import { keepTheirOrder } from "./commandOrder";

keepTheirOrder();

/**
 * **The sidebar is read again only when what it answers can have changed** (#4729).
 *
 * It was read on every change of the window's tabs, a tab brought to the front among them, so
 * each switch between chats sent the core a read that walks the plane, and each answer, equal or
 * not, drew the window again. It is read when the set of chats open here changes, or when the
 * core says the sidebar changed, and an answer equal to the one held is not drawn.
 *
 * Held by counts, the same on every machine (ADR 0086).
 */

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const SHAPE = { projects: 1, chats: 3, workspaces: 1 };

const sidebarReads = (asked: readonly Asked[], from: number) =>
  asked.slice(from).filter((one) => one.cmd === "plane_sidebar").length;

/** A chat's tab on the strip, named after its persona and then the chat's own name, before the
 *  state it is in. */
const chatTab = (chat: string) =>
  screen.getByRole("tab", { name: (name) => name.startsWith(`steward ${chat}`) });

describe("when the window reads the sidebar again", () => {
  it("reads nothing for a switch between chats already open", async () => {
    const { asked } = await aWindow(SHAPE);
    const from = asked.length;
    fireEvent.click(chatTab("steward 2"));
    await settled(asked);
    fireEvent.click(chatTab("steward 3"));
    await settled(asked);

    expect(chatTab("steward 3")).toHaveAttribute("aria-selected", "true");
    expect(sidebarReads(asked, from)).toBe(0);
  });

  it("commits nothing for an answer equal to the one it holds", async () => {
    const { asked, planeChanged, holdSidebar } = await aWindow(SHAPE);
    const release = holdSidebar();
    const from = asked.length;
    await planeChanged(rootOf(0), "sidebar");
    await settled(asked);
    expect(sidebarReads(asked, from)).toBe(1);

    windowCommits.count = 0;
    await act(async () => {
      release();
    });
    await settled(asked);

    expect(windowCommits.count).toBe(0);
  });

  it("reads it again when a chat is opened", async () => {
    const { asked } = await aWindow(SHAPE);
    const from = asked.length;

    fireEvent.click(screen.getAllByRole("button", { name: "Split right" })[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Start chat" }));
    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(2));
    await settled(asked);

    expect(sidebarReads(asked, from)).toBeGreaterThan(0);
  });

  it("reads it again when a chat is closed", async () => {
    const { asked } = await aWindow(SHAPE);
    const from = asked.length;

    fireEvent.click(screen.getByRole("button", { name: "End chat steward steward 3" }));
    const asking = await screen.findByRole("alertdialog", { name: "End chat steward steward 3?" });
    fireEvent.click(within(asking).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await waitFor(() =>
      expect(
        screen.queryByRole("tab", { name: (name) => name.startsWith("steward steward 3") }),
      ).toBeNull(),
    );
    await settled(asked);

    expect(sidebarReads(asked, from)).toBeGreaterThan(0);
  });
});
