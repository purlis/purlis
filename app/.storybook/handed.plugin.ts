// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The built-in themes, handed to a story's page as `virtual:purlis-handed`** (#2901).
 *
 * The real window finds what its core resolved on a global before its own code runs, and
 * `theme.ts` refuses to load without it. A unit test's window is handed it by
 * `src/theme/handed.testkit.ts`, which reads the core's theme folder from disk; a story runs in a
 * real browser, which cannot, so this plugin reads it on the server side through that same
 * testkit and serves it as a module. One reader, so a story and a unit test draw the same
 * themes. Read here rather than imported as JSON, which would mean opening the dev server's file
 * access to the whole repository (as `ownLicence` in `vite.config.ts` says).
 */
import type { Plugin } from "vite";

const ID = "virtual:purlis-handed";

export function handedThemes(): Plugin {
  return {
    name: "purlis-handed-themes",
    resolveId: (asked) => (asked === ID ? `\0${ID}` : undefined),
    async load(loading) {
      if (loading !== `\0${ID}`) return undefined;
      const { handedAtCreation } = await import("../src/theme/handed.testkit.ts");
      return `export default ${JSON.stringify(handedAtCreation())};\n`;
    },
  };
}
