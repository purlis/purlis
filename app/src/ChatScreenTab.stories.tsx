// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * ***Read it*'s tab** (#6214): a closed task's kept last screen, drawn read-only by the
 * terminal's read-only mode, and the tab where no screen was kept. In Chromium and WebKit, in
 * every cell of the theme matrix, with axe. Its keyboard path reaches the screen in one Tab
 * stop and finds a word in it.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { expect, screen, userEvent, waitFor, within } from "storybook/test";
import type { PlaneId } from "./bindings";
import { ChatScreenTab } from "./ChatScreenTab";
import { onAMac } from "./mac";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const PLANE = "/projects/alpha" as unknown as PlaneId;
const CHAT = "01K6W0RKER000000000000000B";

/** What chat #641 showed as it closed. */
const KEPT = [
  "> sign the commits on every lane",
  "",
  "\u001b[1m● Bash(git log --show-signature -12)\u001b[0m",
  "  ⎿ 12 commits, each signed with the machine's SSH key",
  "",
  "Done. Commits are signed on every lane, and CI checks each signature.",
  "",
  "\u001b[2m● purlis dispatch report --outcome done\u001b[22m",
].join("\r\n");

/** The core answering `chat_screen` with `screen`. */
function screenIs(screen: string | null) {
  return () => {
    mockIPC((cmd) => (cmd === "chat_screen" ? screen : null));
    return () => clearMocks();
  };
}

const meta = {
  title: "Chats/Closed chat screen",
  component: ChatScreenTab,
  args: { plane: PLANE, chat: CHAT, title: "#641 · Signed commits 5607 (read-only)" },
  parameters: { screenshot: "page" },
} satisfies Meta<typeof ChatScreenTab>;
export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **The kept screen**, read-only: one Tab stop on the screen, named by the tab's title, and ⌘F
 * (Ctrl+F) finds a word on it.
 */
export const KeptScreen: Story = {
  beforeEach: screenIs(KEPT),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const region = await canvas.findByRole("region", { name: "The chat's last screen" });
    // Its text is the list beside the grid, line by line, and the grid draws it too.
    await waitFor(() =>
      expect(within(region).getAllByText(/Commits are signed/).length).toBeGreaterThan(0),
    );
    await userEvent.tab();
    await untilFocused(() => region, "Tab to the screen");
    expectFocusInSight();
    await userEvent.keyboard(onAMac() ? "{Meta>}f{/Meta}" : "{Control>}f{/Control}");
    const find = await screen.findByRole("searchbox", { name: "Find" });
    await untilFocused(() => find, "the find bar takes the keyboard");
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => region, "Esc hands the keyboard back to the screen");
  },
};

/** **No screen kept**: the tab says so, and why. */
export const NotKept: Story = {
  beforeEach: screenIs(null),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("This chat's screen is not kept")).toBeVisible();
    await expect(canvas.getByText(/\[task_screens\] keep/)).toBeVisible();
  },
};
