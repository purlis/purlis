// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pinned artifact's tab and its icons** (#5952, ADR 0120 §2–3): every state a person meets
 * them in, held to axe and to their keyboard paths. The core is faked at its command boundary;
 * the page itself is a native webview no story can draw, so the frame is the element it is laid
 * over, named by the artifact.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { ArtifactAbout, ArtifactOpened, ArtifactPinShown } from "./bindings";
import { ArtifactGroup } from "./ArtifactGroup";
import { ArtifactTab } from "./ArtifactTab";
import { expectFocusInSight } from "./stories.testkit";
import { ActivityBarFrame } from "./ui/activity/ActivityItem";

const ABOUT: ArtifactAbout = { name: "Backlog state", pinned_by: "Ada Lovelace", missing: null };

function answers(about: ArtifactAbout, opened: Partial<ArtifactOpened> = {}) {
  mockIPC((cmd) => {
    if (cmd === "artifact_about") return about;
    if (cmd === "artifact_open")
      return { label: "artifact-1", standing: "script-free", changed_by: null, ...opened };
    if (cmd.startsWith("artifact_")) return null;
    return new Promise(() => {});
  });
}

const meta: Meta<typeof ArtifactTab> = {
  title: "Views/Pinned artifact",
  component: ArtifactTab,
  args: {
    plane: "p",
    workspace: "alpha",
    folder: "backlog-state",
    title: "Backlog state",
    waits: false,
    onAsk: () => {},
  },
  decorators: [
    (Story) => (
      <div style={{ display: "flex", blockSize: "24rem", inlineSize: "40rem" }}>
        <Story />
      </div>
    ),
  ],
};
export default meta;
type Story = StoryObj<typeof ArtifactTab>;

/** Open: purlis's bar naming who pinned it, and the frame the page is laid over. */
export const Open: Story = {
  beforeEach: () => answers(ABOUT),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const frame = await page.findByTestId("artifact-frame");
    await expect(frame).toHaveAccessibleName("Backlog state");
    await expect(page.getByTestId("artifact-bar")).toHaveTextContent("Pinned by Ada Lovelace");
    await userEvent.tab();
    await expect(frame).toHaveFocus();
    expectFocusInSight();
  },
};

/** A page with script, never approved here: shown without it, and the bar asks (#5959). */
export const AsksForScript: Story = {
  beforeEach: () => answers(ABOUT, { standing: "new" }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const approve = await page.findByRole("button", { name: "Approve script" });
    await expect(page.getByTestId("artifact-edge")).toHaveAttribute("data-standing", "asks");
    await userEvent.tab();
    await expect(approve).toHaveFocus();
    expectFocusInSight();
  },
};

/** Its pin changed since it was approved: the bar names who changed it, and asks again. */
export const PinChanged: Story = {
  beforeEach: () => answers(ABOUT, { standing: "pin-changed", changed_by: "Grace Hopper" }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await expect(
      await page.findByText(
        "Grace Hopper changed its pin since you approved it, so it is shown without its script",
      ),
    ).toBeInTheDocument();
    await expect(page.getByRole("button", { name: "Approve script" })).toBeInTheDocument();
  },
};

/** Approved here: its script runs, and the edge says so. Nothing to press. */
export const ScriptApproved: Story = {
  beforeEach: () => answers(ABOUT, { standing: "approved" }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await expect(await page.findByText("Script approved on this machine")).toBeInTheDocument();
    await expect(page.getByTestId("artifact-edge")).toHaveAttribute("data-standing", "runs");
    await expect(page.queryByRole("button", { name: "Approve script" })).toBeNull();
  },
};

/** Pinned here and not saved: the bar says so. */
export const NotSaved: Story = {
  beforeEach: () => answers({ ...ABOUT, pinned_by: null }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await expect(
      await page.findByText("Pinned here, not saved to the project yet"),
    ).toBeInTheDocument();
  },
};

