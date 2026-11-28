// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Project › Chats › Hibernate after** (SC-4h, #7637): the box at the default, a value set from
 * the keyboard, and one the core refuses, said under the box. Drawn, held to axe, walked by the
 * keyboard and pictured in every pictured theme cell (`docs/component-stories.md`). The core is
 * faked at its command boundary, refusing as it does.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import type { SettingsGroup } from "./groups";
import { hibernateAfterSetting } from "./hibernateAfter";
import { OnItsPage } from "./settingsStory.testkit";

const PLANE = "/Users/you/projects/charter";

const UNDER = "5m is under 10 minutes, the least it may be. Set 10m or more, or off.";

/** The core, faked: `[chats] hibernate_after` at `after`, refusing what is under 10 minutes. */
function theCore(after: string) {
  const state = { after };
  mockIPC((cmd, args) => {
    if (cmd === "hibernate_after") return state.after;
    if (cmd === "set_hibernate_after") {
      const asked = (args as { after: string }).after;
      if (asked === "5m") throw UNDER;
      state.after = asked;
      return state.after;
    }
    return null;
  });
}

const GROUP: SettingsGroup = {
  id: "project.chats",
  label: "Chats",
  help: "When purlis hibernates this project's chats that nobody is looking at.",
  settings: [hibernateAfterSetting(PLANE)],
};

function Page() {
  return <OnItsPage group={GROUP} />;
}

const meta = {
  component: Page,
  beforeEach: () => {
    theCore("30m");
    return () => clearMocks();
  },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The box, once the core has said what is in force. */
async function theBox(canvas: ReturnType<typeof within>) {
  const box = await canvas.findByRole("textbox", { name: "Hibernate after" });
  await waitFor(() => expect(box).not.toBeDisabled());
  return box;
}

/**
 * **At the default**: Tab reaches the box, which reads 30m; a time typed and Enter sets it. The
 * picture is the row at rest.
 */
export const AtTheDefault: Story = {
  play: async ({ canvas, userEvent }) => {
    const box = await theBox(canvas);
    await expect(box).toHaveValue("30m");
    await userEvent.tab();
    await untilFocused(() => box, "the box");
    expectFocusInSight();
    await userEvent.clear(box);
    await userEvent.keyboard("2h{Enter}");
    await waitFor(() => expect(box).toHaveValue("2h"));
    await expect(canvas.queryByText(UNDER)).toBeNull();
    await userEvent.clear(box);
    await userEvent.keyboard("30m{Enter}");
    await waitFor(() => expect(box).toHaveValue("30m"));
    box.blur();
  },
};

/**
 * **Under 10 minutes**: the core refuses it, the row says why under the box, and the box keeps
 * the words. The picture is the refusal.
 */
export const UnderTenMinutes: Story = {
  play: async ({ canvas, userEvent }) => {
    const box = await theBox(canvas);
    await userEvent.click(box);
    await userEvent.clear(box);
    await userEvent.keyboard("5m{Enter}");
    await expect(await canvas.findByText(UNDER)).toBeVisible();
    await expect(box).toHaveValue("5m");
    box.blur();
  },
};
