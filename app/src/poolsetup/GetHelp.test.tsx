// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { FIX_STARTED } from "../saving";
import { GetHelp, PoolSetupPlane } from "./GetHelp";

afterEach(() => {
  cleanup();
  clearMocks();
});

const CHAT = {
  session: 7,
  name: "7",
  label: "Help: Check the cluster",
  persona: null,
  harness: "claude",
  workspace: null,
  existing: false,
};

describe("Get help (#4497)", () => {
  it("starts a chat about the step and the error shown, in the tab's project, and opens it", async () => {
    const sent: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      sent.push({ cmd, args });
      return cmd === "pool_setup_get_help" ? CHAT : null;
    });
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent).detail);
    window.addEventListener(FIX_STARTED, heard);
    try {
      render(
        <PoolSetupPlane.Provider value="/home/ada/team">
          <GetHelp step="check" error="kubectl said: forbidden" />
        </PoolSetupPlane.Provider>,
      );
      await userEvent.click(screen.getByRole("button", { name: "Get help" }));
      await waitFor(() => expect(opened).toEqual([{ plane: "/home/ada/team", chat: CHAT }]));
    } finally {
      window.removeEventListener(FIX_STARTED, heard);
    }
    expect(sent).toContainEqual({
      cmd: "pool_setup_get_help",
      args: {
        plane: "/home/ada/team",
        step: "check",
        error: "kubectl said: forbidden",
        columns: 80,
        rows: 24,
      },
    });
  });

  it("says why no chat started, where the core refused one", async () => {
    mockIPC((cmd) => {
      if (cmd === "pool_setup_get_help")
        throw "This project runs chats without the sandbox, and Get help opens a chat only under it.";
      return null;
    });
    render(
      <PoolSetupPlane.Provider value="/home/ada/team">
        <GetHelp step="check" />
      </PoolSetupPlane.Provider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Get help" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "No chat was started: This project runs chats without the sandbox",
    );
  });

  it("is drawn only inside Pool setup's tab, which names its project", () => {
    render(<GetHelp step="check" />);
    expect(screen.queryByRole("button", { name: "Get help" })).toBeNull();
  });
});
