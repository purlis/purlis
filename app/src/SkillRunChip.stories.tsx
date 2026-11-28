// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The skill run chip of a run of another workspace's copy** (#6083): its persona names
 * `workspaces/ops:deploy`, so the chat in `smart-ide` runs `ops`'s copy. The chip is drawn as
 * every run's is, and says the copy in its name and its tooltip, "deploy of workspace ops", as
 * the core worked it out at the run's start. Pressed, it opens that copy's runs. Walked by the
 * keyboard, held to axe, and pictured at rest.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, waitFor } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { OpenRun } from "./bindings";
import { SkillRunChip } from "./SkillRunChip";
import { SkillRunsHere, fixedRuns } from "./skillRuns";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const SESSION = 21;

/** A run of `ops`'s `deploy`, two tool calls in, as the core tells it. */
const RUN: OpenRun = {
  skill: "deploy",
  name: "deploy",
  id: "01RUN",
  tool_calls: 2,
  tokens: null,
  level: "workspace",
  copy_workspace: "ops",
};

type Args = { onOpen: (run: OpenRun, session: number) => void };

/** The chip on a pane's head, its run the one {@link RUN} is. */
function OnAHead({ onOpen }: Args) {
  return (
    <SkillRunsHere.Provider value={fixedRuns({ [SESSION]: RUN })}>
      <div className="pane-head">
        <div className="pane-chips">
          <span className="pane-using">
            <SkillRunChip session={SESSION} onOpen={onOpen} />
          </span>
        </div>
      </div>
    </SkillRunsHere.Provider>
  );
}

const meta = {
  component: OnAHead,
  args: { onOpen: fn() },
  beforeEach: () => {
    mockIPC(() => null);
  },
} satisfies Meta<typeof OnAHead>;
export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **A run of the copy its persona names by another workspace**: Tab reaches the chip, which is
 * named for that copy, its tooltip says so, and Enter opens that copy's runs.
 */
export const ANamedWorkspacesCopy: Story = {
  play: async ({ args, canvas, userEvent }) => {
    // A browser spaces the name's unseen pieces as it likes (PaneHeadNarrow's stories match it
    // the same way); the whole sentence is held in SkillRunChip.test.tsx.
    const chip = await canvas.findByRole("button", {
      name: /^Using the skill deploy of workspace ops\b/,
    });
    await userEvent.tab();
    await untilFocused(() => chip, "Tab onto the skill run");
    expectFocusInSight();
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"]')).toHaveTextContent(
        "Using the skill deploy of workspace ops: 2 tool calls so far. Open its runs.",
      ),
    );
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onOpen).toHaveBeenCalledWith(RUN, SESSION));
    // The picture is of the chip at rest: neither its ring nor its tooltip.
    chip.blur();
    await waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull());
  },
};
