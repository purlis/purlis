// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The meter family** (#3674): the Grades tab's latest grade, three bars that compare (with it,
 * without it, its previous version), and a running grade's progress, chats done of N, which
 * moves while the story plays, and one going with no end known. Each held to axe and compared
 * with its baseline in Chromium and WebKit, in every theme's cell (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { Button } from "../button/Button";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Meter, MeterGroup, ProgressBar } from "./Meter";

/** A page's width of the Grades tab. */
function OnAPage({ children }: { children: React.ReactNode }) {
  return <div style={{ inlineSize: "32rem", maxInlineSize: "100%" }}>{children}</div>;
}

/** **The latest grade**: with it, without it and its previous version, on one scale. */
function LatestGrade() {
  return (
    <OnAPage>
      <MeterGroup aria-label="Latest grade, 28 Oct">
        <Meter label="With it" value={86} valueLabel="86%" variant="subject" />
        <Meter label="Without it" value={68} valueLabel="68%" variant="baseline" />
        <Meter label="Previous version" value={80} valueLabel="80%" variant="previous" />
      </MeterGroup>
    </OnAPage>
  );
}

const meta = {
  component: LatestGrade,
} satisfies Meta<typeof LatestGrade>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **Three bars that compare**: one scale and one label column, so the bars start together and a
 * longer bar is a larger score. A reader hears each meter by its label with its value. The
 * picture is the three, each in its own colour.
 */
export const Compared: Story = {
  play: async ({ canvas }) => {
    const group = canvas.getByRole("group", { name: "Latest grade, 28 Oct" });
    const meters = canvas.getAllByRole("meter");
    await expect(meters.map((one) => one.getAttribute("aria-valuetext"))).toEqual([
      "86%",
      "68%",
      "80%",
    ]);
    await expect(group).toBeVisible();
    // The tracks start and end together, and the fills are as long as their scores.
    const tracks = meters.map(
      (one) => one.querySelector('[data-part="track"]')?.getBoundingClientRect() as DOMRect,
    );
    for (const track of tracks) {
      await expect(Math.abs(track.left - tracks[0].left)).toBeLessThan(1);
      await expect(Math.abs(track.width - tracks[0].width)).toBeLessThan(1);
    }
    const share = (at: number) =>
      (meters[at].querySelector('[data-part="fill"]') as HTMLElement).getBoundingClientRect()
        .width / tracks[at].width;
    await expect(share(0)).toBeCloseTo(0.86, 1);
    await expect(share(1)).toBeCloseTo(0.68, 1);
    await expect(share(2)).toBeCloseTo(0.8, 1);
  },
};

/** **A running grade**: its chats done of 12, a press a chat further on. */
function Running() {
  const [done, setDone] = useState(3);
  return (
    <OnAPage>
      <ProgressBar label="Chats" value={done} maxValue={12} valueLabel={`${done} of 12`} />
      <p style={{ marginBlockStart: "var(--space-5)" }}>
        <Button onPress={() => setDone((now) => Math.min(12, now + 1))}>Run a chat</Button>
      </p>
    </OnAPage>
  );
}

/**
 * **A progress bar**: chats done of N. A chat further on moves its fill and its value, and says
 * the new value politely once. The picture is 4 of 12.
 */
export const Progress: Story = {
  render: () => <Running />,
  play: async ({ canvas, userEvent }) => {
    const bar = canvas.getByRole("progressbar", { name: "Chats" });
    await expect(bar).toHaveAttribute("aria-valuetext", "3 of 12");
    const run = canvas.getByRole("button", { name: "Run a chat" });
    await userEvent.tab();
    await untilFocused(() => run, "Progress: Tab lands on Run a chat");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(bar).toHaveAttribute("aria-valuetext", "4 of 12");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("4 of 12"));
  },
};

/**
 * **Going, with no end known**: the whole track in the accent's stripes, which move, and stand
 * still under reduced motion. No value is said. No picture: the stripes move while it is taken.
 */
export const Indeterminate: Story = {
  render: () => (
    <OnAPage>
      <ProgressBar label="Starting the chats" isIndeterminate />
    </OnAPage>
  ),
  parameters: { screenshot: false },
  play: async ({ canvas }) => {
    const bar = canvas.getByRole("progressbar", { name: "Starting the chats" });
    await expect(bar).not.toHaveAttribute("aria-valuenow");
  },
};
