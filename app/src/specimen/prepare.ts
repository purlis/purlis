// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The theme specimen's page, drawn as the window draws itself** (#2536): the theme put on
 * `root` by the window's own theme module, still (every duration collapsed), tinted with the
 * workspace colour it was handed by the window's own arithmetic, at the window's default text
 * size and the page's own size. Its fonts' faces are made from the bytes the page was handed, as
 * the window makes them from the bytes purlis serves it, and every one is made before this
 * settles, so what is laid out after is laid out in them.
 */
import { DEFAULT_TEXT, drawWindowText } from "../textDefaults";
import { apply, drawIn, drawTint, facesLoaded, fetchFacesWith } from "../theme/theme";
import { handedFace, type Given } from "./page";

export async function prepare(handed: Given, root: HTMLElement): Promise<void> {
  const { theme, width, height } = handed;
  fetchFacesWith((asset) => Promise.resolve(handedFace(handed, asset)));
  drawIn(theme, root);
  // Still: every duration collapsed, as reduced motion collapses it.
  apply(theme, root, true);
  // After the theme, as the window tints it: the tint writes over the tokens it turns.
  drawTint(handed.tint ?? null);
  drawWindowText(DEFAULT_TEXT.window, root);
  root.style.setProperty("--specimen-width", `${width}px`);
  root.style.setProperty("--specimen-height", `${height}px`);
  await facesLoaded();
}
