// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Project › Dispatch › Keep task screens** (#6306): the switch on, the ask before it is turned
 * off, and the switch off, on the aligned page. Drawn, held to axe, walked by the keyboard and
 * pictured in every pictured theme cell (`docs/component-stories.md`). The core is faked at its
 * command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { TaskScreens } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import type { SettingsGroup } from "./groups";
import { OnItsPage } from "./settingsStory.testkit";
import { taskScreensSetting } from "./taskScreens";

const PLANE = "/Users/you/projects/charter";

/** The core, faked: keeping `keep`, with `kept` screens kept. */
function theCore(keep: boolean, kept: number) {
  const state: TaskScreens = { keep, kept };
  mockIPC((cmd, args) => {
    if (cmd === "task_screens") return { ...state };
    if (cmd === "set_task_screens") {
      state.keep = (args as { keep: boolean }).keep;
      if (!state.keep) state.kept = 0;
      return { ...state };
    }
    return null;
  });
}

const GROUP: SettingsGroup = {
  id: "project.dispatch",
  label: "Dispatch",
  help: "How many tasks a chat may start, and what a task leaves to read once it closes.",
  settings: [taskScreensSetting(PLANE)],
};

function Page() {
  return <OnItsPage group={GROUP} />;
}

const meta = {
  component: Page,
  beforeEach: ({ parameters }) => {
    theCore((parameters.keep as boolean | undefined) ?? true, 3);
    return () => clearMocks();
  },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The switch, once the core has said what is in force. */
async function theSwitch(canvas: ReturnType<typeof within>) {
  const toggle = await canvas.findByRole("switch", { name: "Keep task screens" });
  await waitFor(() => expect(toggle).not.toBeDisabled());
  return toggle;
}

/**
 * **On, as it is by default**: Tab reaches the switch and Space asks before turning it off. Esc
 * keeps everything and hands the keyboard back to the switch, still on. The picture is the row
 * at rest.
 */
export const KeptOn: Story = {
  play: async ({ canvas, userEvent }) => {
    const toggle = await theSwitch(canvas);
    await expect(toggle).toBeChecked();
    toggle.focus();
    await untilFocused(() => toggle, "the switch");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    const ask = await within(document.body).findByRole("alertdialog");
    await expect(ask).toHaveTextContent("3 kept screens");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(within(document.body).queryByRole("alertdialog")).toBeNull());
    await untilFocused(() => toggle, "Esc hands the keyboard back to the switch");
    await expect(toggle).toBeChecked();
    toggle.blur();
  },
};

/**
 * **The ask before turning it off**: what goes and what stays, and Delete kept screens in the
 * danger look. The picture is the ask.
 */
export const AsksBeforeDeleting: Story = {
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(await theSwitch(canvas));
    const ask = await within(document.body).findByRole("alertdialog", {
      name: "Stop keeping task screens?",
    });
    await expect(ask).toHaveTextContent("purlis deletes the 3 kept screens now.");
    await expect(ask).toHaveTextContent("Its report stays.");
    // Once the ask has finished coming in: it appears with the dialog family's motion.
    const remove = within(ask).getByRole("button", { name: "Delete kept screens" });
    await waitFor(() => expect(remove).toBeVisible());
  },
};

/** **Delete kept screens turns it off**: the ask goes and the switch is off. */
export const TurnedOff: Story = {
  play: async ({ canvas, userEvent }) => {
    const toggle = await theSwitch(canvas);
    await userEvent.click(toggle);
    const ask = await within(document.body).findByRole("alertdialog");
    await userEvent.click(within(ask).getByRole("button", { name: "Delete kept screens" }));
    await waitFor(() => expect(within(document.body).queryByRole("alertdialog")).toBeNull());
    await waitFor(() => expect(toggle).not.toBeChecked());
    toggle.blur();
  },
};

/** **Off**: Space turns it on at once, asking nothing. */
export const KeptOff: Story = {
  parameters: { keep: false },
  play: async ({ canvas, userEvent }) => {
    const toggle = await theSwitch(canvas);
    await expect(toggle).not.toBeChecked();
    toggle.focus();
    await untilFocused(() => toggle, "the switch");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(toggle).toBeChecked());
    await expect(within(document.body).queryByRole("alertdialog")).toBeNull();
    toggle.blur();
  },
};
