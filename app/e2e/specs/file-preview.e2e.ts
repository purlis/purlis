// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { pressMenuPath, ROW } from "../menus.js";
import {
  closeTab,
  focusWorkspace,
  openMenuOn,
  projectRoot,
  tabTexts,
  untilInFront,
} from "../acting.js";
import { showView } from "../opening.js";

/**
 * **The file preview draws an animated GIF and an SVG from their bytes, and its header has Copy
 * path and Reveal** (#1132, #1143; a6675834, 8ece5a07), in the built app against the real core.
 *
 * The spec puts a two-frame GIF and a small SVG beside the fixture branch's `README.md`
 * (`.worktrees/svc/fix-login`, untracked, which a branch's tree lists), opens the branch's
 * *Files* tab from its menu in the Explorer, and picks each file in the tab's tree:
 *
 * - **The GIF is decoded onto the preview's canvas**, at its own size, and it is read as
 *   animated: it has *Pause* (or *Play* under reduced motion) where the webview has WebCodecs'
 *   `ImageDecoder`, and says it shows its first frame where it has not. Both are right, and
 *   which one a runner's webview gives is not this spec's to choose.
 * - **The SVG is never put into the page as markup**: it is drawn on the canvas where the webview
 *   decodes SVG as an image, and shown as its text where it does not (the one-time probe,
 *   D-1132-2). Either way no element of its own reaches the document.
 * - **Copy path and Reveal are in the preview's header**, enabled. They are not pressed: the
 *   scenario cannot read the clipboard or the file manager, and the commands are the tree row's
 *   (FM-10), held by the core's own tests.
 *
 * **It leaves the project as it found it**: both files are removed, the tab is closed, and the
 * left side shows Chats again.
 */

const PIECE = "fix-login";
const GIF = "e2e-frames.gif";
const SVG = "e2e-drawing.svg";
const SVG_ID = "e2e-drawn-svg";
const TAB = `Files · ${PIECE}`;
const SHOWN = (name: string) => `section.piece-files-shown[aria-label="${name}"]`;

/**
 * A 1 × 1 GIF of two frames, black then white, looping: the header, a two-colour table, the
 * NETSCAPE loop block, then each frame as a graphic control block, an image descriptor and its
 * LZW data (clear, the colour's index, end).
 */
const FRAMES = Buffer.from([
  ...[0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00],
  ...[0x00, 0x00, 0x00, 0xff, 0xff, 0xff],
  ...[0x21, 0xff, 0x0b, ..."NETSCAPE2.0".split("").map((c) => c.charCodeAt(0))],
  ...[0x03, 0x01, 0x00, 0x00, 0x00],
  ...[0x21, 0xf9, 0x04, 0x00, 0x0a, 0x00, 0x00, 0x00],
  ...[0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00],
  ...[0x21, 0xf9, 0x04, 0x00, 0x0a, 0x00, 0x00, 0x00],
  ...[0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x4c, 0x01, 0x00],
  0x3b,
]);

const DRAWING = `<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20" id="${SVG_ID}"><rect width="40" height="20" fill="#3a7bd5"/></svg>\n`;

/** What the preview of `name` draws, read in one pass. */
function drawn(name: string) {
  return browser.execute(
    (shown: string, id: string) => {
      const section = document.querySelector(shown);
      const canvas = section?.querySelector<HTMLCanvasElement>('canvas[role="img"]') ?? null;
      return {
        canvas: canvas === null ? null : { width: canvas.width, height: canvas.height },
        caption: (section?.querySelector("figcaption")?.textContent ?? "").trim(),
        played: section?.querySelector(".piece-image-play") !== null,
        text: (section?.querySelector(".cm-content")?.textContent ?? "").trim(),
        // An element of the file's own in the document would be its markup, put in the page.
        markup: document.getElementById(id) !== null,
      };
    },
    SHOWN(name),
    SVG_ID,
  );
}

/** Picks `name` in the Files tab's tree, from the page. */
async function pick(name: string): Promise<void> {
  const row = `[data-testid="piece-files-tree"] [data-row="file:svc/${PIECE}:${name}"]`;
  await $(row).waitForExist({ timeout: 20_000, timeoutMsg: `the tree never listed ${name}` });
  await browser.execute((css: string) => document.querySelector<HTMLElement>(css)?.click(), row);
  await $(SHOWN(name)).waitForExist({ timeout: 20_000 });
}

describe("the file preview, from bytes", function () {
  this.timeout(180_000);

  let branch = "";

  before(async () => {
    branch = join(await projectRoot(), "workspaces", "alpha", ".worktrees", "svc", PIECE);
    writeFileSync(join(branch, GIF), FRAMES);
    writeFileSync(join(branch, SVG), DRAWING);

    // The branch's menu in the Explorer opens its Files tab (FM-2).
    await showView("Explorer");
    await focusWorkspace("alpha");
    await openMenuOn(`[data-testid="piece-svc-${PIECE}"] .spot`);
    await pressMenuPath(ROW.browseFiles);
    await untilInFront(TAB);
  });

  after(async () => {
    if ((await tabTexts()).includes(TAB)) await closeTab(TAB);
    for (const name of [GIF, SVG]) rmSync(join(branch, name), { force: true });
    await showView("Chats");
  });

  it("decodes an animated GIF onto its canvas, and reads it as animated", async () => {
    await pick(GIF);

    let seen = await drawn(GIF);
    await browser.waitUntil(
      async () => {
        seen = await drawn(GIF);
        return seen.caption.startsWith("1 × 1 · image/gif");
      },
      { timeout: 20_000, timeoutMsg: `the GIF was never drawn: ${JSON.stringify(seen)}` },
    );
    expect(seen.canvas).toEqual({ width: 1, height: 1 });
    // Playing (Pause or Play), or its first frame said as one: never drawn as a still in silence.
    await browser.waitUntil(
      async () => {
        seen = await drawn(GIF);
        return seen.played || seen.caption.includes("its first frame");
      },
      { timeout: 10_000, timeoutMsg: `the GIF was not read as animated: ${JSON.stringify(seen)}` },
    );
  });

  it("draws an SVG as an image or shows its text, and never puts its markup in the page", async () => {
    await pick(SVG);

    let seen = await drawn(SVG);
    await browser.waitUntil(
      async () => {
        seen = await drawn(SVG);
        return seen.caption.startsWith("40 × 20 · image/svg+xml") || seen.text.includes(SVG_ID);
      },
      {
        timeout: 20_000,
        timeoutMsg: `the SVG was neither drawn nor shown: ${JSON.stringify(seen)}`,
      },
    );
    if (seen.canvas !== null) expect(seen.canvas).toEqual({ width: 40, height: 20 });
    expect(seen.markup).toBe(false);
  });

  it("offers Copy path and Reveal in the preview's header", async () => {
    const offered = await browser.execute((shown: string) => {
      const head = document.querySelector(`${shown} header.piece-files-head`);
      return [...(head?.querySelectorAll<HTMLButtonElement>("button") ?? [])].map((button) => ({
        words: (button.textContent ?? "").trim(),
        enabled: !button.disabled && button.getAttribute("aria-disabled") !== "true",
      }));
    }, SHOWN(SVG));

    expect(offered).toContainEqual({ words: "Copy path", enabled: true });
    expect(offered.some((one) => one.words.startsWith("Reveal in ") && one.enabled)).toBe(true);
  });
});
