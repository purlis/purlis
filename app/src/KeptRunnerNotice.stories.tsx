// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **An older runner purlis keeps for work it can't bring back** (ruled 2026-11-07): the Notice in
 * Devices that offers Let it go…, the ask that names what is lost, and what a let go that did not
 * happen says; and one kept unchecked (ruled 2026-11-08), stopped or with only its saved home left,
 * with its own line and ask. Drawn in every cell of the theme matrix, light and dark among them, each held to axe
 * and walked by the keyboard. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { NotChecked } from "./bindings";
import { KeptRunnerNotice } from "./KeptRunnerNotice";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** What the faked core says to a let go: nothing, or why it did not happen. */
let letGoSays: string | undefined;

const LEFT = [
  "in project alpha, the project's own repository on the runner holds 1 stashed change (refs/stash), which Fetch again does not bring back",
  "in project alpha, HEAD of the repository workspaces/app/web on the runner is at a commit no ref here reaches",
];

function Devices({ notChecked }: { notChecked?: NotChecked }) {
  return (
    <div className="devices-view" style={{ inlineSize: 720 }}>
      <KeptRunnerNotice
        runner="builds"
        pool="builds"
        left={notChecked === undefined ? LEFT : []}
        notChecked={notChecked}
        onDone={() => undefined}
      />
    </div>
  );
}

const meta = {
  component: Devices,
  beforeEach: () => {
    letGoSays = undefined;
    mockIPC((cmd) => {
      if (cmd === "docker_pool_let_runner_go") {
        if (letGoSays !== undefined) throw letGoSays;
        return null;
      }
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Devices>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Kept**: the line, with its way out, reached by Tab. */
export const Kept: Story = {
  play: async ({ canvas, userEvent }) => {
    await canvas.findByText(/purlis keeps older runner builds of pool builds/);
    const letGo = canvas.getByRole("button", { name: "Let it go…" });
    await userEvent.tab();
    await untilFocused(() => letGo, "Let it go…");
    expectFocusInSight();
  },
};

/** **Let it go… pressed**: the ask names what is lost, opens on Cancel, and Esc is the way back. */
export const LetItGoAsks: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const letGo = await canvas.findByRole("button", { name: "Let it go…" });
    letGo.focus();
    await userEvent.keyboard("{Enter}");
    const page = within(canvasElement.ownerDocument.body);
    const asking = await page.findByRole("alertdialog", { name: "Let runner builds go?" });
    await expect(asking).toHaveTextContent("1 stashed change");
    await expect(asking).toHaveTextContent("Pool builds and its runner now stay as they are.");
    await untilFocused(
      () => within(asking).getByRole("button", { name: "Cancel" }),
      "the ask's Cancel",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => within(asking).getByRole("button", { name: "Let it go" }),
      "the ask's Let it go",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => letGo, "Let it go…, once Esc closed the ask");
    await expect(page.queryByRole("alertdialog")).toBeNull();
  },
};

/** **A let go that did not happen**: why, under the line, which stays. */
export const NotLetGo: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    letGoSays = "1 chat still runs on runner builds, so it stays until it ends.";
    const letGo = await canvas.findByRole("button", { name: "Let it go…" });
    letGo.focus();
    await userEvent.keyboard("{Enter}");
    const page = within(canvasElement.ownerDocument.body);
    const asking = await page.findByRole("alertdialog", { name: "Let runner builds go?" });
    within(asking).getByRole("button", { name: "Let it go" }).focus();
    await userEvent.keyboard("{Enter}");
    await canvas.findByText(/Runner builds was not let go:/);
    await expect(canvas.getByRole("button", { name: "Let it go…" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Dismiss" })).toBeVisible();
  },
};

/** **Kept unchecked, stopped**: its own line, which says purlis checks it once it runs again, and
 *  its ask, which says what goes unseen. Opens on Cancel; Esc is the way back. */
export const NotCheckedStopped: Story = {
  args: { notChecked: "stopped" },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await canvas.findByText(/since it stopped before purlis could check it/);
    const letGo = canvas.getByRole("button", { name: "Let it go…" });
    await userEvent.tab();
    await untilFocused(() => letGo, "Let it go…");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const page = within(canvasElement.ownerDocument.body);
    const asking = await page.findByRole("alertdialog", { name: "Let runner builds go?" });
    await expect(asking).toHaveTextContent("purlis never checked them");
    await untilFocused(
      () => within(asking).getByRole("button", { name: "Cancel" }),
      "the ask's Cancel",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => letGo, "Let it go…, once Esc closed the ask");
  },
};

/** **Kept unchecked, only its saved home left**: the line and the ask name the home, never a
 *  container that is gone. Opens on Cancel; Esc is the way back. */
export const NotCheckedHomeOnly: Story = {
  args: { notChecked: "home_only" },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await canvas.findByText(/purlis keeps the saved home of older runner builds/);
    const letGo = canvas.getByRole("button", { name: "Let it go…" });
    letGo.focus();
    await userEvent.keyboard("{Enter}");
    const page = within(canvasElement.ownerDocument.body);
    const asking = await page.findByRole("alertdialog", { name: "Let runner builds go?" });
    await expect(asking).toHaveTextContent("Its saved home is removed from your Docker.");
    await untilFocused(
      () => within(asking).getByRole("button", { name: "Cancel" }),
      "the ask's Cancel",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => letGo, "Let it go…, once Esc closed the ask");
  },
};
