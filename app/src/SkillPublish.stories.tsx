// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The publish review and the export dialog** (#2781, `prototype/skill-publish`): the review in
 * the studio's own panel, clean, stopped by a secret, and kept on this machine, then the dialog.
 * Walked by the keyboard, held to axe, and pictured in every pictured theme cell. The core is
 * faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PublishReviewView } from "./bindings";
import { STORY_HISTORY as HISTORY, StudioInPanel } from "./skillStudioStory.testkit";
import { inWorkspace, skillRowId } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";
import { EXPORT, GITHUB, LOCAL, REVIEW, STOPPED } from "./skillPublish.testkit";
import { expectFocusInSight } from "./stories.testkit";
import { ExportDialog } from "./SkillPublish";

const DEPLOY = skill("deploy", { description: "Deploys the service and checks it came up." });

/** The studio tab, as a view tab draws it, on `deploy`'s publish review. */
function Tab({ width }: { width: number; review: PublishReviewView }) {
  return <StudioInPanel width={width} height={560} />;
}

const meta = {
  component: Tab,
  args: { width: 900, review: REVIEW },
  beforeEach: ({ args }) => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio")
        return studio([DEPLOY], { workspace: "smart-ide", workspaces: ["smart-ide"] });
      if (cmd === "skill_sources") return [GITHUB];
      if (cmd === "skill_publish_review") return args.review;
      if (cmd === "skill_export_review") return EXPORT;
      return null;
    });
    openViewAt(HISTORY, {
      page: "library",
      scope: inWorkspace("smart-ide"),
      skill: skillRowId(DEPLOY),
      publishing: { at: "project", local: false },
    });
    return () => {
      forgetViewHistory(HISTORY);
      clearMocks();
    };
  },
} satisfies Meta<typeof Tab>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Clean: Story = {
  play: async ({ canvas }) => {
    await canvas.findByText("deploy/scripts/release.sh");
    const open = canvas.getByRole("button", { name: "Open draft pull request" });
    await waitFor(() => expect(open).not.toHaveAttribute("aria-disabled", "true"));
    await userEvent.tab();
    await expectFocusInSight();
  },
};

export const SomethingStopsIt: Story = {
  args: { review: STOPPED },
  play: async ({ canvas }) => {
    await canvas.findByTestId("publish-stops");
    await expect(canvas.getByRole("button", { name: "Open draft pull request" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  },
};

export const KeptOnThisMachine: Story = {
  args: { review: LOCAL },
  play: async ({ canvas }) => {
    await canvas.findByRole("button", { name: "Move…" });
  },
};

export const Narrow: Story = {
  args: { width: 420 },
  play: async ({ canvas }) => {
    const page = await canvas.findByTestId("publish-review");
    await canvas.findByText("deploy/scripts/release.sh");
    await expect(page.scrollWidth).toBeLessThanOrEqual(page.clientWidth + 1);
  },
};

export const Export: Story = {
  render: () => (
    <ExportDialog
      plane="p"
      name="deploy"
      level={{ at: "project", local: false }}
      onClose={() => {}}
      onExported={() => {}}
    />
  ),
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    await body.findByText(/and notify, the skill it refers to/);
  },
};
