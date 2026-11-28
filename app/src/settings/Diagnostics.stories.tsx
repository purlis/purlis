// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Diagnostics** (#7864): Detailed remote log for one hour, off and on, turned on and off
 * from the keyboard. Drawn, held to axe, walked by the keyboard and pictured in every pictured
 * theme cell (`docs/component-stories.md`). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DetailedRemoteLog } from "../bindings";
import { expectFocusInSight, tabInto, untilFocused } from "../stories.testkit";
import { diagnosticsGroup, untilSaid } from "./diagnostics";
import { OnItsPage } from "./settingsStory.testkit";

/** When the switch, once on, turns itself off: far enough off that no story sees it happen. */
const UNTIL = "2099-06-01T15:30:00Z";

let kept: DetailedRemoteLog = { until: null };

function Diagnostics() {
  return <OnItsPage group={diagnosticsGroup()} />;
}

const meta = {
  component: Diagnostics,
  // What the core says: a story's `parameters.on`, off by default.
  beforeEach: ({ parameters }) => {
    kept = { until: parameters.on === true ? UNTIL : null };
    mockIPC((cmd, args) => {
      if (cmd === "detailed_remote_log") return kept;
      if (cmd === "set_detailed_remote_log") {
        kept = { until: (args as { on: boolean }).on ? UNTIL : null };
        return kept;
      }
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Diagnostics>;

export default meta;
type Story = StoryObj<typeof meta>;

const theSwitch = (group: HTMLElement) =>
  within(group).getByRole("switch", { name: "Detailed remote log for one hour" });

/** **Off**: Space turns it on for an hour and says until when; Space again turns it off. */
export const Off: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Diagnostics" });
    await expect(theSwitch(group)).not.toBeChecked();
    await expect(within(group).getByRole("status")).toHaveTextContent("Off.");
    await tabInto(() => theSwitch(group));
    await untilFocused(() => theSwitch(group), "Tab reaches the switch");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(theSwitch(group)).toBeChecked());
    await expect(within(group).getByRole("status")).toHaveTextContent(
      `On until ${untilSaid(UNTIL)}, then off.`,
    );
    await expect(kept.until).toBe(UNTIL);
    await userEvent.keyboard(" ");
    await waitFor(() => expect(theSwitch(group)).not.toBeChecked());
    await expect(kept.until).toBeNull();
    expectFocusInSight();
  },
};

/** **On**: the hour it turns itself off at, said beside it. */
export const On: Story = {
  parameters: { on: true },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Diagnostics" });
    await waitFor(() => expect(theSwitch(group)).toBeChecked());
    await expect(within(group).getByRole("status")).toHaveTextContent(
      `On until ${untilSaid(UNTIL)}, then off.`,
    );
  },
};
