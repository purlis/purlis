// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **One settings search across every level** (#2920, N27; #2814·4): the Settings tab itself, at
 * the Project level, with the core faked at its command boundary. The box searches every
 * setting of every level the tab offers; the nav's first page is every match, under the level
 * and page each is on; a match here is drawn with its control and one at another level is a way
 * there, which lands on its field; Set here narrows to what this level's files hold. Each story
 * is drawn, held to axe, walked by the keyboard and pictured in every pictured theme cell
 * (`docs/component-stories.md`).
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PlaneId, SettingsFile } from "../bindings";
import { expectFocusInSight, pictureNow, untilFocused } from "../stories.testkit";
import { SettingsTab } from "./SettingsTab";
import type { Level } from "./groups";
import { forgetGroups } from "./links";

const PLANE = "/home/dev/plane" as PlaneId;

/** The team's file sets how far a save goes; the local file holds nothing. */
const SHARED: SettingsFile = {
  which: "shared",
  file: "purlis.toml",
  exists: true,
  text: '[plane]\nmode = "push"\n',
  refusals: [],
  parsed: true,
  fields: [{ path: [{ key: "plane" }, { key: "mode" }], value: { kind: "text", value: "push" } }],
};
const LOCAL: SettingsFile = {
  which: "local",
  file: "purlis.local.toml",
  exists: true,
  text: "",
  refusals: [],
  parsed: true,
  fields: [],
};

/** A Settings tab whose level switcher changes its level, as the window's tabs do. */
function Tab() {
  const [level, setLevel] = useState<Level>("project");
  return (
    // The view's body, as a Settings tab is drawn in it: the side column takes its height.
    <div className="view-body" style={{ blockSize: 680, inlineSize: 1040 }}>
      <SettingsTab plane={PLANE} level={level} onLevelChange={setLevel} />
    </div>
  );
}

const meta = {
  component: Tab,
  beforeEach: () => {
    forgetGroups();
    mockIPC((cmd) => {
      if (cmd === "project_settings") return { shared: SHARED, local: LOCAL };
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

const box = (canvas: ReturnType<typeof within>) =>
  canvas.getByRole("searchbox", { name: "Search settings" });
const allResults = (canvas: ReturnType<typeof within>) =>
  within(canvas.getByRole("navigation", { name: "Groups" })).getByRole("button", {
    name: /^All results/,
  });

/** The level is read: its first page is on screen. */
async function read(canvas: ReturnType<typeof within>) {
  await waitFor(() => expect(canvas.getByRole("region", { name: "General" })).toBeVisible());
}

/**
 * **Every match, at every level**: typed words fill the nav's first page with what they find at
 * You and at the Project level, each under its level and page. The keyboard goes from the box to
 * Set here and on to that page in the nav, then into this level's pages under it.
 */
export const EveryLevel: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await read(canvas);

    await userEvent.click(box(canvas));
    await userEvent.keyboard("theme");
    await waitFor(() =>
      expect(canvas.getByRole("heading", { level: 3, name: "You › Theme" })).toBeVisible(),
    );
    await expect(
      canvas.getByRole("heading", { level: 3, name: "Project › Appearance" }),
    ).toBeVisible();
    expectFocusInSight();

    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Set here" }), "Tab to Set here");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => allResults(canvas), "Tab to every match, the nav's first page");
    await expect(allResults(canvas)).toHaveAttribute("aria-current", "page");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => allResults(canvas), "back on every match");
  },
};

/**
 * **A match at another level lands on its field there**: the way to it is a button, Return
 * takes the tab to You at Text, with the keyboard on the slider found.
 */
export const GoToAnotherLevel: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await read(canvas);

    await userEvent.click(box(canvas));
    await userEvent.keyboard("window text size");
    const go = await waitFor(() =>
      canvas.getByRole("button", { name: "Go to Text: Window text size" }),
    );
    go.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");

    await untilFocused(
      () => canvas.getByRole("slider", { name: /window text size/i }),
      "the field found, at You",
    );
    expectFocusInSight();
    await expect(box(canvas)).toHaveValue("");
  },
};

/**
 * **Set here**: pressed, the page holds only what this level's files set, each with the set-here
 * mark, and says how many; pressed again, the level is as it was.
 */
export const SetHere: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await read(canvas);

    const toggle = canvas.getByRole("button", { name: "Set here" });
    toggle.focus();
    await userEvent.keyboard(" ");
    await expect(toggle).toHaveAttribute("aria-pressed", "true");
    await waitFor(() =>
      expect(canvas.getByRole("region", { name: "1 setting found" })).toBeVisible(),
    );
    await expect(canvas.getByRole("heading", { level: 3, name: "Project › Saving" })).toBeVisible();
    await expect(canvas.getByRole("img", { name: "Set at this level" })).toBeVisible();
    expectFocusInSight();
    // Pictured narrowed, before the toggle puts the level back.
    await pictureNow();

    await userEvent.keyboard(" ");
    await waitFor(() => expect(canvas.getByRole("region", { name: "General" })).toBeVisible());
  },
};

/** **Nothing found**: the page says so, in the words searched for; Escape clears the box. */
export const NothingFound: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await read(canvas);

    await userEvent.click(box(canvas));
    await userEvent.keyboard("colour of the moon");
    await waitFor(() =>
      expect(canvas.getByRole("region", { name: "No setting found" })).toBeVisible(),
    );
    await expect(
      canvas.getByText("No setting matches “colour of the moon”.", { ignore: "[role=status]" }),
    ).toBeVisible();
    expectFocusInSight();
    // Pictured with what it says, before Escape clears it.
    await pictureNow();

    await userEvent.keyboard("{Escape}");
    await expect(box(canvas)).toHaveValue("");
  },
};