/** Put back by a launch: nothing runs until Open is pressed. */
export const Waits: Story = {
  args: { waits: true },
  beforeEach: () => answers(ABOUT),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const open = await page.findByRole("button", { name: "Open Backlog state" });
    await expect(page.queryByTestId("artifact-frame")).toBeNull();
    await userEvent.tab();
    await expect(open).toHaveFocus();
    expectFocusInSight();
  },
};

/** Its folder is gone: why, and no frame. */
export const Missing: Story = {
  beforeEach: () =>
    answers({ ...ABOUT, missing: "there is no folder 'backlog-state' in artifacts/" }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await expect(await page.findByTestId("artifact-missing")).toHaveTextContent(
      "there is no folder 'backlog-state' in artifacts/",
    );
  },
};

const pin = (folder: string, name: string, icon: string): ArtifactPinShown => ({
  id: `id-${folder}`,
  folder,
  name,
  icon,
  missing: null,
  live: true,
});

/** The left bar's artifact group: after a divider, below purlis's own icons. */
export const OnTheBar: StoryObj<typeof ArtifactGroup> = {
  render: () => (
    <ActivityBarFrame
      variant="bar"
      className="activity-column"
      data-side="left"
      top={
        <ArtifactGroup
          plane="p"
          workspace="alpha"
          current="release"
          pins={[
            pin("backlog-state", "Backlog state", "chart"),
            pin("release", "Release", "rocket"),
            pin("oncall", "On call", "zap"),
          ]}
          onOpen={() => {}}
        />
      }
      bottom={null}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const group = page.getByRole("toolbar", { name: "Artifacts" });
    const [first, second] = within(group).getAllByRole("button");
    await userEvent.tab();
    // The tab stop is the artifact in front.
    await expect(second).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{ArrowUp}");
    await expect(first).toHaveFocus();
    expectFocusInSight();
  },
};

const ELEVEN = Array.from({ length: 11 }, (_, n) => pin(`page-${n}`, `Page ${n + 1}`, "chart"));

/**
 * More than eight after a merge (#5953): eight drawn, and the rest in the group's show-more,
 * none dropped. The arrows reach show-more; Enter opens its menu on its first row.
 */
export const ShowMore: StoryObj<typeof ArtifactGroup> = {
  render: () => (
    <ActivityBarFrame
      variant="bar"
      className="activity-column"
      data-side="left"
      top={<ArtifactGroup plane="p" workspace="alpha" pins={ELEVEN} onOpen={() => {}} />}
      bottom={null}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const group = page.getByRole("toolbar", { name: "Artifacts" });
    await expect(within(group).getAllByRole("button")).toHaveLength(9);
    await userEvent.tab();
    await userEvent.keyboard("{End}");
    const more = page.getByRole("button", { name: "3 more artifacts" });
    await expect(more).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const menu = await within(canvasElement.ownerDocument.body).findByRole("menu");
    await expect(within(menu).getAllByRole("menuitem")).toHaveLength(3);
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await expect(more).toHaveFocus();
  },
};

/** An icon's menu (#5953), from the keyboard: Open, Move up, Move down, Unpin from workspace…. */
export const IconMenu: StoryObj<typeof ArtifactGroup> = {
  beforeEach: () => answers(ABOUT),
  render: () => (
    <ActivityBarFrame
      variant="bar"
      className="activity-column"
      data-side="left"
      top={
        <ArtifactGroup plane="p" workspace="alpha" pins={ELEVEN.slice(0, 3)} onOpen={() => {}} />
      }
      bottom={null}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await userEvent.tab();
    await expect(page.getByRole("button", { name: "Page 1" })).toHaveFocus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await within(canvasElement.ownerDocument.body).findByRole("menu");
    await expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((row) => row.textContent),
    ).toEqual(["Open", "Move up", "Move down", "Unpin from workspace…"]);
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await expect(page.getByRole("button", { name: "Page 1" })).toHaveFocus();
  },
};
