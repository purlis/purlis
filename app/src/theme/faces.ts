// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "../bindings";

/**
 * **A theme's face, as purlis serves it** (#2326): the bytes of the woff2 file whose sha-256 is
 * `asset`, which the core's theme engine read from inside the theme's folder and reads again,
 * under its cap, to answer (`theme_font`, `purlis_core::theme::served`). They cross as base64,
 * as a file tab's image does, and `theme.ts` makes a `FontFace` from them: the window is never
 * handed a path or an address for a font, and nothing fetches one. `undefined` when purlis
 * would not serve it, which draws the family list's fallback.
 *
 * Asked after the first frame, never before it (ADR 0026's 2 s cold start).
 */
export async function fetchFace(asset: string): Promise<ArrayBuffer | undefined> {
  const answered = await commands.themeFont(asset);
  if (answered.status !== "ok") return undefined;
  const text = atob(answered.data);
  const bytes = new Uint8Array(text.length);
  for (let at = 0; at < text.length; at++) bytes[at] = text.charCodeAt(at);
  return bytes.buffer;
}
