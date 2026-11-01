// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The publish review and the export dialog** (#2781, `prototype/skill-publish`): clean, stopped
 * by a secret, and kept on this machine, then the dialog. Walked by the keyboard, held to axe, and
 * pictured in every pictured theme cell. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PublishReviewView, SkillLevel } from "./bindings";
import { ExportDialog, SkillPublishPage } from "./SkillPublish";
import { EXPORT, GITHUB, LOCAL, REVIEW, STOPPED } from "./skillPublish.testkit";
import { expectFocusInSight } from "./stories.testkit";

const LEVEL: SkillLevel = { at: "project", local: false };
const none = () => {};

function fake(review: PublishReviewView) {
  clearMocks();
  mockIPC((cmd) => {
    if (cmd === "skill_sources") return [GITHUB];
    if (cmd === "skill_publish_review") return review;
    if (cmd === "skill_export_review") return EXPORT;
    return null;
  });
}

function Page({ review }: { review: PublishReviewView }) {
  fake(review);
  return (
    <div style={{ width: 720, height: 560, display: "flex" }}>
      <SkillPublishPage
        plane="p"
        name="deploy"
        level={LEVEL}
        onCancel={none}
        onExport={none}
        onMove={none}
        onSources={none}
      />
    </div>
  );
}

const meta = {
  component: Page,
  args: { review: REVIEW },
} satisfies Meta<typeof Page>;
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

export const Export: Story = {
  render: () => {
    fake(REVIEW);
    return <ExportDialog plane="p" name="deploy" level={LEVEL} onClose={none} onExported={none} />;
  },
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    await body.findByText(/and notify, the skill it refers to/);
    await userEvent.keyboard("{Escape}");
  },
};
