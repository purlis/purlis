// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A story's system monospace is a face the stories ship, never the machine's** (#5425).
 *
 * Purlis Classic names no mono face of its own: its mono and terminal stacks name the system's
 * (`ui-monospace, Menlo, Consolas, monospace`). The pinned image has none of those, so which
 * monospace a story drew depended on what the browser had drawn before: ChatText, Badge,
 * MergeViewer, SkillPublish and Explorer changed size and glyphs by shard. Making `Menlo` from
 * a shipped face was not enough (#5438's first run still drew some cells in the machine's), so a
 * story's page puts a face of its own first instead: every mono or terminal stack that names no
 * face is drawn as `"purlis-stories-mono", <the theme's stack>`, and that face is made from the
 * mono faces Purlis ships. Nothing in the stack after it is reached. The window itself still
 * draws the machine's.
 */

/** The family the stories make from Purlis's mono faces and put first in a stack with none. */
export const STORIES_MONO = "purlis-stories-mono";

/** One face a theme's font names: its bytes' digest, weight and style. */
export type Face = { asset: string; weight: number; style: string };

type Font = { stack: string; faces: Face[] };
type Drawn = { fonts?: Record<string, Font> };
type Themes = Record<string, Record<string, Drawn>>;

/** The slots drawn in a monospace. */
const MONO_SLOTS = ["mono", "terminal"] as const;

/** The faces {@link STORIES_MONO} is made from: the mono faces of Purlis's dark. */
export function faces(themes: Themes): Face[] {
  const found = themes.purlis?.dark?.fonts?.mono?.faces ?? [];
  if (found.length === 0)
    throw new Error("Purlis's dark names no mono face to make the stories' monospace from");
  return found;
}

/** `theme` as a story draws it: each monospace slot that names no face of its own drawn first in
 *  {@link STORIES_MONO}. A slot with faces, and every other part of the theme, is as handed. */
export function inStoriesMono<Theme extends Drawn>(theme: Theme): Theme {
  if (!theme.fonts) return theme;
  const fonts = { ...theme.fonts };
  for (const slot of MONO_SLOTS) {
    const font = fonts[slot];
    if (font === undefined || font.faces.length > 0) continue;
    fonts[slot] = { ...font, stack: `"${STORIES_MONO}", ${font.stack}` };
  }
  return { ...theme, fonts };
}

/** Makes {@link STORIES_MONO} from `made`, with the bytes `bytesOf` gives, and settles once
 *  every face is in the document's fonts. */
export async function makeStoriesMono(
  made: Face[],
  bytesOf: (asset: string) => Promise<ArrayBuffer | undefined>,
): Promise<void> {
  await Promise.all(
    made.map(async (face) => {
      const bytes = await bytesOf(face.asset);
      if (bytes === undefined) throw new Error(`no bytes for the mono face ${face.asset}`);
      const one = new FontFace(STORIES_MONO, bytes.slice(0), {
        weight: String(face.weight),
        style: face.style,
      });
      await one.load();
      document.fonts.add(one);
    }),
  );
}
