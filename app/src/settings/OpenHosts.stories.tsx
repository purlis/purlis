// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Settings › Sandbox › Network, Open hosts** (#1662), with the *Model providers* preset on: each
 * preset host by host, and under them that a chat on a key needs no host here, since its calls go
 * through purlis's key proxy and show as its own (R-411, W79, T1a-19). Drawn, held to axe and
 * pictured in every pictured theme cell (`docs/component-stories.md`); the core is faked at its
 * command boundary. Nothing on it takes the keyboard.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SandboxNetwork } from "../bindings";
import { windowCopy } from "../windowCopy";
import { SettingGroup, SettingRow } from "./components";
import { networkGroup } from "./GrantedList";
import type { LiveSetting } from "./groups";

const NETWORK: SandboxNetwork = {
  on: true,
  open: [
    { title: "Model providers", hosts: ["api.anthropic.com", "api.openai.com"] },
    { title: "This project's hosts", hosts: ["registry.internal"] },
  ],
  blocked: [],
};

/** The Network page's Open hosts row, as the Settings tab draws a live setting. */
function OpenHosts() {
  const setting = networkGroup("/home/op/project", "charter.toml").settings.find(
    (one) => one.id === "project.sandbox.network.open",
  ) as LiveSetting;
  const { control, grouped, error } = setting.useControl();
  return (
    <div style={{ inlineSize: 720 }}>
      <SettingGroup label="Network">
        <SettingRow
          setting={setting.id}
          label={setting.label}
          help={setting.help}
          grouped={grouped}
          error={error}
          control={control}
        />
      </SettingGroup>
    </div>
  );
}

const meta = {
  component: OpenHosts,
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "sandbox_network" ? NETWORK : null));
    return () => clearMocks();
  },
} satisfies Meta<typeof OpenHosts>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A chat on a key needs no host here** (R-411): W79, under the presets. */
export const NetworkPageSaysKeyChatsNeedNoHost: Story = {
  play: async ({ canvas }) => {
    await canvas.findByRole("list", { name: "Model providers" });
    await expect(canvas.getByText(windowCopy.W79)).toBeVisible();
  },
};
