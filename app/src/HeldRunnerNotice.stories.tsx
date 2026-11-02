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
    await untilFocused(() => asking, "the first ask");
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
    await untilFocused(() => again, "the second ask");
    expectFocusInSight();
    await expect(again).toHaveTextContent("There is no undo.");
  },
};

/** **A bring back that did not finish**: why, under the line, which stays. */
export const BringBackDidNotFinish: Story = {
  play: async ({ canvas, userEvent }) => {
    bringBackSays =
      "1 of 2 conversations came back. The rest could not, so purlis keeps runner builds with them: opencode's conversations always stay where they are. Discard them to remove it.";
    const back = await canvas.findByRole("button", { name: "Bring back conversations" });
    back.focus();
    await userEvent.keyboard("{Enter}");
    await canvas.findByText(/Bringing back runner builds's conversations did not finish:/);
    await expect(canvas.getByRole("button", { name: "Bring back conversations" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Dismiss" })).toBeVisible();
  },
};
