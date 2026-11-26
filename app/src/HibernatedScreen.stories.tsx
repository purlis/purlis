// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Pictures: drawn by the update-baselines bot (#7711 merged these stories without them).
/**
 * **A hibernated chat's pane** (SC-4b, #7631): the band with Resume over what the chat showed as
 * it was hibernated, drawn read-only, and the pane where no copy was kept. In Chromium and
 * WebKit, in every cell of the theme matrix, with axe. Its keyboard path reaches Resume, then the
 * screen in one Tab stop.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, waitFor, within } from "storybook/test";
import { HibernatedBand } from "./HibernatedNotice";
import { HIBERNATED_SCREEN_LABEL, HibernatedScreenShown, NONE_KEPT } from "./HibernatedScreen";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** What chat 4 showed as it was hibernated. */
const KEPT = [
  "> run the migration on staging",
  "",
  "\u001b[1m● Bash(./migrate --env staging)\u001b[0m",
  "  ⎿ 14 migrations applied, token ‹secret: deploy-key›",
  "",
  "Done. Staging is on schema 42.",
].join("\r\n");

function Pane({ screen, onResume }: { screen: string | null; onResume: () => void }) {
  return (
    <div style={{ inlineSize: 960 }}>
      <HibernatedBand session={4} onResume={onResume} />
      <HibernatedScreenShown screen={screen} />
    </div>
  );
}

const meta = {
  title: "Chats/Hibernated chat pane",
  component: Pane,
  args: { screen: KEPT, onResume: fn() },
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Pane>;
export default meta;
type Story = StoryObj<typeof meta>;

/** **A screen kept**: Resume first, then the screen, one Tab stop, read-only. */
export const WithASnapshot: Story = {
  play: async ({ canvasElement, args, userEvent }) => {
    const canvas = within(canvasElement);
    const region = await canvas.findByRole("region", { name: HIBERNATED_SCREEN_LABEL });
    await waitFor(() =>
      expect(within(region).getAllByText(/Staging is on schema 42/).length).toBeGreaterThan(0),
    );
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Resume" }), "Tab to Resume");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => region, "Tab to the screen");
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    await userEvent.keyboard("{Enter}");
    await expect(args.onResume).toHaveBeenCalled();
  },
};

/** **None kept**: the pane says so, and Resume still works. */
export const WithoutASnapshot: Story = {
  args: { screen: null },
  play: async ({ canvasElement, args, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText(NONE_KEPT)).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Resume" }), "Tab to Resume");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onResume).toHaveBeenCalled();
  },
};
