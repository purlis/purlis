// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Approve and start, in place** (#3942): the question a Skill studio start refused for its
 * profile's approval asks, with the picker's own sentence, line and mark for the profile. Held to
 * axe, walked by the keyboard and pictured as it opens, before the play answers it
 * (`pictureNow`). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { NeedsApproval } from "./bindings";
import { SkillChatApproveAsk } from "./SkillChatApproval";
import type { SkillChatAsk } from "./skillDraftAsks";
import { pictureNow } from "./stories.testkit";

const PLANE = "/home/dev/plane";

const WORK: NeedsApproval = {
  profile: "work",
  kind: "claude",
  source: "charter.local.toml",
  approval: "new",
  shown: "CLAUDE_CONFIG_DIR=~/.claude-work claude --model opus",
};

const TRY: SkillChatAsk = { plane: PLANE, kind: "try", id: "d1", profile: "work" };

/** The question over a page, and what its answer did once it is answered. */
function Asking() {
  const [said, setSaid] = useState<string>();
  return (
    <>
      <p data-testid="said">{said ?? "Asking"}</p>
      {said === undefined && (
        <SkillChatApproveAsk
          plane={PLANE}
          asked={{ wanted: TRY, approval: WORK }}
          onCancel={() => setSaid("Cancelled")}
          onApproved={(again) =>
            setSaid(`Started ${again.kind} on ${again.kind === "try" ? again.profile : ""}`)
          }
        />
      )}
    </>
  );
}

const meta = {
  component: Asking,
  parameters: { screenshot: "page" },
  beforeEach: () => {
    mockIPC((cmd) => (cmd === "approve_profile" ? null : undefined));
    return () => clearMocks();
  },
} satisfies Meta<typeof Asking>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **The question**: what starts, the command it would run, and Approve and start. */
export const ApproveAndStart: Story = {
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    const question = await page.findByRole("alertdialog", { name: "Approve work?" });
    await expect(question).toHaveTextContent(
      "purlis has not run this profile before. It would run: CLAUDE_CONFIG_DIR=~/.claude-work claude --model opus",
    );
    await pictureNow();
    await userEvent.click(within(question).getByRole("button", { name: "Approve and start" }));
    await waitFor(() => expect(page.getByTestId("said")).toHaveTextContent("Started try on work"));
  },
};
