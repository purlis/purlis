// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Project › General's rule over Always send** (#5696, spec #5683, Q12): "Members may let agents
 * send reports without asking", a switch over `[reports].always_send` in the project's shared
 * file, which the team sees. On while the file does not set it, as the Settings tab draws an
 * on/off key with its value in force; turned off, Always send is held off for every member.
 * Drawn, held to axe, walked by the keyboard and pictured in every pictured theme cell
 * (`docs/component-stories.md`).
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { Choice, SettingGroup, SettingRow } from "./components";
import { MEMBERS_MAY_ALWAYS_SEND, PROJECT_RULE as RULE } from "./fileControls";

/** The row as the Settings tab draws an on/off key the file holds `on` or `off`, or not set. */
function ProjectRule({ start }: { start: "" | "on" | "off" }) {
  const [value, setValue] = useState(start);
  return (
    <div style={{ inlineSize: 720 }}>
      <SettingGroup
        label="General"
        help="What a chat starts on when nothing else names it, the update channel, the version lock, where branch folders go and what agents may send."
        kept="Kept in purlis.toml, which your team sees."
      >
        <SettingRow
          setting="project.general.reports"
          label={RULE.label}
          help={RULE.hint}
          modified={value !== ""}
          control={(ids) => (
            <Choice
              kind="switch"
              ids={ids}
              checked={value === "" ? RULE.inForce === true : value === "on"}
              onCheckedChange={(on) => setValue(on ? "on" : "off")}
            />
          )}
        />
      </SettingGroup>
    </div>
  );
}

const meta = { component: ProjectRule } satisfies Meta<typeof ProjectRule>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **On by default**: the file does not set it; Space from the keyboard turns it off. */
export const OnByDefault: Story = {
  args: { start: "" },
  play: async ({ canvas, userEvent }) => {
    const rule = await canvas.findByRole("switch", { name: MEMBERS_MAY_ALWAYS_SEND });
    await expect(rule).toBeChecked();
    await userEvent.tab();
    await untilFocused(() => rule, "Tab reaches the rule");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(rule).not.toBeChecked());
  },
};

/** **Turned off**: the team keeps its drafts from going unread. */
export const TurnedOff: Story = {
  args: { start: "off" },
  play: async ({ canvas }) => {
    await expect(
      await canvas.findByRole("switch", { name: MEMBERS_MAY_ALWAYS_SEND }),
    ).not.toBeChecked();
  },
};
