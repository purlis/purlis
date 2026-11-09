// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A diff, drawn in every built-in** (#4424): the merge view's line washes and word washes under
 * real syntax colours, so a picture shows whether a changed word still reads and still stands out
 * from its line, and axe holds every word of it at 4.5:1 in every cell (#3796): a changed file
 * and a new one.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor } from "storybook/test";
import { untilFocused } from "../stories.testkit";
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

/** Its chunk loads on a busy runner: longer than `waitFor`'s second. */
const GRAMMAR_ARRIVES = { timeout: 10_000 };

const meta = {
  component: MergeViewer,
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
      if (!code || !word) throw new Error("the merge view has not drawn its changed comment");
      expect(getComputedStyle(word).color).not.toBe(getComputedStyle(code).color);
    }, GRAMMAR_ARRIVES);
    expect(canvasElement.querySelector(".cm-changedText")).not.toBeNull();
    // Each side is reached from the keyboard.
    const side = (name: string) => () =>
      canvasElement.querySelector<HTMLElement>(`[aria-label="${name}"]`);
    await userEvent.tab();
    await untilFocused(side("wash.ts, before"), "the side before the change");
    await userEvent.tab();
    await untilFocused(side("wash.ts, after"), "the side after the change");
  },
};

/** **A new file** (#3796): every line inserted, on the after side alone, held to axe like a change. */
export const NewFile: Story = {
  args: {
    path: "notes.md",
    base: "",
    head: `# Release checklist

1. Run the tests.
2. Write the changelog entry, then \`npm run fold\`.

> Ask before a release on a Friday.
`,
    hunks: [{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 6 }],
  },
  play: async ({ canvasElement }) => {
    // The grammar arrives after the first draw: wait until the heading takes its own colour.
    await waitFor(() => {
      const code = canvasElement.querySelector<HTMLElement>(".cm-merge-b .cm-content");
      const spans = canvasElement.querySelectorAll<HTMLElement>(".cm-merge-b .cm-line span");
      const heading = [...spans].find((span) => span.textContent?.includes("Release checklist"));
      if (!code || !heading) throw new Error("the merge view has not drawn its heading");
      expect(getComputedStyle(heading).color).not.toBe(getComputedStyle(code).color);
    }, GRAMMAR_ARRIVES);
    const side = (name: string) => () =>
      canvasElement.querySelector<HTMLElement>(`[aria-label="${name}"]`);
    await userEvent.tab();
    await untilFocused(side("notes.md, before"), "the empty side before the file");
    await userEvent.tab();
    await untilFocused(side("notes.md, after"), "the new file");
  },
};
