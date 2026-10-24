// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A harness's icon** (#2922): the three glyphs purlis draws beside their names, the monograms
 * a profile with no icon shows, a person's own SVG, and one standing alone with its name. Each
 * held to axe and compared with its baseline in every theme (`docs/component-stories.md`); the
 * glyphs are the theme's where its icon set draws them, the window's own under Purlis Classic.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import type { HarnessIconRow, SvgIcon } from "../../bindings";
import { HarnessIcon, ownIconsOf } from "./HarnessIcon";

/** One harness as a list names it: its icon, then its name. */
type Named = { name: string; kind: string; icon: HarnessIconRow | null };

const BUILT_INS: Named[] = [
  { name: "Claude Code", kind: "claude", icon: { glyph: "spark", own: null, monogram: "C" } },
  { name: "opencode", kind: "opencode", icon: { glyph: "braces", own: null, monogram: "OC" } },
  { name: "Codex", kind: "codex", icon: { glyph: "terminal", own: null, monogram: "CX" } },
];

const MONOGRAMS: Named[] = [
  { name: "claude-work", kind: "claude", icon: { glyph: null, own: null, monogram: "C" } },
  { name: "opencode-pair", kind: "opencode", icon: { glyph: null, own: null, monogram: "OC" } },
  { name: "codex-ci", kind: "codex", icon: { glyph: null, own: null, monogram: "CX" } },
];

/** A person's own mark, as the core hands it: a ring and a softer dot. */
const MINE: SvgIcon = {
  view_box: "0 0 24 24",
  shapes: [
    {
      tag: "circle",
      attributes: {
        cx: "12",
        cy: "12",
        r: "9",
        fill: "none",
        stroke: "currentColor",
        "stroke-width": "2",
      },
    },
    {
      tag: "circle",
      attributes: { cx: "12", cy: "12", r: "4", fill: "currentColor", opacity: "0.6" },
    },
  ],
};

const OWN: Named = {
  name: "mine",
  kind: "claude",
  icon: { glyph: null, own: "mine.svg", monogram: "C" },
};

/** The answer's own icons, by the file name a row names. */
const SVGS: Record<string, SvgIcon> = { "mine.svg": MINE };

/** A list of harnesses, each its icon beside its name, in the quiet text a list draws them in. */
function Listed({ rows }: { rows: Named[] }) {
  const owns = ownIconsOf(rows, SVGS);
  return (
    <ul
      style={{ listStyle: "none", margin: 0, padding: 0, display: "grid", gap: "var(--space-3)" }}
    >
      {rows.map((row, at) => (
        <li
          key={row.name}
          className="ui-text-body"
          style={{ display: "flex", alignItems: "center", gap: "var(--space-3)" }}
        >
          <HarnessIcon icon={row.icon} svg={owns[at]} kind={row.kind} />
          <span>{row.name}</span>
        </li>
      ))}
    </ul>
  );
}

const meta = {
  component: Listed,
} satisfies Meta<typeof Listed>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Beside a name, each icon is hidden from assistive technology: the name says it. */
const decorative: Story["play"] = async ({ canvas, canvasElement }) => {
  await expect(canvas.queryByRole("img")).toBeNull();
  for (const icon of canvasElement.querySelectorAll(".harness-icon")) {
    await expect(icon).toHaveAttribute("aria-hidden", "true");
  }
};

/** Claude Code, opencode and Codex, each drawn with the glyph purlis draws for it. */
export const BuiltIns: Story = {
  args: { rows: BUILT_INS },
  play: async (context) => {
    await decorative(context);
    for (const icon of context.canvasElement.querySelectorAll(".harness-icon")) {
      await expect(icon).toHaveAttribute("data-drawn", "glyph");
    }
  },
};

/** Profiles that name no icon: their harness kind's monogram, never a blank. */
export const Monograms: Story = {
  args: { rows: MONOGRAMS },
  play: async (context) => {
    await decorative(context);
    for (const letters of ["C", "OC", "CX"]) {
      await expect(context.canvas.getByText(letters)).toBeVisible();
    }
  },
};

/** A profile naming an SVG of the person's own, drawn from the shapes the core kept. */
export const OwnSvg: Story = {
  args: { rows: [OWN, ...BUILT_INS.slice(0, 1)] },
  play: async (context) => {
    await decorative(context);
    const own = context.canvasElement.querySelector(".harness-icon");
    await expect(own).toHaveAttribute("data-drawn", "svg");
  },
};

/** Standing alone, with no name beside it, the icon is an image named for its harness. */
export const Alone: Story = {
  args: { rows: [] },
  render: () => (
    <span className="ui-text-body" style={{ color: "var(--text-muted)" }}>
      <HarnessIcon icon={BUILT_INS[0].icon} kind="claude" label="Claude Code" />
    </span>
  ),
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("img", { name: "Claude Code" })).toBeVisible();
  },
};
