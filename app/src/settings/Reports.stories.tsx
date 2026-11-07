// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Reports** (#5688, spec #5683): the two answers as Settings holds them, unanswered,
 * all on and all off, with "See exactly what's sent…". Drawn, held to axe, walked by the keyboard
 * and pictured in every pictured theme cell (`docs/component-stories.md`). The core is faked at
 * its command boundary, keeping what it is told.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReportConsent } from "../bindings";
import { forgetReportConsent, UNANSWERED } from "../reportAnswers";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { SettingGroup, SettingRow } from "./components";
import type { LiveSetting } from "./groups";
import { reportsGroup } from "./reports";

let kept: ReportConsent = UNANSWERED;

/** One row, as the Settings tab draws a live setting. */
function Row({ setting }: { setting: LiveSetting }) {
  const { control, grouped, error } = setting.useControl();
  return (
    <SettingRow
      setting={setting.id}
      label={setting.label}
      help={setting.help}
      grouped={grouped}
      error={error}
      control={control}
    />
  );
}

/** The Reports group as Settings › You draws it. */
function Reports() {
  const group = reportsGroup();
  return (
    <SettingGroup label={group.label} help={group.help}>
      {group.settings.map((one) => (
        <Row key={one.id} setting={one as LiveSetting} />
      ))}
    </SettingGroup>
  );
}

const meta = {
  component: Reports,
  // What this machine keeps: a story's `parameters.consent`, nothing answered by default.
  beforeEach: ({ parameters }) => {
    kept = (parameters.consent as ReportConsent | undefined) ?? UNANSWERED;
    forgetReportConsent();
    mockIPC((cmd, args) => {
      if (cmd === "report_consent") return kept;
      if (cmd === "keep_report_consent") kept = (args as { consent: ReportConsent }).consent;
      return null;
    });
    return () => clearMocks();
  },
  decorators: [
    (Story) => (
      <div style={{ inlineSize: 720 }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Reports>;

export default meta;
type Story = StoryObj<typeof meta>;

const radios = (group: HTMLElement, name: string) =>
  within(within(group).getByRole("radiogroup", { name })).getAllByRole("radio");

/** **Unanswered**: neither choice made for the person; choosing one keeps it. */
export const Unanswered: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Reports" });
    for (const stream of ["Crash reports", "Agent drafts"])
      for (const radio of radios(group, stream)) await expect(radio).not.toBeChecked();
    await userEvent.tab();
    await untilFocused(
      () => within(group).getByRole("radio", { name: "Send" }),
      "Tab reaches Crash reports",
    );
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(kept.crash_reports).toBe(true));
    await expect(kept.agent_drafts).toBeNull();
  },
};

/** **All on**: Send and Allow chosen; Tab walks on to See exactly what's sent…. */
export const AllOn: Story = {
  parameters: { consent: { crash_reports: true, agent_drafts: true, asked: true } },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Reports" });
    await waitFor(() => expect(within(group).getByRole("radio", { name: "Send" })).toBeChecked());
    await expect(within(group).getByRole("radio", { name: "Allow" })).toBeChecked();
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(
      () => within(group).getByRole("button", { name: "See exactly what's sent…" }),
      "Tab reaches See exactly what's sent…",
    );
    expectFocusInSight();
  },
};

/** **All off**: Don't send and Don't allow chosen; Allow from the keyboard turns drafts on. */
export const AllOff: Story = {
  parameters: { consent: { crash_reports: false, agent_drafts: false, asked: true } },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Reports" });
    await waitFor(() =>
      expect(within(group).getByRole("radio", { name: "Don't send" })).toBeChecked(),
    );
    const dont = within(group).getByRole("radio", { name: "Don't allow" });
    await expect(dont).toBeChecked();
    dont.focus();
    await untilFocused(() => dont, "Don't allow takes the keyboard");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(kept.agent_drafts).toBe(true));
  },
};
