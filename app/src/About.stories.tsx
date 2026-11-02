// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **About purlis**, with **Copy diagnostics** (#4774): the dialog the title bar's About opens,
 * drawn with the core faked at its command boundary and the clipboard held in the story, held to
 * axe, walked by the keyboard, and pictured in every pictured theme cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AboutCharter } from "./About";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

/** What the story's clipboard was given last. */
let pasted = "";

const meta = {
  component: AboutCharter,
  // A dialog is drawn over the page, outside the story's frame: the page is pictured.
  parameters: { screenshot: "page" },
  beforeEach: () => {
    pasted = "";
    mockIPC((cmd) => {
      if (cmd === "about_charter")
        return {
          version: "1.60.0",
          build: { kind: "release" },
          profile: "release",
          notes: { version: "1.60.0", date: "2026-11-02", markdown: "### Added\n\n- A thing." },
        };
      if (cmd === "diagnostics_here") return { os: "Darwin 27.2.0, aarch64", open: 12, hot: 4 };
      if (cmd === "machine_profiles") return [];
      return null;
    });
    // The page's own clipboard asks the browser for leave a story cannot give, so it is held.
    const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          pasted = text;
          return Promise.resolve();
        },
      },
    });
    return () => {
      clearMocks();
      if (clipboard) Object.defineProperty(navigator, "clipboard", clipboard);
      else Reflect.deleteProperty(navigator, "clipboard");
    };
  },
} satisfies Meta<typeof AboutCharter>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Copy diagnostics** by the keyboard: from Close to it, Return copies and says so, Esc closes. */
export const CopyDiagnostics: Story = {
  play: async ({ canvasElement }) => {
    await userEvent.click(within(canvasElement).getByTestId("title-about"));
    const dialog = await waitFor(() =>
      within(document.body).getByRole("dialog", { name: "About purlis" }),
    );
    const close = within(dialog).getByRole("button", { name: "Close" });
    await untilFocused(() => close, "the dialog starts on Close");
    expectFocusInSight();

    const copy = within(dialog).getByRole("button", { name: "Copy diagnostics" });
    await userEvent.tab();
    await untilFocused(() => copy, "Tab reaches Copy diagnostics");
    expectFocusInSight();

    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(within(dialog).getByRole("status")).toHaveTextContent("Diagnostics copied."),
    );
    await expect(pasted).toContain("Chats: 12 open, 4 hot");
    // Pictured with its confirmation said, before Esc takes the dialog away.
    await pictureNow();

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(within(document.body).queryByRole("dialog")).toBeNull());
  },
};
