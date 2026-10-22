// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The built-in themes, handed to a story's page** (#2901, #2945).
 *
 * The real window finds what its core resolved on a global before its own code runs, and
 * `theme.ts` refuses to load without it. A story runs in a real browser, which the core cannot
 * hand anything to, so this plugin serves, as modules:
 *
 * - `virtual:purlis-handed`: every appearance of every built-in theme as the core hands it to a
 *   window, from `built-in-themes.json`, which a test of the app's crate holds to what the core
 *   resolves (`app/src-tauri/src/storythemes.rs`). Its default export is what the window is
 *   handed at creation (Purlis Classic's appearances, dark first); `themes` is every theme by its
 *   id and each appearance by its id, which the matrix draws (`matrix.ts`).
 * - `virtual:purlis-faces`: the bytes of each face those themes name, by the digest the window
 *   asks for them by, read from the core's built-in theme folders, as purlis serves the window a
 *   face (`theme_font`).
 *
 * Read here rather than imported as files, which would mean opening the dev server's file access
 * to the whole repository (as `ownLicence` in `vite.config.ts` says).
 */
/// <reference types="node" />
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

const HANDED = "virtual:purlis-handed";
const FACES = "virtual:purlis-faces";

/** The core's built-in theme folders. */
const BUILT_IN = fileURLToPath(
  new URL("../../crates/purlis-core/src/theme/built-in", import.meta.url),
);

type Themes = Record<
  string,
  Record<string, { fonts?: Record<string, { faces: { asset: string }[] }> }>
>;

/** The file the core's resolution of the built-in themes is kept in. */
const THEMES = fileURLToPath(new URL("built-in-themes.json", import.meta.url));

function themes(): Themes {
  return JSON.parse(readFileSync(THEMES, "utf8")) as Themes;
}

/**
 * Every face `named` asks for, as base64 by its digest, from the built-in folders, and each file
 * read for them. Throws when a digest is in no folder: a theme drawn in its fallback would be
 * pictured as though it were drawn in its own font.
 */
function faces(named: Themes): { found: Record<string, string>; read: string[] } {
  const wanted = new Set(
    Object.values(named).flatMap((appearances) =>
      Object.values(appearances).flatMap((theme) =>
        Object.values(theme.fonts ?? {}).flatMap((font) => font.faces.map((face) => face.asset)),
      ),
    ),
  );
  const found: Record<string, string> = {};
  const read: string[] = [];
  for (const folder of readdirSync(BUILT_IN)) {
    const dir = join(BUILT_IN, folder, "fonts");
    if (!existsSync(dir)) continue;
    for (const file of readdirSync(dir).filter((name) => name.endsWith(".woff2"))) {
      read.push(join(dir, file));
      const bytes = readFileSync(join(dir, file));
      const digest = createHash("sha256").update(bytes).digest("hex");
      if (wanted.has(digest)) found[digest] = bytes.toString("base64");
    }
  }
  const missing = [...wanted].filter((digest) => !(digest in found));
  if (missing.length > 0)
    throw new Error(
      `no built-in theme folder has the face of digest ${missing.join(", ")} that ` +
        "built-in-themes.json names: a font file was moved or changed since it was written; " +
        "write it again (docs/component-stories.md)",
    );
  return { found, read };
}

export function handedThemes(): Plugin {
  return {
    name: "purlis-handed-themes",
    resolveId: (asked) => (asked === HANDED || asked === FACES ? `\0${asked}` : undefined),
    load(loading) {
      // Watched, so a running Storybook draws a theme written again, or a face changed.
      if (loading === `\0${HANDED}`) {
        this.addWatchFile(THEMES);
        const all = themes();
        const classic = Object.values(all["purlis-classic"]);
        return (
          `export default ${JSON.stringify({ built_in: classic })};\n` +
          `export const themes = ${JSON.stringify(all)};\n`
        );
      }
      if (loading === `\0${FACES}`) {
        this.addWatchFile(THEMES);
        const { found, read } = faces(themes());
        for (const file of read) this.addWatchFile(file);
        return `export default ${JSON.stringify(found)};\n`;
      }
      return undefined;
    },
  };
}
