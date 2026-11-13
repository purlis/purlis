// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Setup's Protection row** (T1-24; Q115, Q159): First run's page and the New project dialog,
 * each with the one row, Basic sandbox preselected, drawn with the core faked at its command
 * boundary, held to axe and walked by the keyboard: the arrows move the choice, and the page's
 * own act still finishes setup in one press.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { FirstRun } from "./FirstRun";
import { NewProject } from "./NewProject";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** What each story hands the page: its acts, each held to be asked about. */
type Acts = {
  onOpenRepo: ReturnType<typeof fn>;
  onMake?: ReturnType<typeof fn>;
  onCreate?: ReturnType<typeof fn>;
};

const meta = {
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "first_run_found")
        return {
          harnesses: [{ name: "claude", title: "Claude Code", installed: true, signed_in: true }],
          forges: [],
          templates: [],
        };
      if (cmd === "new_project_folder") return "/home/dev/Projects/billing-api";
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<Acts>;

export default meta;
type Story = StoryObj<Meta<Acts>>;

/** The row's radio group, found by its name. */
const rowIn = (inside: HTMLElement) =>
  within(inside).getByRole("radiogroup", { name: "Protection" });

/** From Basic sandbox, the arrow key moves the choice to Full sandbox, the focus with it. */
async function arrowToFull(row: HTMLElement) {
  const basic = within(row).getByRole("radio", { name: /^Basic sandbox \(recommended\)/ });
  basic.focus();
  await untilFocused(() => basic, "the row starts on Basic sandbox");
  expectFocusInSight();
  await userEvent.keyboard("{ArrowDown}");
  const full = within(row).getByRole("radio", { name: /^Full sandbox/ });
  await untilFocused(() => full, "the arrow moves to Full sandbox");
  await waitFor(() => expect(full).toBeChecked());
  expectFocusInSight();
}

/** First run's page: one row for both of its acts. */
export const OnFirstRun: Story = {
  args: { onOpenRepo: fn(), onMake: fn() },
  render: (args) => (
    <FirstRun
      onOpenRepo={args.onOpenRepo as never}
      onMake={args.onMake as never}
      onBack={() => undefined}
      opening={false}
    />
  ),
  play: async ({ canvasElement, args }) => {
    const row = await waitFor(() => rowIn(canvasElement));
    await arrowToFull(row);
    const canvas = within(canvasElement);
    await userEvent.type(canvas.getByLabelText("Or type the repo's path"), "/home/dev/widget");
    await userEvent.click(canvas.getByRole("button", { name: "Open repo" }));
    await expect(args.onOpenRepo).toHaveBeenCalledTimes(1);
    await expect(args.onOpenRepo).toHaveBeenCalledWith("/home/dev/widget", { kind: "fits" }, "on");
  },
};

/** The New project dialog: the same row, above its acts. */
export const InNewProject: Story = {
  args: { onOpenRepo: fn(), onCreate: fn() },
  parameters: { screenshot: "page" },
  render: (args) => (
    <NewProject
      making={false}
      opening={false}
      onCreate={args.onCreate as never}
      onOpenRepo={args.onOpenRepo as never}
      onCancel={() => undefined}
    />
  ),
  play: async ({ canvasElement, args }) => {
    const dialog = await waitFor(() =>
      within(canvasElement.ownerDocument.body).getByRole("dialog", { name: "New project" }),
    );
    await arrowToFull(rowIn(dialog));
    await userEvent.type(within(dialog).getByLabelText("Repo"), "/home/dev/widget");
    await userEvent.click(within(dialog).getByRole("button", { name: "Open repo" }));
    await expect(args.onOpenRepo).toHaveBeenCalledTimes(1);
    await expect(args.onOpenRepo).toHaveBeenCalledWith("/home/dev/widget", "on");
  },
};
