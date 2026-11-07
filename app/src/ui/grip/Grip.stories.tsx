// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The grip** (#5290): a mark that turns into dots to be picked up by, on each ground it stands
 * on, at rest and with the keyboard on it. Its dots are drawn by the keyboard's focus, which its
 * `play` puts there before the picture is taken, never by a pointer resting on it for a while
 * (`docs/component-stories.md`). Each held to axe and compared with its baseline.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { CSSProperties } from "react";
import { Bot, Settings } from "lucide-react";
import { expect, fn, userEvent, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { ChatMark } from "../chat-mark/ChatMark";
import { Grip } from "./Grip";

/** Each place a grip stands, with the size and the ground it sets: a row's, then a head's. */
const PLACES: readonly { name: string; size: string; ground: string; head: boolean }[] = [
  {
    name: "A row in the list",
    size: "calc(var(--space-6) + var(--space-2))",
    ground: "--panel-fill",
    head: false,
  },
  {
    name: "The chosen row",
    size: "calc(var(--space-6) + var(--space-2))",
    ground: "--list-selected",
    head: false,
  },
  { name: "A pane's head", size: "var(--space-6)", ground: "--terminal-background", head: true },
];

function EveryPlace({ onPress }: { onPress: () => void }) {
  return (
    <div style={{ display: "grid", gap: "var(--space-5)" }}>
      {PLACES.map((place) => {
        const sized = {
          "--chat-mark-size": place.size,
          "--chat-mark-ground": `var(${place.ground})`,
          ...(place.head ? { "--grip-mark": place.size, "--grip-reach": "var(--space-7)" } : {}),
          display: "flex",
          alignItems: "center",
          gap: "var(--space-6)",
          padding: "var(--space-3)",
          background: `var(${place.ground})`,
        } as CSSProperties;
        const grip = (mark: React.ReactNode, what: string) =>
          place.head ? (
            <Grip label={`Move ${what}`} onPress={onPress}>
              {mark}
            </Grip>
          ) : (
            <Grip onPress={onPress}>{mark}</Grip>
          );
        return (
          <div key={place.name} role="group" aria-label={place.name} style={sized}>
            {place.head ? (
              <>
                <ChatMark state="working" who={grip(<Bot aria-hidden="true" />, "steward 1")} />
                <ChatMark state="needs" who={grip(<Bot aria-hidden="true" />, "steward 2")} />
                {/* A view's, in a split: its mark alone, with no state. */}
                {grip(<Settings aria-hidden="true" />, "Settings")}
              </>
            ) : (
              // A row is the control the grip stands in: the keyboard on it shows the dots.
              <button type="button" className="ui-row" aria-label={`steward on ${place.name}`}>
                <ChatMark state="idle" who={grip(<Bot aria-hidden="true" />, "")} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

const meta = {
  component: EveryPlace,
  args: { onPress: fn() },
} satisfies Meta<typeof EveryPlace>;
export default meta;
type Story = StoryObj<typeof meta>;

/** At rest: each mark as it is, and the state's badge on a chat's. */
export const AtRest: Story = {
  play: async ({ canvasElement }) => {
    const grips = canvasElement.querySelectorAll('[data-scope="grip"][data-part="root"]');
    for (const grip of grips) {
      const dots = grip.querySelector('[data-part="dots"]') as Element;
      await expect(getComputedStyle(dots).visibility).toBe("hidden");
    }
  },
};

/** The keyboard on a head's grip: its dots in the mark's place, the badge still beside them, and
 *  Enter picks up what it moves. */
export const WithTheKeyboard: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const grip = canvas.getByRole("button", { name: "Move steward 1" });
    // The rows first, which are the controls their grips stand in.
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "steward on A row in the list" }),
      "Tab lands on the first row",
    );
    const inRow = canvasElement.querySelector('[data-part="dots"]') as Element;
    await expect(getComputedStyle(inRow).visibility).toBe("visible");
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => grip, "Tab lands on the head's first grip");
    const dots = grip.querySelector('[data-part="dots"]') as Element;
    await expect(getComputedStyle(dots).visibility).toBe("visible");
    await expect(dots.querySelectorAll('[data-part="dot"]')).toHaveLength(6);
    const badge = grip.closest('[data-scope="chat-mark"][data-part="root"]');
    await expect(badge?.querySelector('[data-part="badge"]')).not.toBeNull();
    await userEvent.keyboard("{Enter}");
    await expect(args.onPress).toHaveBeenCalledTimes(1);
    expectFocusInSight();
  },
};
