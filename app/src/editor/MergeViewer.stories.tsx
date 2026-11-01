// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A diff, drawn in every built-in** (#4424): the merge view's line washes and word washes under
 * real syntax colours, so a picture shows whether a changed word still reads and still stands out
 * from its line.
 *
 * The editor is kept out of axe as the Skill studio's story keeps it (#3796). The theme check
 * holds its colours on these washes (`theme/check.rs`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor } from "storybook/test";
import { MergeViewer } from "./LightEditor";

const BASE = `// The wash marks a word.
export interface Line { alpha: number }
const first = { tone: "soft", depth: 1 };
function drawn(line: Line): number {
  return line.alpha;
}
`;

const HEAD = `// The wash marks a changed word.
export type Line = { weight: number }
let first = { shade: "firm", depth: 2 };
function painted(line: Line): string {
  return line.weight.toString();
}
`;

const HUNKS = [{ oldStart: 1, oldLines: 6, newStart: 1, newLines: 6 }];

const meta = {
  component: MergeViewer,
  parameters: {
    a11y: { context: { exclude: [".cm-editor"] } },
  },
  args: { path: "wash.ts", base: BASE, head: HEAD, hunks: HUNKS },
  decorators: [
    (Story) => (
      <div style={{ width: 860, height: 220 }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof MergeViewer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const WordsChanged: Story = {
  play: async ({ canvasElement }) => {
    // The grammar arrives after the first draw: wait until the comment takes its own colour.
    await waitFor(() => {
      const code = canvasElement.querySelector<HTMLElement>(".cm-merge-b .cm-content");
      const spans = canvasElement.querySelectorAll<HTMLElement>(".cm-merge-b .cm-line span");
      const word = [...spans].filter((span) => span.textContent?.includes("changed")).at(-1);
      expect(code && word).toBeTruthy();
      expect(getComputedStyle(word!).color).not.toBe(getComputedStyle(code!).color);
    });
    expect(canvasElement.querySelector(".cm-changedText")).not.toBeNull();
    // Each side is reached from the keyboard.
    await userEvent.tab();
    await waitFor(() =>
      expect(document.activeElement).toHaveAttribute("aria-label", "wash.ts, before"),
    );
    await userEvent.tab();
    await waitFor(() =>
      expect(document.activeElement).toHaveAttribute("aria-label", "wash.ts, after"),
    );
  },
};
