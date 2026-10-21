// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What every story is drawn in and held to** (#2901, `docs/component-stories.md`).
 *
 * - **Drawn in the window's own stylesheet and a built-in theme**, picked by the `theme` global
 *   (the toolbar's Theme, or `globals` on a story), as `theme.ts` draws the window.
 * - **Inside one frame**, `[data-story-frame]`, on the theme's ground: what a picture is taken of
 *   (`screenshots.setup.ts`), so a picture is the story's size, not the page's.
 * - **Held to WCAG 2.2 AA by axe**: every rule tagged A or AA in WCAG 2.0, 2.1 and 2.2, with
 *   `target-size` (2.5.8) on, and any violation fails the story's test (`test: "error"`).
 */
import "./handed";
import "../src/styles.css";
import type { Decorator, Preview } from "@storybook/react-vite";
import { BUILT_IN, DEFAULT_THEME, drawIn } from "../src/theme/theme";

/** The axe tags of WCAG 2.2 level AA: every A and AA rule of 2.0, 2.1 and 2.2. */
const WCAG_22_AA = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

const drawnIn: Decorator = (Story, { globals }) => {
  drawIn(BUILT_IN[globals.theme as string] ?? DEFAULT_THEME);
  return (
    <div
      data-story-frame=""
      style={{
        display: "inline-block",
        padding: "var(--space-4)",
        background: "var(--surface-base)",
        color: "var(--text-primary)",
      }}
    >
      <Story />
    </div>
  );
};

const preview: Preview = {
  decorators: [drawnIn],
  globalTypes: {
    theme: {
      description: "The built-in theme the story is drawn in",
      toolbar: { title: "Theme", items: Object.keys(BUILT_IN), dynamicTitle: true },
    },
  },
  initialGlobals: { theme: DEFAULT_THEME.name },
  parameters: {
    a11y: {
      test: "error",
      options: {
        runOnly: { type: "tag", values: WCAG_22_AA },
        // Off in axe by default; on here, as the spec rules (#2897).
        rules: { "target-size": { enabled: true } },
      },
    },
  },
};

export default preview;
