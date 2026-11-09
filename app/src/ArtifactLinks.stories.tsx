// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { ArtifactLinkDialog } from "./ArtifactLinks";
import { untilFocused } from "./stories.testkit";

const meta: Meta<typeof ArtifactLinkDialog> = {
  title: "ArtifactLinks",
  component: ArtifactLinkDialog,
  // The dialog draws over the page, outside the story's frame.
  parameters: { screenshot: "page" },
  args: { onClose: () => {} },
};
export default meta;

/** An https link written in the page: its host and its whole address, asked before it opens. */
export const Asks: StoryObj<typeof ArtifactLinkDialog> = {
  args: {
    link: {
      label: "artifact-1",
      url: "https://github.com/purlis/purlis/issues?q=is%3Aopen+label%3Aspec",
      host: "github.com",
      refused: null,
    },
  },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const cancel = await page.findByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "It starts on Cancel");
    await userEvent.tab();
    await untilFocused(
      () => page.getByRole("button", { name: "Always open github.com" }),
      "Tab reaches Always open",
    );
    await userEvent.tab();
    await untilFocused(() => page.getByRole("button", { name: "Open" }), "Tab reaches Open");
    await userEvent.keyboard("{Shift>}{Tab}{Tab}{/Shift}");
    await untilFocused(() => cancel, "Shift+Tab goes back to Cancel");
    expect(page.getByRole("dialog").textContent).toContain("label%3Aspec");
  },
};

/** A link purlis will not open: the sentence that says why, and Close. */
export const Refused: StoryObj<typeof ArtifactLinkDialog> = {
  args: {
    link: {
      label: "artifact-1",
      url: null,
      host: null,
      refused:
        "purlis opens only https links from an artifact, so it will not open this file: link",
    },
  },
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const close = await page.findByRole("button", { name: "Close" });
    await untilFocused(() => close, "It starts on Close");
    expect(page.queryByRole("button", { name: "Open" })).toBeNull();
  },
};
