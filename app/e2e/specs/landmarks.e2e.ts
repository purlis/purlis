// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { structureFound } from "../axe.js";
import { noDialogIsLeftUp, showView } from "../opening.js";

/**
 * **The window's landmarks and headings** (#2929, spec #2897, N35; WCAG 2.2 1.3.1 and 2.4.1),
 * in the built app: what a screen reader's rotor lists, and what it jumps between.
 *
 * - axe's own landmark and heading rules pass (`axe.ts`, `STRUCTURE_RULES`): one main, a banner at
 *   the top level, every landmark of a kind named apart, no heading that skips a level or says
 *   nothing.
 * - **The title bar is the window's banner**: outside the main, so it is announced as one, and the
 *   main is the work under it, not the whole window.
 * - **The project rail is a navigation landmark** that names what it navigates.
 * - **Every region F6 stops in is a landmark with a name** (the sides, the editor, the Panel's
 *   halves, #1751), and each side and half shown has a heading that names its view.
 */

/** A landmark's role from its element: its own `role`, else what its tag maps to in place. */
const LANDMARKS =
  'main, header, footer, nav, aside, section[aria-label], section[aria-labelledby], form[aria-label], [role="main"], [role="banner"], [role="contentinfo"], [role="navigation"], [role="complementary"], [role="region"], [role="search"], [role="form"]';

describe("the window's landmarks and headings", function () {
  this.timeout(120_000);

  before(async () => {
    await noDialogIsLeftUp();
    await showView("Asks");
  });

  it("passes axe's landmark and heading rules", async () => {
    expect(await structureFound()).toEqual([]);
  });

  it("draws the title bar as the window's banner, outside the one main that holds the work", async () => {
    const read = await browser.execute((landmarks: string) => {
      const bar = document.querySelector('[data-testid="title-bar"]');
      const mains = [...document.querySelectorAll('main, [role="main"]')];
      return {
        bar: bar?.tagName.toLowerCase() ?? null,
        // A `header` is the banner only when no other landmark or sectioning element holds it.
        barInside:
          bar?.parentElement?.closest(`${landmarks}, article, section`)?.tagName.toLowerCase() ??
          null,
        mains: mains.length,
        mainHoldsBar: mains.some((main) => bar !== null && main.contains(bar)),
        mainHoldsEditor: mains.some((main) => main.querySelector(".region-centre") !== null),
      };
    }, LANDMARKS);
    expect(read).toEqual({
      bar: "header",
      barInside: null,
      mains: 1,
      mainHoldsBar: false,
      mainHoldsEditor: true,
    });
  });

  it("puts the project rail in a navigation landmark that names it", async () => {
    const nav = await browser.execute(() => {
      const rail = document.querySelector('[data-strip="Projects"]');
      const landmark = rail?.closest('nav, [role="navigation"]');
      return landmark ? (landmark.getAttribute("aria-label") ?? "") : null;
    });
    expect(nav).toBe("Projects");
  });

  it("makes every region F6 stops in a named landmark, headed by its view's name", async () => {
    const regions = await browser.execute(() =>
      [
        ...document.querySelectorAll<HTMLElement>(
          ".region-centre, .slot-left, .slot-right, .panel-half[data-side]",
        ),
      ]
        .filter((el) => !el.closest("[hidden]") && el.getBoundingClientRect().width > 0)
        .map((el) => {
          // A resizable panel draws its class on an inner box and its role and name on the box
          // around it (`react-resizable-panels`): the region is whichever of the two says one.
          const landmark = el.hasAttribute("role") ? el : el.parentElement?.closest("[data-panel]");
          return { el, landmark: landmark ?? el };
        })
        .map(({ el, landmark }) => ({
          role: landmark.getAttribute("role"),
          named: (landmark.getAttribute("aria-label") ?? "").trim() !== "",
          // The editor is headed by its tabs; a side or a half by its view's title.
          headed:
            el.classList.contains("region-centre") ||
            [...el.querySelectorAll("h1, h2, h3")].some(
              (h) => (h.textContent ?? "").trim() !== "" && h.getBoundingClientRect().width > 0,
            ),
          which: el.className.split(" ")[0] + (el.dataset.side ? `:${el.dataset.side}` : ""),
        })),
    );
    expect(regions.map((one) => one.which)).toEqual(
      expect.arrayContaining(["region-centre", "region-slot"]),
    );
    expect(regions).toEqual(
      regions.map((one) => ({ ...one, role: "region", named: true, headed: true })),
    );
  });
});
