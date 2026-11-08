// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every flat fill a theme draws, side by side** (#3178): each surface step, each wash and each
 * ground a row or a tab is chosen or pointed at on, a square of it on the window's ground, so a
 * change to any of them fails this story's picture in every cell, whichever other stories draw it
 * and however little of it they show. A wash is see-through, so it is pictured as the window
 * draws it, laid on `surface.base`. What a picture lets through is `.storybook/pictures.ts`'s.
 *
 * Nothing here is a control, and nothing has words: a picture is all it is for.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import { property, TOKENS, type Token } from "./theme";

/** A token drawn as a flat fill: a surface step, a wash, a layer of the axis, or a ground a row,
 *  a tab or a control is chosen, aimed at or pointed at on. */
const FLAT =
  /^surface\.|\.surface$|wash|^layer\.(?!divider$)|^(list\.selected|tab\.active|control\.(hover|aimed)|overlay\.scrim)$/;

/** Every flat fill, in the order a theme file lists them. */
const FLAT_FILLS: readonly Token[] = TOKENS.filter((token) => FLAT.test(token));

function Fills() {
  return (
    <div
      data-flat-fills=""
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(8, var(--space-8))",
        gap: "var(--space-2)",
      }}
    >
      {FLAT_FILLS.map((token) => (
        <div
          key={token}
          data-fill={token}
          style={{ blockSize: "var(--space-8)", background: `var(${property(token)})` }}
        />
      ))}
    </div>
  );
}

const meta = { component: Fills } satisfies Meta<typeof Fills>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Each flat fill once**: the surface steps, the washes, the layers and the chosen grounds. */
export const EveryFlatFill: Story = {
  play: async ({ canvasElement }) => {
    const drawn = [...canvasElement.querySelectorAll<HTMLElement>("[data-fill]")];
    await expect(drawn.map((one) => one.dataset.fill)).toEqual([...FLAT_FILLS]);
    for (const token of ["wash.warning", "accent.wash", "surface.overlay", "danger.wash"])
      await expect(FLAT_FILLS).toContain(token);
  },
};
