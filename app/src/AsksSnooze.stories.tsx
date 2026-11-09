// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Snooze all on Asks** (#5647, ADR 0119) and **for a while** (#6018, ADR 0121 decision 14):
 * the line at the head of Asks, asked once inside the view with three ways to snooze (until a
 * chat asks something new, for 1 hour, until tomorrow) and Cancel; and, while asks are snoozed
 * with a time, the line saying how many and until when, with Show them now. The window's
 * `Snoozing` is faked: a story reads what it was asked.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import type { Shown } from "./bindings";
import { SnoozeAll, type Snoozing } from "./AsksSnooze";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const permission = (session: number, ask: string, says: string): Shown => ({
  session,
  ask,
  says,
  options: [],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

const LISTED = [permission(1, "ask-1", "Run cargo test"), permission(2, "ask-2", "Run npm test")];

/** 10:00 on 1 March 2030, local time: far enough on that the line says the date. */
const LATER = Math.floor(new Date(2030, 2, 1, 10, 0, 0).getTime() / 1000);

function TheLine({
  listed,
  snoozed,
  snoozedUntil,
  snooze,
  wake,
}: {
  listed: readonly Shown[];
  snoozed: readonly Shown[];
  snoozedUntil: number | null;
  snooze: Snoozing["snooze"];
  wake: Snoozing["wake"];
}) {
  return (
    <div style={{ inlineSize: 460 }}>
      <SnoozeAll listed={listed} snoozing={{ snoozed, snoozedUntil, snooze, wake }} />
    </div>
  );
}

const meta = {
  component: TheLine,
  args: {
    listed: LISTED,
    snoozed: [],
    snoozedUntil: null,
    snooze: fn(async () => undefined),
    wake: fn(async () => undefined),
  },
} satisfies Meta<typeof TheLine>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Asked, with a choice of time**: Snooze all opens its line, the keyboard walks Snooze all,
 *  For 1 hour and Until tomorrow, and For 1 hour snoozes with a time. */
export const AskedWithAChoiceOfTime: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const open = canvas.getByRole("button", { name: "Snooze all" });
    open.focus();
    await userEvent.keyboard("{Enter}");
    const line = await waitFor(() => canvas.getByRole("group", { name: "Snooze all" }));
    const inLine = within(line);
    inLine.getByRole("button", { name: "Snooze all" }).focus();
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => inLine.getByRole("button", { name: "For 1 hour" }), "For 1 hour");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => inLine.getByRole("button", { name: "Until tomorrow" }),
      "Until tomorrow",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => inLine.getByRole("button", { name: "Cancel" }), "Cancel");
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    await untilFocused(() => inLine.getByRole("button", { name: "For 1 hour" }), "back");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.snooze).toHaveBeenCalled());
    const [, until] = (args.snooze as ReturnType<typeof fn>).mock.calls[0] as [unknown, number];
    await expect(until).toBeGreaterThan(Date.now() / 1000);
  },
};

/** **The line open**, as it waits for a choice. */
export const TheChoiceOpen: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    canvas.getByRole("button", { name: "Snooze all" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("group", { name: "Snooze all" })).toBeVisible());
  },
};

/** **Snoozed with a time**: how many, until when, and Show them now, which wakes every chat. */
export const SnoozedUntilATime: Story = {
  args: { listed: [], snoozed: LISTED, snoozedUntil: LATER },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByText(/^2 asks snoozed until .+, or until their chats ask again\.$/),
    ).toBeVisible();
    const show = canvas.getByRole("button", { name: "Show them now" });
    show.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.wake).toHaveBeenCalledWith(undefined));
  },
};
