// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What an action answered** (#3344, the sidebar-views prototype's Q3 A): a workspace made, as
 * the closable notice at the top of the content draws it, with its title, its one plain
 * sentence, Add a vision, Details holding the core's lines (never the one written for an agent) and ×.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { ActionResultNotice } from "./ActionResultNotice";
import { Notice } from "./Notice";
import { workspaceMade } from "./actionResult";

const LINES = [
  "Workspace 'git-ci-cd' ready (LIVE — charter + manifest + memory committed + shared + auto-saved) → workspaces/git-ci-cd/",
  '⬢ No vision yet — ask the developer what this workspace is for, then record it: purlis workspace vision "<the goal>"',
];

function Result({ noChip = false }: { noChip?: boolean }) {
  const [said, setSaid] = useState("Nothing pressed");
  const [closed, setClosed] = useState(false);
  const made = workspaceMade("git-ci-cd", LINES, {
    onAddVision: () => setSaid("Asked for a vision"),
  });
  return (
    <div
      className="window-work"
      style={{ display: "grid", gap: "var(--space-3)", inlineSize: 760 }}
    >
      {!closed && (
        <ActionResultNotice
          cause="result:workspace.create"
          said={made}
          onClose={() => setClosed(true)}
          onSettings={() => undefined}
          kept={noChip}
          opens={
            noChip
              ? { label: "Open git-ci-cd", onPress: () => setSaid("Went to git-ci-cd") }
              : undefined
          }
        />
      )}
      <p style={{ margin: 0 }} data-testid="said">
        {closed ? "Closed" : said}
      </p>
    </div>
  );
}

const meta = { component: Result } satisfies Meta<typeof Result>;
export default meta;
type Story = StoryObj<typeof meta>;

/** Its details open: the core's line, and nothing written for an agent. */
export const WorkspaceMade: Story = {
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByText("Workspace git-ci-cd is ready")).toBeVisible();
    // The ways stand in the title's column: the first one flush with the title, under the
    // sentence (the prototype's Q3 A).
    const title = canvas.getByText("Workspace git-ci-cd is ready").getBoundingClientRect();
    const first = canvas.getByRole("button", { name: "Add a vision" }).getBoundingClientRect();
    await expect(Math.abs(first.left - title.left)).toBeLessThanOrEqual(1);
    await expect(first.top).toBeGreaterThan(title.bottom);
    await expect(
      canvas.getByText("It has no vision yet. Add one so its chats know what it is for."),
    ).toBeVisible();
    await userEvent.click(canvas.getByRole("button", { name: "Details" }));
    await expect(canvas.getByText(/ready \(LIVE/)).toBeVisible();
    await expect(canvas.queryByText(/ask the developer/)).toBeNull();
    // Said once: the title and the sentence are the live region, Details is not in it.
    await expect(canvas.getByRole("status")).not.toHaveTextContent(/ready \(LIVE/);
    await userEvent.click(canvas.getByRole("button", { name: "Add a vision" }));
    await expect(canvas.getByTestId("said")).toHaveTextContent("Asked for a vision");
  },
};

/** × and Esc close it. */
export const Closes: Story = {
  play: async ({ canvas, userEvent }) => {
    canvas.getByRole("button", { name: "Add a vision" }).focus();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(canvas.getByTestId("said")).toHaveTextContent("Closed"));
  },
};

/**
 * **A notice about a workspace with no chip** (#3651, D-3648-16), as the Notices centre lists it:
 * nothing floats over a chip for it, so the line offers "Open git-ci-cd", its link, which goes
 * there.
 */
export const WithNoChipOffersOpen: Story = {
  args: { noChip: true },
  play: async ({ canvas, userEvent }) => {
    const open = canvas.getByRole("button", { name: "Open git-ci-cd" });
    await expect(open).toHaveAttribute("data-way", "link");
    open.focus();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByTestId("said")).toHaveTextContent("Went to git-ci-cd");
  },
};

/** A card of each tone, each with a primary way and the family's secondary button. */
function TwoWays() {
  return (
    <div
      className="window-work"
      style={{ display: "grid", gap: "var(--space-3)", inlineSize: 760 }}
    >
      {(["news", "trouble", "attention"] as const).map((tone) => (
        <Notice
          key={tone}
          cause={`story:${tone}`}
          tone={tone}
          title={`A ${tone} card`}
          fixes={[
            { label: `Update ${tone}`, onPress: () => undefined, primary: true },
            { label: `Later ${tone}`, onPress: () => undefined },
          ]}
          onDismiss={() => undefined}
        >
          Update it to bring its settings to this version of purlis.
        </Notice>
      ))}
    </div>
  );
}

/** Below this OKLab distance a person sees one colour (`purlis_core::theme::parts::SEEN`). */
const SEEN = 0.02;

/** A computed colour as OKLab, read back from a canvas so every way CSS writes one is one. */
function oklabOf(colour: string): [number, number, number] {
  const canvas = document.createElement("canvas");
  canvas.width = 1;
  canvas.height = 1;
  const drawn = canvas.getContext("2d") as CanvasRenderingContext2D;
  drawn.fillStyle = colour;
  drawn.fillRect(0, 0, 1, 1);
  const [r, g, b] = [...drawn.getImageData(0, 0, 1, 1).data]
    .slice(0, 3)
    .map((c) => c / 255)
    .map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

const apart = (x: string, y: string) => {
  const [a, b] = [oklabOf(x), oklabOf(y)];
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
};

/**
 * **A secondary way on a card is visibly a button** (#3478, #4169's review), on each tone's
 * card: at least a seen step from the card's ground, and farther from it under the pointer.
 */
export const ASecondaryWayOnTheCard: Story = {
  render: () => <TwoWays />,
  play: async ({ canvas }) => {
    for (const tone of ["news", "trouble", "attention"]) {
      const later = canvas.getByRole("button", { name: `Later ${tone}` });
      await expect(later).toHaveAttribute("data-variant", "secondary");
      const card = later.closest<HTMLElement>('[data-scope="notice"][data-part="root"]');
      await expect(card).toHaveAttribute("data-layout", "card");
      const ground = getComputedStyle(card as HTMLElement).backgroundColor;
      const rest = getComputedStyle(later).backgroundColor;
      later.setAttribute("data-hovered", "true");
      const pointed = getComputedStyle(later).backgroundColor;
      later.removeAttribute("data-hovered");
      await expect(apart(rest, ground)).toBeGreaterThanOrEqual(SEEN);
      await expect(apart(pointed, ground)).toBeGreaterThan(apart(rest, ground));
    }
  },
};
