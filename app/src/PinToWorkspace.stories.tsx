// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Pin to workspace and Unpin from workspace** (#5953, ADR 0120 §1): every dialog that changes
 * a workspace's pins, in each way it reads, held to axe and to its keyboard path.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import type { ArtifactPinAsk } from "./bindings";
import { PinToWorkspace, UnpinArtifact } from "./PinToWorkspace";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const ASK: ArtifactPinAsk = {
  live: true,
  folder: "backlog-state",
  copied: false,
  name: "Backlog state",
  icon: "chart",
  pinned: false,
};

const meta: Meta<typeof PinToWorkspace> = {
  title: "Artifacts/Pin to workspace",
  component: PinToWorkspace,
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
  args: {
    project: "acme",
    workspace: "alpha",
    ask: ASK,
    pinning: false,
    onPin: fn(),
    onCancel: fn(),
  },
};
export default meta;
type Story = StoryObj<typeof PinToWorkspace>;

/**
 * In a LIVE workspace: it says everyone in the project sees it. It starts in the name, which
 * is the page's title; Tab reaches the icons, the arrows choose one, and Enter pins.
 */
export const Live: Story = {
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("dialog");
    await expect(dialog).toHaveTextContent("Visible to everyone in acme");
    const name = await page.findByLabelText("Name");
    await untilFocused(() => name, "It starts in the name");
    expectFocusInSight();
    await userEvent.tab();
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    expectFocusInSight();
    await userEvent.click(name);
    await userEvent.keyboard("{Enter}");
    await expect(args.onPin).toHaveBeenCalledWith("Backlog state", expect.any(String));
  },
};

/** In a LOCAL workspace: it says the artifact stays on this machine. Esc pins nothing. */
export const Local: Story = {
  args: { ask: { ...ASK, live: false } },
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("dialog");
    await expect(dialog).toHaveTextContent(
      "alpha is a LOCAL workspace, so this artifact stays on this machine.",
    );
    await untilFocused(() => page.getByLabelText("Name"), "It starts in the name");
    await userEvent.keyboard("{Escape}");
    await expect(args.onCancel).toHaveBeenCalled();
    await expect(args.onPin).not.toHaveBeenCalled();
  },
};

/** A page from outside the workspace's artifacts/: the footer names the folder it goes to. */
export const Copied: Story = {
  args: { ask: { ...ASK, copied: true, folder: "report", name: "Weekly report" } },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await expect(await page.findByRole("dialog")).toHaveTextContent(
      "The page is copied to artifacts/report/ in alpha.",
    );
  },
};

/** Pinning: the answer says so, and holds its width. */
export const Pinning: Story = {
  args: { pinning: true },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await expect(await page.findByRole("button", { name: "Pinning…" })).toBeDisabled();
  },
};

/** Refused: the core's sentence beside the answers, and the dialog stays up. */
export const Refused: Story = {
  args: { trouble: "there is already a folder 'report' in artifacts/; choose another name" },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await expect(await page.findByRole("alert")).toHaveTextContent("choose another name");
  },
};

/** Unpin from workspace: it asks first, starting on Cancel, and says who loses the icon. */
export const Unpin: StoryObj<typeof UnpinArtifact> = {
  render: (args) => <UnpinArtifact {...args} />,
  args: {
    project: "acme",
    workspace: "alpha",
    pin: {
      id: "id-release",
      folder: "release",
      name: "Release",
      icon: "rocket",
      missing: null,
      live: true,
      too_large: null,
    },
    live: true,
    unpinning: false,
    onUnpin: fn(),
    onCancel: fn(),
  },
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("alertdialog");
    await expect(dialog).toHaveTextContent("for everyone in acme");
    await expect(dialog).toHaveTextContent("Its files stay in artifacts/release/.");
    await untilFocused(() => page.getByRole("button", { name: "Cancel" }), "It starts on Cancel");
    expectFocusInSight();
    await userEvent.tab();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onUnpin).toHaveBeenCalled();
  },
};

/** Unpin in a LOCAL workspace: no one else had it, so it names no one. */
export const UnpinLocal: StoryObj<typeof UnpinArtifact> = {
  ...Unpin,
  args: { ...Unpin.args, live: false },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("alertdialog");
    await expect(dialog).not.toHaveTextContent("everyone");
  },
};
