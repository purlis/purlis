// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Attention** (#6019, ADR 0121 decision 22): Quiet, Remind me (#6344), the notifications
 * per source, the Expiring sound and the project's Stuck? thresholds on one page. Drawn as it opens, with long
 * finishes turned off, and in a project with its Stuck? row; held to axe, walked by the keyboard
 * and pictured in every pictured theme cell (`docs/component-stories.md`), the high-contrast
 * appearances among them; a pull request's `stories` run draws those pictures, never a hand
 * (#2901). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DispatchLimits } from "../bindings";
import { notificationPrefs, setSourceOn } from "../notificationPrefs";
import { forgetThisLaunch } from "../regions";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { SettingGroup } from "./components";
import { LiveRow } from "./SettingsTab";
import type { LiveSetting } from "./groups";
import { notificationsGroup } from "./notifications";

/** The core's dispatch limits, the three Stuck? thresholds among them, the project setting one. */
const LIMITS: DispatchLimits = {
  file: "purlis.toml",
  local_file: "purlis.local.toml",
  base: "schema = 1\n",
  local_base: null,
  limits: [
    ["running-per-chat", "Running per chat", 6],
    ["stuck-silent-minutes", "Silent minutes", 15],
    ["stuck-failing-times", "Failing times", 3],
    ["stuck-long-minutes", "Turn minutes", 60],
  ].map(([word, label, standard]) => ({
    word: word as string,
    label: label as string,
    help: `When a working chat reads Stuck?: ${String(label).toLowerCase()}.`,
    default: standard as number,
    persona_only: false,
    most: 10000,
    ceiling: null,
  })),
  rows: [
    {
      scope: "project",
      name: "",
      values: [null, 20, null, null],
      beneath: [6, 15, 3, 60],
      ignored: [],
    },
  ],
  mine: [],
  locked_by: null,
  refused: [],
  local_left_out: null,
  policy_refused: false,
};

/** The Attention group as Settings › You draws it, in a project when the story says so. */
function Attention({ plane }: { plane?: string }) {
  const group = notificationsGroup(plane);
  return (
    <SettingGroup label={group.label} help={group.help}>
      {group.settings.map((one) => (
        <LiveRow key={one.id} setting={one as LiveSetting} />
      ))}
    </SettingGroup>
  );
}

const meta = {
  component: Attention,
  beforeEach: ({ parameters }) => {
    forgetThisLaunch();
    mockIPC((cmd) => {
      if (cmd === "dispatch_limits") return LIMITS;
      if (cmd === "quiet_now") return { quiet: false, by_hand: false, ended: null };
      if (cmd === "start_options") return { profiles: [], refused: [], personas: [] };
      if (cmd === "plane_sidebar") return { workspaces: [] };
      return null;
    });
    if (parameters.longFinishesOff === true) setSourceOn("task-done", false);
    return () => clearMocks();
  },
  decorators: [
    (Story) => (
      <div style={{ inlineSize: 720 }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Attention>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **As it opens**: quiet hours off and their times out of reach, both sources on, the sound off,
 * and asks that stop a chat said to always notify, with nothing to turn off. Tab reaches the
 * quiet hours' switch and Space turns them on.
 */
export const AsItOpens: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Attention" });
    await expect(within(group).getByText("Always notify")).toBeInTheDocument();
    await expect(
      within(group).getByRole("switch", { name: "Notify when a task fails" }),
    ).toBeChecked();
    await expect(
      within(group).getByRole("switch", { name: "Sound for an expiring ask" }),
    ).not.toBeChecked();
    await userEvent.tab();
    await untilFocused(
      () => within(group).getByRole("switch", { name: "Quiet hours" }),
      "Tab reaches Quiet hours",
    );
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(notificationPrefs().quiet.on).toBe(true));
  },
};

/**
 * **Remind me**: Gently picked as it opens, each pace saying what it does. The arrow keys move
 * the pick within the group, from Gently to Often, which is kept.
 */
export const RemindMe: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Attention" });
    const remind = within(group).getByRole("radiogroup", { name: "Remind me" });
    const gently = within(remind).getByRole("radio", { name: /^Gently/ });
    await expect(gently).toBeChecked();
    await expect(within(remind).getByText("Every 5 minutes.")).toBeInTheDocument();
    gently.focus();
    await untilFocused(() => gently, "Gently takes the keyboard");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(notificationPrefs().remind).toBe("often"));
    await untilFocused(
      () => within(remind).getByRole("radio", { name: /^Often/ }),
      "the arrow moves the pick to Often",
    );
  },
};

/**
 * **Long finishes off**: the minutes that make a finish long are out of reach; turning long
 * finishes back on from the keyboard brings them back.
 */
export const LongFinishesOff: Story = {
  parameters: { longFinishesOff: true },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Attention" });
    const minutes = within(group).getByRole("textbox", { name: "Long means more than" });
    await expect(minutes).toBeDisabled();
    const long = within(group).getByRole("switch", { name: "Notify when a long task finishes" });
    long.focus();
    await untilFocused(() => long, "the long finishes' switch takes the keyboard");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(minutes).toBeEnabled());
  },
};

/**
 * **In a project**: the Stuck? row is the project's, its table holding the three thresholds and
 * no dispatch limit. Tab reaches each threshold's heading, whose tooltip holds its help, and then
 * the project's silent minutes.
 */
export const InAProject: Story = {
  args: { plane: "/home/dev/plane" },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Attention" });
    const table = await within(group).findByRole("table", { name: "Stuck? thresholds" });
    await expect(within(table).queryByText("Running per chat")).toBeNull();
    const silent = within(table).getByLabelText("Silent minutes for the project");
    await expect(silent).toHaveValue("20");
    within(group).getByRole("switch", { name: "Sound for an expiring ask" }).focus();
    for (const heading of ["Silent", "Failing", "Turn"]) {
      await userEvent.tab();
      await untilFocused(
        () => within(table).getByText(heading, { selector: "span" }),
        `Tab reaches the ${heading} heading`,
      );
      expectFocusInSight();
    }
    await userEvent.tab();
    await untilFocused(() => silent, "Tab reaches the project's silent minutes");
    expectFocusInSight();
  },
};
