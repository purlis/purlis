// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Storybook's configuration** (#2901): every `*.stories.tsx` under `src/`, drawn by the app's
 * own Vite configuration (`vite.config.ts`, which Storybook's Vite builder reads), with the
 * accessibility and Vitest addons. `docs/component-stories.md` is the guide.
 */
import type { StorybookConfig } from "@storybook/react-vite";
import { handedThemes } from "./handed.plugin.ts";

/**
 * `vite.config.ts`'s third-party notices are generated for a build of the window from every
 * crate's source; a build of Storybook ships nothing, so it leaves them out.
 */
const notTheShippedNotices = (plugin: unknown) =>
  !(
    typeof plugin === "object" &&
    plugin !== null &&
    "name" in plugin &&
    plugin.name === "purlis-third-party-notices"
  );

const config: StorybookConfig = {
  stories: ["../src/**/*.stories.tsx"],
  addons: ["@storybook/addon-a11y", "@storybook/addon-vitest"],
  framework: { name: "@storybook/react-vite", options: {} },
  core: { disableTelemetry: true },
  viteFinal: (vite) => ({
    ...vite,
    plugins: [...(vite.plugins ?? []).flat().filter(notTheShippedNotices), handedThemes()],
  }),
};

export default config;
