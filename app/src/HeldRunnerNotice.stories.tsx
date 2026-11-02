// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A runner purlis kept with its chats' conversations** (#4633): the Notice in Devices that offers
 * Bring back conversations and Discard…, Discard asking twice, and what a bring back that did not
 * finish says. Drawn in every cell of the theme matrix, light and dark among them, each held to
 * axe and walked by the keyboard. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { HeldRunnerNotice } from "./HeldRunnerNotice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** What the faked core says to a bring back: nothing, or why it did not finish. */
let bringBackSays: string | undefined;

function Devices() {
  return (
    <div className="devices-view" style={{ inlineSize: 720 }}>
      <HeldRunnerNotice runner="builds" pool="builds" onDone={() => undefined} />
    </div>
  );
}

const meta = {
  component: Devices,
  beforeEach: () => {
    bringBackSays = undefined;
    mockIPC((cmd) => {
      if (cmd === "docker_pool_bring_back_held_runner") {
        if (bringBackSays !== undefined) throw bringBackSays;
        return null;
      }
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Devices>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Kept**: the line, with its two ways out, each reached by Tab. */
export const Kept: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/purlis kept runner builds of pool builds/);
    const back = canvas.getByRole("button", { name: "Bring back conversations" });
    const discard = canvas.getByRole("button", { name: "Discard…" });
    await userEvent.tab();
    await untilFocused(() => back, "Bring back conversations");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => discard, "Discard…");
    expectFocusInSight();
  },
};

/** **Discard… pressed**: the first ask, Esc the way back, and the keyboard back on Discard…. */
export const DiscardAsksFirst: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const discard = await canvas.findByRole("button", { name: "Discard…" });
    discard.focus();
    await userEvent.keyboard("{Enter}");
    const page = within(canvasElement.ownerDocument.body);
    const asking = await page.findByRole("alertdialog", {
      name: "Discard runner builds's conversations?",
    });
    await untilFocused(
      () => within(asking).getByRole("button", { name: "Cancel" }),
      "the first ask's Cancel",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => discard, "Discard…, once Esc closed the ask");
    await expect(page.queryByRole("alertdialog")).toBeNull();
  },
};

/** **Discard… pressed, then Discard… again**: the second ask, before anything is removed. */
export const DiscardAsksAgain: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const discard = await canvas.findByRole("button", { name: "Discard…" });
    discard.focus();
    await userEvent.keyboard("{Enter}");
    const page = within(canvasElement.ownerDocument.body);
    const first = await page.findByRole("alertdialog", {
      name: "Discard runner builds's conversations?",
    });
    within(first).getByRole("button", { name: "Discard…" }).focus();
    await userEvent.keyboard("{Enter}");
    const again = await page.findByRole("alertdialog", { name: "Discard them for good?" });
    // It opens on Cancel, never on the button the first ask was answered with.
    await untilFocused(
      () => within(again).getByRole("button", { name: "Cancel" }),
      "the second ask's Cancel",
    );
    expectFocusInSight();
    await expect(again).toHaveTextContent("There is no undo.");
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => discard, "Discard…, once Esc closed the second ask");
    await expect(page.queryByRole("alertdialog")).toBeNull();
  },
};

/** **A bring back that did not finish**: why, under the line, which stays. */
export const BringBackDidNotFinish: Story = {
  play: async ({ canvas, userEvent }) => {
    bringBackSays =
      "3 files of 5 files of its chats' conversations came back. purlis keeps runner builds with the 2 files left. 2 files of them opencode's, which stay where they are. Discard them to remove it.";
    const back = await canvas.findByRole("button", { name: "Bring back conversations" });
    back.focus();
    await userEvent.keyboard("{Enter}");
    await canvas.findByText(/Bringing back runner builds's conversations did not finish:/);
    await expect(canvas.getByRole("button", { name: "Bring back conversations" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Dismiss" })).toBeVisible();
  },
};
