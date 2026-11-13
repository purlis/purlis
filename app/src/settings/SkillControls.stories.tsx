// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Settings › Skill sources and Recorded skill acts** (#6031, `prototype/skill-policy` A): an
 * org's `[skill_policy]` read only, each key set in charter.toml, then the skill acts this machine
 * recorded as a table. Drawn, held to axe, walked by the keyboard and pictured in every pictured
 * theme cell (`docs/component-stories.md`). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { expectFocusInSight, untilFocusWithin } from "../stories.testkit";
import { SettingGroup } from "./components";
import { LiveRow, ShownWhile } from "./SettingsTab";
import type { LiveSetting, SettingsGroup } from "./groups";
import { projectSkillActsGroup, projectSkillSourcesGroup } from "./skillsSummary";

const SET = {
  file: "charter.toml",
  state: {
    kind: "set",
    allow: [
      "https://github.com/acme/skills",
      "pdf from https://github.com/anthropics/skills",
      "the extension acme.registry",
    ],
    block: ["yolo from https://github.com/acme/skills"],
    min_age_days: 7,
    exempt: ["9f3c1d27a0b4e5f6a7b8c9d0e1f2a3b4c5d6e7f8"],
    said: "This project's controls apply: 3 sources allowed, 1 skill blocked, commits at least 7 days old.",
  },
  sources: [],
};

const NONE = { file: "charter.toml", state: { kind: "none" }, sources: [] };

const ACTS = {
  acts: [
    {
      at: Date.parse("2026-11-13T14:02:00Z") / 1000,
      act: "install",
      skills: ["release-train", "notify"],
      source: "https://github.com/acme/skills",
      pin: "4be9c21aa0",
      level: "project",
      by: "window",
    },
    {
      at: Date.parse("2026-11-05T17:15:00Z") / 1000,
      act: "update",
      skills: ["pdf"],
      source: "https://github.com/anthropics/skills",
      pin: "e11b0aa77c",
      level: "project",
      by: "window",
    },
    {
      at: Date.parse("2026-11-02T09:03:00Z") / 1000,
      act: "remove",
      skills: ["lint-fix"],
      source: "https://github.com/acme/skills",
      pin: "1aa20f9b3e",
      level: "persona reviewer",
      by: "terminal",
    },
  ],
  more: false,
};

function Group({ group }: { group: SettingsGroup }) {
  return (
    <SettingGroup label={group.label} help={group.help} kept={group.kept}>
      {group.settings.map((one) => (
        <ShownWhile key={one.id} useShown={(one as LiveSetting).useShown}>
          <LiveRow setting={one as LiveSetting} />
        </ShownWhile>
      ))}
    </SettingGroup>
  );
}

/** The two groups as Settings › Project draws them, in the project `p1`, whose controls are
 *  `controls` (the faked core answers by it). */
function SkillControls({ controls }: { controls: "set" | "none" }) {
  return (
    <div data-controls={controls}>
      <Group group={projectSkillSourcesGroup("p1", "charter.toml")} />
      <Group group={projectSkillActsGroup("p1")} />
    </div>
  );
}

const meta = {
  component: SkillControls,
  args: { controls: "set" },
  beforeEach: ({ args }) => {
    mockIPC((cmd) => {
      if (cmd === "skill_policy") return args.controls === "set" ? SET : NONE;
      if (cmd === "skill_acts") return args.controls === "set" ? ACTS : { acts: [], more: false };
      return null;
    });
    return () => clearMocks();
  },
  decorators: [
    (Story) => (
      <div style={{ inlineSize: 860 }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof SkillControls>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **An org's controls**: each key read only, set in charter.toml, and the acts recorded. Tab
 *  reaches the table of acts. */
export const Set: Story = {
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-policy-allow")).toHaveTextContent(
      "https://github.com/acme/skills",
    );
    const table = await canvas.findByRole("grid", { name: "Recorded skill acts" });
    for (let step = 0; step < 20 && !table.contains(document.activeElement); step++)
      await userEvent.tab();
    await untilFocusWithin(() => table, "Tab reaches the recorded acts");
    expectFocusInSight();
  },
};

/** **A project without controls**: one line says so, and nothing is recorded. */
export const None: Story = {
  args: { controls: "none" },
  play: async ({ canvas }) => {
    await expect(await canvas.findByTestId("skill-policy-controls")).toHaveTextContent(
      "None: charter.toml holds no [skill_policy]",
    );
    await expect(await canvas.findByTestId("skill-acts-none")).toBeInTheDocument();
  },
};
