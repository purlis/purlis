// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What every story is drawn in and held to** (#2901, #2945, `docs/component-stories.md`).
 *
 * - **Drawn in the window's own stylesheet and a built-in theme**, as `theme.ts` draws the
 *   window: the theme, appearance and density of the matrix cell the test runs in (`matrix.ts`),
 *   or, in Storybook's own UI, the toolbar's Theme, Appearance and Density. The one decorator
 *   below does it for every story; no story picks a theme of its own.
 * - **In the theme's own fonts**, made from the bytes purlis ships them in, as the window makes
 *   them from the bytes purlis serves it (`handed.plugin.ts`).
 * - **Inside one frame**, `[data-story-frame]`, on the theme's ground: what a picture is taken of
 *   (`screenshots.setup.ts`), so a picture is the story's size, not the page's.
 * - **Held to WCAG 2.2 AA by axe** in every cell: every rule tagged A or AA in WCAG 2.0, 2.1 and
 *   2.2, with `target-size` (2.5.8) on, and any violation fails the story's test
 *   (`test: "error"`).
 */
import "./handed";
import "../src/styles.css";
import type { Decorator, Preview } from "@storybook/react-vite";
import { themes } from "virtual:purlis-handed";
import {
  CLASSIC,
  DEFAULT_THEME,
  drawIn,
  drawPerson,
  fetchFacesWith,
  personInForce,
  type Theme,
} from "../src/theme/theme";
import { DENSITIES, cellInTest, type CellDensity } from "./matrix";

/** The axe tags of WCAG 2.2 level AA: every A and AA rule of 2.0, 2.1 and 2.2. */
const WCAG_22_AA = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

// A face's bytes, by digest, as purlis would serve them; asked for only once a theme names one.
fetchFacesWith(async (asset) => {
  const { default: faces } = await import("virtual:purlis-faces");
  const text = faces[asset];
  if (text === undefined) return undefined;
  const bytes = Uint8Array.from(atob(text), (char) => char.charCodeAt(0));
  return bytes.buffer;
});

/** The theme `id`'s appearance `appearance`, or its first one when it has none of that id. */
function appearanceOf(id: string, appearance: string): Theme {
  const theme = themes[id] ?? themes[CLASSIC];
  return (theme?.[appearance] ?? Object.values(theme ?? {})[0] ?? DEFAULT_THEME) as Theme;
}

const drawnIn: Decorator = (Story, { globals }) => {
  const cell = cellInTest() ?? {
    theme: String(globals.theme),
    appearance: String(globals.appearance),
    density: globals.density as CellDensity,
  };
  const density = DENSITIES.includes(cell.density) ? cell.density : DENSITIES[0];
  if (personInForce().density !== density) drawPerson({ ...personInForce(), density });
  drawIn(appearanceOf(cell.theme, cell.appearance));
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

/** Every appearance id a built-in theme has, in their order. */
const APPEARANCES = [...new Set(Object.values(themes).flatMap((one) => Object.keys(one)))];

const preview: Preview = {
  decorators: [drawnIn],
  globalTypes: {
    theme: {
      description: "The built-in theme the story is drawn in",
      toolbar: { title: "Theme", items: Object.keys(themes), dynamicTitle: true },
    },
    appearance: {
      description: "The theme's appearance; a theme without it draws its first",
      toolbar: { title: "Appearance", items: APPEARANCES, dynamicTitle: true },
    },
    density: {
      description: "The person's density",
      toolbar: { title: "Density", items: [...DENSITIES], dynamicTitle: true },
    },
  },
  initialGlobals: { theme: CLASSIC, appearance: "dark", density: DENSITIES[0] },
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
