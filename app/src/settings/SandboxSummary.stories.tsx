// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Settings › Project › Sandbox, the sentence at the top** (#1340), with the *Model providers*
 * preset on (R-019, R-661, T1a-19): the core names it *Model providers*, and the sentence says it
 * mid-sentence as *model providers*. Drawn, held to axe and pictured in every pictured theme cell
 * (`docs/component-stories.md`). Nothing on it takes the keyboard.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import type { SandboxPreset } from "../bindings";
import { SettingGroup } from "./components";
import { whatAChatCanDo } from "./sandbox";

/** The core's `model-providers` preset, as `sandbox_state` hands it (`Preset::title`). */
const MODEL_PROVIDERS: SandboxPreset = {
  word: "model-providers",
  title: "Model providers",
  hosts: ["api.anthropic.com", "api.openai.com"],
  widens_caches: false,
};

const CODE: SandboxPreset = {
  word: "forge",
  title: "Code hosting",
  hosts: ["github.com"],
  widens_caches: false,
};

/** The Sandbox page's head: its title and the sentence the page draws as its help. */
function SandboxHead({ presets }: { presets: readonly SandboxPreset[] }) {
  return (
    <div style={{ inlineSize: 640 }}>
      <SettingGroup
        label="Sandbox"
        help={whatAChatCanDo({
          on: true,
          presets: [...presets],
          projectHosts: 0,
          yourHosts: 0,
          folders: 0,
          certificateService: false,
        })}
      >
        {null}
      </SettingGroup>
    </div>
  );
}

const meta = {
  component: SandboxHead,
  args: { presets: [MODEL_PROVIDERS] },
} satisfies Meta<typeof SandboxHead>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **The summary names model providers** (R-661): *"…and reach model providers"*. */
export const SandboxPageSummaryNamesModelProviders: Story = {
  args: { presets: [MODEL_PROVIDERS, CODE] },
  play: async ({ canvas }) => {
    await expect(
      canvas.getByText(
        "A chat here can change files in the folder it works in, and reach model providers and code hosting.",
      ),
    ).toBeVisible();
  },
};
