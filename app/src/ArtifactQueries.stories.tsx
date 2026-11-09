// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a pinned artifact's page reads of purlis, in its bar** (#5969, ADR 0120 §7): the
 * queries its page declared, the parameters that widen them, a deprecated one's warning, and
 * the plane-wide grant of its own. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { ArtifactOpened, ArtifactQuery } from "./bindings";
import { ArtifactTab } from "./ArtifactTab";
import { expectFocusInSight } from "./stories.testkit";

const TODOS: ArtifactQuery = {
  id: "purlis.todo_list@1",
  scope: "workspace",
  widens: ["done: the done todos, not only the open ones"],
  warning: null,
};

const PLANE: ArtifactQuery = {
  id: "purlis.everything@1",
  scope: "plane",
  widens: [],
  warning: null,
};

function answers(opened: Partial<ArtifactOpened>) {
  mockIPC((cmd) => {
    if (cmd === "artifact_about")
      return { name: "Backlog state", pinned_by: "Ada Lovelace", missing: null, too_large: null };
    if (cmd === "artifact_open")
      return {
        label: "artifact-1",
        standing: "approved",
        changed_by: null,
        generated: true,
        queries: [],
        plane_granted: false,
        ...opened,
      };
    if (cmd.startsWith("artifact_")) return null;
    return new Promise(() => {});
  });
}

const meta: Meta<typeof ArtifactTab> = {
  title: "Views/Pinned artifact/Queries",
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

/** Asking: the bar lists what the page reads beside Approve script. */
export const Declared: Story = {
  beforeEach: () => answers({ standing: "new", queries: [TODOS] }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await expect(await page.findByTestId("artifact-queries")).toHaveTextContent(
      "Reads purlis.todo_list@1 (done: the done todos, not only the open ones)",
    );
  },
};

/** A deprecated query still answers, and the bar warns of it. */
export const Deprecated: Story = {
  beforeEach: () =>
    answers({
      queries: [
        { ...TODOS, warning: "Calls purlis.todo_list@1, which is deprecated: use purlis.todos" },
      ],
    }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await expect(await page.findByTestId("artifact-query-warning")).toBeInTheDocument();
  },
};

/** Approved, with a plane-scoped query: project-wide reads are asked apart. */
export const AsksForPlane: Story = {
  beforeEach: () => answers({ queries: [TODOS, PLANE] }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const allow = await page.findByRole("button", { name: "Allow project-wide reads" });
    await userEvent.tab();
    await expect(allow).toHaveFocus();
    expectFocusInSight();
  },
};

/** Project-wide reads allowed: said, with nothing to press. */
export const PlaneAllowed: Story = {
  beforeEach: () => answers({ queries: [PLANE], plane_granted: true }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await expect(
      await page.findByText("Project-wide reads allowed on this machine"),
    ).toBeInTheDocument();
    await expect(page.queryByRole("button", { name: "Allow project-wide reads" })).toBeNull();
  },
};
