// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat's one mark** (#5340): each of its six states, with who it runs as and with its badge
 * alone, on each ground it stands on: a row in the list, a tab, a pane's head and a folded pane's
 * rail. Each held to axe and compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { CSSProperties, ReactNode } from "react";
import { expect, within } from "storybook/test";
import { Bot, SquareTerminal } from "lucide-react";
import { MARK_STATES } from "../../shownState";
import { ChatMark } from "./ChatMark";

/** Each place a mark stands, with the size and the ground it sets. */
const PLACES: readonly { name: string; size: string; ground: string }[] = [
  {
    name: "A row in the list",
    size: "calc(var(--space-6) + var(--space-2))",
    ground: "--panel-fill",
  },
  { name: "A tab", size: "var(--space-6)", ground: "--surface-base" },
  { name: "A pane's head", size: "var(--space-6)", ground: "--terminal-background" },
  { name: "A folded pane's rail", size: "var(--space-6)", ground: "--surface-raised" },
];

function EveryState({ who = <Bot aria-hidden="true" /> }: { who?: ReactNode }) {
  return (
    <div style={{ display: "grid", gap: "var(--space-5)" }}>
      {PLACES.map((place) => (
        <div
          key={place.name}
          role="group"
          aria-label={place.name}
          style={
            {
              "--chat-mark-size": place.size,
              "--chat-mark-ground": `var(${place.ground})`,
              display: "flex",
              gap: "var(--space-6)",
              padding: "var(--space-3)",
              background: `var(${place.ground})`,
            } as CSSProperties
          }
        >
          {MARK_STATES.map((state) => (
            <ChatMark key={state} state={state} who={who} word={state} />
          ))}
          <ChatMark state={null} who={who} word="running (no detail)" />
        </div>
      ))}
    </div>
  );
}

const meta = {
  title: "Families/Chat mark",
  component: EveryState,
} satisfies Meta<typeof EveryState>;
export default meta;
type Story = StoryObj<typeof meta>;

/** Each state, as it stands on who the chat runs as, in each place. */
export const Everywhere: Story = {
  play: async ({ canvasElement }) => {
    const row = within(canvasElement).getByRole("group", { name: "A tab" });
    const glyphs = [...row.querySelectorAll('[data-part="badge"] svg')].map((one) =>
      [...one.classList].find((name) => name.startsWith("lucide-")),
    );
    // No two states share an outline (WCAG 1.4.1).
    await expect(new Set(glyphs).size).toBe(MARK_STATES.length);
  },
};

/** A chat with no persona wears the terminal's mark, with the same state on it. */
export const OnATerminal: Story = { args: { who: <SquareTerminal aria-hidden="true" /> } };

/** A mark with no one to stand on: its badge alone, where it stands (the quit warning's list). */
export const Alone: Story = {
  render: () => (
    <div style={{ display: "flex", gap: "var(--space-5)" }}>
      {MARK_STATES.map((state) => (
        <ChatMark key={state} state={state} word={state} />
      ))}
    </div>
  ),
};
