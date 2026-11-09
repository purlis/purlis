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
import type { ArtifactAbout, ArtifactFresh, ArtifactPinShown } from "./bindings";
import { ArtifactGroup } from "./ArtifactGroup";
import { ArtifactTab } from "./ArtifactTab";
import { expectFocusInSight } from "./stories.testkit";
import { ActivityBarFrame } from "./ui/activity/ActivityItem";

const ABOUT: ArtifactAbout = {
  name: "Backlog state",
  pinned_by: "Ada Lovelace",
  missing: null,
  fresh: null,
};

const HOUR = 60 * 60 * 1000;

/** Updated `hours` ago, and stale a day after. */
const updated = (hours: number): ArtifactFresh => {
  const at = Date.now() - hours * HOUR;
  return { at, stale_at: at + 24 * HOUR, source: "declared" };
};

function answers(about: ArtifactAbout) {
  mockIPC((cmd) => {
    if (cmd === "artifact_about") return about;
    if (cmd === "artifact_open") return "artifact-1";
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

/** Its data was updated three hours ago: the bar says so (#5955). */
export const Fresh: Story = {
  beforeEach: () => answers({ ...ABOUT, fresh: updated(3) }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await expect(await page.findByText("Updated 3 hours ago")).toBeInTheDocument();
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
  fresh: null,
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

/** Freshness on the bar (#5955): Backlog state's data is two days old, so its icon is dimmed;
 *  Release's is an hour old. Each says when in its tooltip. */
export const StaleOnTheBar: StoryObj<typeof ArtifactGroup> = {
  render: () => (
    <ActivityBarFrame
      variant="bar"
      className="activity-column"
      data-side="left"
      top={
        <ArtifactGroup
          plane="p"
          workspace="alpha"
          pins={[
            { ...pin("backlog-state", "Backlog state", "chart"), fresh: updated(48) },
            { ...pin("release", "Release", "rocket"), fresh: updated(1) },
          ]}
          onOpen={() => {}}
        />
      }
      bottom={null}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const stale = page.getByRole("button", { name: "Backlog state" });
    await expect(stale).toHaveAttribute("data-dimmed");
    await expect(page.getByRole("button", { name: "Release" })).not.toHaveAttribute("data-dimmed");
    await userEvent.tab();
    await expect(stale).toHaveFocus();
    expectFocusInSight();
    // A tooltip is drawn in a layer of the document's own, outside the story's canvas.
    const tooltip = await within(canvasElement.ownerDocument.body).findByRole("tooltip");
    await expect(tooltip).toHaveTextContent("Updated 2 days ago");
  },
};
