// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The right side's bar as ADR 0121 has it** (#6020): each attention view's tooltip says its
 * question, each icon's count is in its name, and the ✋ pulses once, the first time a Blocking
 * ask arrives while Asks is closed. Drawn in every cell of the theme matrix.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, waitFor, within } from "storybook/test";
import { ActivityBar, ActivityCount, ActivityDot, VIEW_MARKS, type Tabbed } from "./ActivityBar";
import type { ViewId } from "./regions";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const VIEWS: Tabbed[] = [
  { view: "asks", name: "Asks", mark: VIEW_MARKS.asks },
  { view: "updates", name: "Updates", mark: VIEW_MARKS.updates },
  { view: "notices", name: "Notices", mark: VIEW_MARKS.notices },
];

function Attention({ pulse, onPulsed }: { pulse?: ViewId; onPulsed?: () => void }) {
  const [open, setOpen] = useState<ViewId | undefined>("updates");
  return (
    <div style={{ display: "flex", blockSize: 200 }}>
      <ActivityBar
        side="right"
        variant="bar"
        name="Attention"
        views={VIEWS}
        open={open}
        current={open}
        badges={{
          asks: <ActivityCount count={2} said="2 things wait on you" tone="needs-you" />,
          updates: <ActivityCount count={1} said="1 unread update from your chats" />,
          notices: <ActivityDot said="A Notice needs fixing" />,
        }}
        pulse={pulse}
        onPulsed={onPulsed}
        panelOf={(view) => `story-panel-${view}`}
        tabOf={(view) => `story-tab-${view}`}
        onPick={(view) => setOpen((was) => (was === view ? undefined : view))}
      />
      {VIEWS.map(({ view, name }) => (
        <div
          key={view}
          id={`story-panel-${view}`}
          role="tabpanel"
          aria-labelledby={`story-tab-${view}`}
          hidden={view !== open}
        >
          {name}
        </div>
      ))}
    </div>
  );
}

const meta = { component: Attention } satisfies Meta<typeof Attention>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Each icon names its count, and its tooltip asks its question**: walked by the arrows. */
export const Questions: Story = {
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const page = within(document.body);
    const asks = canvas.getByRole("tab", { name: "Asks 2 things wait on you" });
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("tab", { name: "Updates 1 unread update from your chats" }),
      "Tab to the open view's icon",
    );
    expectFocusInSight();
    await waitFor(() => expect(page.getByRole("tooltip")).toHaveTextContent("Did my work finish?"));
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => asks, "↑ to the ✋");
    expectFocusInSight();
    await waitFor(() =>
      expect(page.getByRole("tooltip")).toHaveTextContent("Do I need to answer something?"),
    );
    await userEvent.keyboard("{End}");
    await untilFocused(
      () => canvas.getByRole("tab", { name: "Notices A Notice needs fixing" }),
      "End to Notices",
    );
    expectFocusInSight();
    await waitFor(() =>
      expect(page.getByRole("tooltip")).toHaveTextContent(
        "Is something about my setup wrong or worth knowing?",
      ),
    );
  },
};

/**
 * **The ✋'s first pulse**: one breath of its mark, and the window told as it ends. Not
 * pictured: a breath in flight has no frame that holds still, and under reduced motion it has
 * ended before the picture.
 */
export const FirstPulse: Story = {
  args: { pulse: "asks", onPulsed: fn() },
  parameters: { screenshot: false },
  play: async ({ canvas, args }) => {
    await expect(canvas.getByRole("tab", { name: /^Asks/ })).toHaveAttribute("data-pulse", "true");
    await expect(canvas.getByRole("tab", { name: /^Updates/ })).not.toHaveAttribute("data-pulse");
    await waitFor(() => expect(args.onPulsed).toHaveBeenCalledOnce(), { timeout: 5000 });
  },
};
