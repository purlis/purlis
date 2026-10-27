// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Browsers grid** (#2027): every browser in scope as a tile under the Inbox's scope
 * switch, here two projects' three browsers, one of them driven by the person, and a grid with
 * none in its scope. A tile's picture is the browser's mark until its first frame, which a story
 * never sends, so the picture is the same in every run.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { BrowsersGridTab } from "./BrowsersGridTab";
import { forgetProjectsBrowsers, tellProjectBrowsers, type GridTile } from "./browsersGrid";
import { forgetProjectViews } from "./projectViews";
import { expectFocusInSight } from "./stories.testkit";

const HERE = "/home/dev/plane";
const THERE = "/home/dev/shop";

const tile = (plane: string, program: string, extra: Partial<GridTile> = {}): GridTile => ({
  plane,
  program,
  own: false,
  name: "b1",
  chat: "steward 1",
  workspace: "alpha",
  driver: null,
  pool: null,
  ...extra,
});

function tell(tiles: boolean) {
  tellProjectBrowsers({
    plane: HERE,
    project: "plane",
    tiles: tiles
      ? [
          tile(HERE, "A1"),
          tile(HERE, "A2", { name: "b2", driver: "person" }),
          tile(HERE, "A3", { own: true, name: null, chat: null, workspace: "beta" }),
        ]
      : [],
    open: () => undefined,
  });
  tellProjectBrowsers({
    plane: THERE,
    project: "shop",
    tiles: [tile(THERE, "B1", { chat: "web-dev 2" })],
    open: () => undefined,
  });
}

function Grid() {
  return (
    <div style={{ inlineSize: 720 }}>
      <BrowsersGridTab plane={HERE} workspace="alpha" />
    </div>
  );
}

const meta = {
  component: Grid,
  // **A tile is named by its label and described by its one line** (D-2911-22), as the list
  // family's cards are: that meets WCAG 2.5.3, since the name is the visible label, and axe's
  // experimental `label-content-name-mismatch` compares the name with the whole of the tile's
  // visible text (its line as well), so it is off on these stories alone.
  parameters: {
    a11y: { options: { rules: { "label-content-name-mismatch": { enabled: false } } } },
  },
  beforeEach: () => {
    // A view no frame ever reaches: each tile keeps its mark.
    mockIPC((cmd) => (cmd === "watch_browser" ? new Promise(() => undefined) : null));
    return () => {
      clearMocks();
      forgetProjectsBrowsers();
      forgetProjectViews();
    };
  },
} satisfies Meta<typeof Grid>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **This project's three browsers**: Tab reaches the scope switch and then the grid, whose
 * arrows walk its tiles; the switch's arrows pick All projects, which adds the other project's
 * browser and names each tile's project.
 */
export const ThisProject: Story = {
  beforeEach: () => tell(true),
  play: async ({ canvas, userEvent }) => {
    const grid = canvas.getByRole("listbox", { name: "Browsers" });
    await expect(canvas.getAllByRole("option")).toHaveLength(3);

    await userEvent.tab();
    await expect(canvas.getByRole("radio", { name: /This project/ })).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    const first = canvas.getByRole("option", { name: /Browser b1 of steward 1/ });
    await waitFor(() => expect(first).toHaveFocus());
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await expect(canvas.getByRole("option", { name: /Browser b2 of steward 1/ })).toHaveFocus();
    expectFocusInSight();

    await userEvent.tab({ shift: true });
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(canvas.getAllByRole("option")).toHaveLength(4));
    await expect(grid).toHaveTextContent("shop · alpha");

    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/**
 * **None in this project**: the empty state says how many run in every project's, and its one
 * button widens to them.
 */
export const NoneHere: Story = {
  beforeEach: () => tell(false),
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByText("No browser runs in this project")).toBeVisible();
    await expect(canvas.getByText("1 browser runs in all projects.")).toBeVisible();
    await userEvent.tab();
    await expect(canvas.getByRole("radio", { name: /This project/ })).toHaveFocus();
    await userEvent.tab();
    const widen = canvas.getByRole("button", { name: "Show all projects" });
    await expect(widen).toHaveFocus();
    expectFocusInSight();
    (document.activeElement as HTMLElement | null)?.blur();
  },
};
