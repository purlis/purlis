// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { closeTab, focusOn, press, projectRoot, tabTexts, typeInto, untilSays } from "../acting.js";
import { showView } from "../opening.js";

/**
 * **A persona's mark takes any colour typed as `#rrggbb`** (#1454, c8cad841), in the built app
 * against the real core: in `steward`'s own tab, the box under the swatches writes a colour
 * into `personas/steward/persona.md` at Enter, and the picker shows it as the colour picked; a
 * value that is not `#rrggbb` is marked invalid and nothing is written.
 *
 * **Enter is asserted by what the app did with it.** The WebDriver plugin's Enter is an
 * untrusted event, and a form's submission at Enter is the browser's default action, which an
 * untrusted key may not take. So where the driver's Enter wrote nothing, the box's form is
 * submitted from the page, which is what the key does (`requestSubmit`), and the spec reads the
 * definition on disk either way.
 *
 * **It leaves the project as it found it**: `persona.md` is written back as it was, and the
 * persona's tab is closed.
 */

const PERSONA = "steward";
const PANE = `[data-testid="view-pane-charter-persona-${PERSONA}"]`;
const PICKER = `${PANE} section[aria-label="Icon and colour"]`;
const BOX = `${PICKER} input[aria-label="Any colour, as #rrggbb"]`;
const COLOUR = "#3a7bd5";

describe("a persona's mark, in any colour", function () {
  this.timeout(180_000);

  let file = "";
  let was = "";

  /** Enter in the box: the driver's, else the form's own submission from the page. */
  async function enter(landed: () => Promise<boolean>): Promise<void> {
    await focusOn(BOX);
    await browser.keys(["Enter"]);
    const came = await browser
      .waitUntil(landed, { timeout: 3_000, interval: 150 })
      .then(() => true)
      .catch(() => false);
    if (came) return;
    await browser.execute((css: string) => {
      document.querySelector<HTMLInputElement>(css)?.form?.requestSubmit();
    }, BOX);
  }

  before(async () => {
    file = join(await projectRoot(), "personas", PERSONA, "persona.md");
    was = readFileSync(file, "utf8");
    // Personas is a view of the right side (#1678).
    await showView("Personas");
    await untilSays('[data-testid="panel-personas"]', PERSONA);
    await press('[data-testid="panel-personas"]', PERSONA, "containing");
    await $(BOX).waitForExist({ timeout: 20_000 });
  });

  after(async () => {
    writeFileSync(file, was);
    if ((await tabTexts()).includes(PERSONA)) await closeTab(PERSONA);
    await showView("Inbox");
  });

  it("marks a value that is not #rrggbb invalid and writes nothing", async () => {
    await typeInto(BOX, "#12g45z");

    await enter(async () => (await $(BOX).getAttribute("aria-invalid")) === "true");

    await browser.waitUntil(async () => (await $(BOX).getAttribute("aria-invalid")) === "true", {
      timeout: 10_000,
      timeoutMsg: "the box never said its value was not a colour",
    });
    expect(readFileSync(file, "utf8")).toBe(was);
  });

  it("writes any #rrggbb into the definition at Enter, and shows it as the colour picked", async () => {
    const written = () => /^color:\s*["']?#3a7bd5["']?\s*$/im.test(readFileSync(file, "utf8"));
    await typeInto(BOX, COLOUR);

    await enter(async () => written());

    await browser.waitUntil(async () => written(), {
      timeout: 20_000,
      timeoutMsg: `${PERSONA}'s definition never took ${COLOUR}`,
    });
    // The rest of the definition is as it was: its name and its body are still there.
    expect(readFileSync(file, "utf8")).toContain(`name: ${PERSONA}`);
    // Drawn as the colour picked, and held in the box, ready to change.
    const swatch = await $(`${PICKER} input[type="radio"][aria-label="${COLOUR}"]`);
    await swatch.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(async () => swatch.isSelected(), {
      timeout: 10_000,
      timeoutMsg: `${COLOUR} is not the colour the picker shows as picked`,
    });
    expect(await $(BOX).getValue()).toBe(COLOUR);
    expect(await $(BOX).getAttribute("aria-invalid")).not.toBe("true");
  });
});
