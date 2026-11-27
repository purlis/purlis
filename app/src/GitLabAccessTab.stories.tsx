// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **GitLab access** (#1349): the states a person meets the tab in. The core is faked at its command boundary. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { GitlabShown } from "./bindings";
import { GitLabAccessTab } from "./GitLabAccessTab";

const NONE: GitlabShown = {
  rows: [{ host: "git.acme.internal", personas: [], by_hand: [] }],
  personas: ["steward", "reviewer"],
};
const SET_UP: GitlabShown = {
  rows: [{ host: "git.acme.internal", personas: ["steward"], by_hand: [] }],
  personas: ["steward", "reviewer"],
};

function fake(shown: GitlabShown) {
  mockIPC((cmd) => {
    if (cmd === "gitlab_access_list") return shown;
    if (cmd === "gitlab_access_lines") {
      return '[sandbox.personas.steward.tools]\nglab = { vault = "steward", as = "GITLAB_TOKEN env" }';
    }
    throw new Error(`nothing fakes ${cmd}`);
  });
}

const meta: Meta<typeof GitLabAccessTab> = {
  title: "Views/GitLab access",
  component: GitLabAccessTab,
  args: { plane: "p" },
};
export default meta;
type Story = StoryObj<typeof GitLabAccessTab>;

export const NotSetUp: Story = {
  beforeEach: () => fake(NONE),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("checkbox", { name: "steward" }));
    await expect(await canvas.findByText(/GITLAB_TOKEN env/)).toBeTruthy();
  },
};

export const SetUp: Story = {
  beforeEach: () => fake(SET_UP),
  play: async ({ canvasElement }) => {
    await expect(await within(canvasElement).findByText("Set up")).toBeTruthy();
  },
};
