// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A story's system monospace is a face the stories ship, never the machine's** (#5425).
 *
 * Purlis Classic names no face of its own: its mono and terminal stacks name the system's
 * (`ui-monospace, Menlo, Consolas, monospace`). The pinned image has none of those names, so
 * Chromium fell back on whatever monospace fontconfig gave it, and which one depended on what the
 * worker had drawn before: ChatText, Badge, MergeViewer and SkillPublish changed size and glyphs
 * by shard. Here each of those names is made, in a story's page only, from the mono faces
 * Purlis ships, so a stack that names the system's monospace draws the same face in every run.
 * The window itself still draws the machine's.
 */

/** The system monospace names a built-in stack reaches before the generic `monospace`, each
 *  made in a story's page from {@link faces}. `systemMono.test.ts` holds every built-in mono and
 *  terminal stack to naming one of them before `monospace`. */
export const SYSTEM_MONO = ["Menlo", "Consolas"] as const;

/** One face a theme's font names: its bytes' digest, weight and style. */
export type Face = { asset: string; weight: number; style: string };

type Themes = Record<string, Record<string, { fonts?: Record<string, { faces: Face[] }> }>>;

/** The faces the system monospace is made from: the mono faces of Purlis's dark. */
export function faces(themes: Themes): Face[] {
  const found = themes.purlis?.dark?.fonts?.mono?.faces ?? [];
  if (found.length === 0)
    throw new Error("Purlis's dark names no mono face to make the stories' system monospace from");
  return found;
}

/** Makes each name in {@link SYSTEM_MONO} from `faces`, with the bytes `bytesOf` gives, and
 *  settles once every one is in the document's fonts. */
export async function makeSystemMono(
  made: Face[],
  bytesOf: (asset: string) => Promise<ArrayBuffer | undefined>,
): Promise<void> {
  await Promise.all(
    SYSTEM_MONO.flatMap((name) =>
      made.map(async (face) => {
        const bytes = await bytesOf(face.asset);
        if (bytes === undefined) throw new Error(`no bytes for the mono face ${face.asset}`);
        const one = new FontFace(name, bytes.slice(0), {
          weight: String(face.weight),
          style: face.style,
        });
        await one.load();
        document.fonts.add(one);
      }),
    ),
  );
}
