// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's pins** (SK-15d, #3232): the Pin switch a Skill studio row filtered to a workspace
 * draws, and the list a skill's tab draws of every workspace and persona that pins it, each with
 * Unpin. What is pressed is said under them, as the window would change it.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import type { SkillPinner } from "./bindings";
import { SkillPinnedBy, SkillPinSwitch } from "./SkillPins";
import { expectFocusInSight } from "./stories.testkit";

const OPS: SkillPinner = { at: "workspace", name: "ops" };
const REVIEWER: SkillPinner = { at: "persona", name: "reviewer" };

/** A studio row's switch for `deploy` in ops, and the tab's list of who pins it. */
function Pins() {
  const [pinners, setPinners] = useState<SkillPinner[]>([REVIEWER]);
  const pinned = pinners.some((by) => by.at === OPS.at && by.name === OPS.name);
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: 520 }}>
      <SkillPinSwitch
        name="deploy"
        by={OPS}
        pinned={pinned}
        busy={false}
        onPin={(on) =>
          setPinners((now) =>
            on ? [OPS, ...now] : now.filter((by) => by.at !== OPS.at || by.name !== OPS.name),
          )
        }
      />
      <SkillPinnedBy
        name="deploy"
        pinners={pinners}
        busy={false}
        onUnpin={(gone) =>
          setPinners((now) => now.filter((by) => by.at !== gone.at || by.name !== gone.name))
        }
      />
    </div>
  );
}

const meta = {
  component: Pins,
} satisfies Meta<typeof Pins>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **Pinned for a workspace and a persona**: Tab reaches the switch, Space pins deploy for ops
 * and the tab's list names it; Tab reaches each Unpin, and Enter on the persona's takes its pin
 * away. The picture is the switch on, with ops listed.
 */
export const PinnedByTwo: Story = {
  play: async ({ canvas, userEvent }) => {
    const pin = canvas.getByRole("switch", { name: "Pin deploy for ops" });
    await expect(pin).not.toBeChecked();

    await userEvent.tab();
    await expect(pin).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(pin).toBeChecked());
    const list = canvas.getByTestId("skill-pinned-by");
    await expect(list).toHaveTextContent("Pinned for workspace ops");
    await expect(list).toHaveTextContent("Pinned for persona reviewer");

    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Unpin deploy for ops" })).toHaveFocus();
    await userEvent.tab();
    const unpin = canvas.getByRole("button", { name: "Unpin deploy for reviewer" });
    await expect(unpin).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(list).not.toHaveTextContent("persona reviewer"));

    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/** **Pinned by none**: the tab says no workspace or persona pins it, and the switch is off. */
export const PinnedByNone: Story = {
  render: () => (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: 520 }}>
      <SkillPinSwitch name="deploy" by={OPS} pinned={false} busy={false} onPin={() => undefined} />
      <SkillPinnedBy name="deploy" pinners={[]} busy={false} onUnpin={() => undefined} />
    </div>
  ),
  play: async ({ canvas }) => {
    await expect(canvas.getByTestId("skill-pinned-by")).toHaveTextContent(
      "No workspace or persona pins deploy.",
    );
  },
};
