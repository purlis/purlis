// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { drawnCycle, stepRegions } from "./RegionFrame";
import { FOCUS_MODE } from "./keys/focusMode";

/**
 * **F6 stops only where a region is drawn** (#2921): a region focus mode hides, or one out of the
 * flow by `hidden`, `inert` or `display: none`, is passed over, and the editor is always a stop.
 */

afterEach(() => {
  document.body.replaceChildren();
});

/** A window as the frame draws it: the left and right sidebars, the editor, the Panel's halves. */
function frame(): HTMLElement {
  document.body.innerHTML = `
    <main class="window">
      <div class="region-frame" data-cycle="left centre right panel-left">
        <div class="region-slot slot-left"><div class="region-view"><button tabindex="0">Chats row</button></div></div>
        <div class="region-centre"><button tabindex="0">In the editor</button></div>
        <div class="region-slot slot-right"><div class="region-view"><button tabindex="0">Inbox row</button></div></div>
        <div class="region-slot slot-panel">
          <div class="panel-half" data-side="left"><div class="region-view"><button tabindex="0">Repos row</button></div></div>
        </div>
      </div>
    </main>`;
  return the(".region-frame");
}

/** The one element `selector` finds: a fixture's, so its absence is the test's failure. */
function the(selector: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(selector);
  if (found === null) throw new Error(`no ${selector} in the fixture`);
  return found;
}

const button = (name: string): HTMLElement => {
  const found = [...document.querySelectorAll("button")].find((one) => one.textContent === name);
  if (found === undefined) throw new Error(`no button ${name} in the fixture`);
  return found;
};

describe("F6 and the regions drawn", () => {
  it("keeps every region that is drawn, in its order", () => {
    const root = frame();
    expect(drawnCycle(root, ["left", "centre", "right", "panel-left"])).toEqual([
      "left",
      "centre",
      "right",
      "panel-left",
    ]);
  });

  it("passes over what focus mode hides, and keeps the editor", () => {
    const root = frame();
    the("main").setAttribute(FOCUS_MODE, "");
    expect(drawnCycle(root, ["left", "centre", "right", "panel-left"])).toEqual(["centre"]);
    button("In the editor").focus();
    stepRegions(false);
    expect(document.activeElement).toBe(button("In the editor"));
    stepRegions(true);
    expect(document.activeElement).toBe(button("In the editor"));
  });

  it("goes on to the next region again once focus mode is off", () => {
    frame();
    button("In the editor").focus();
    stepRegions(false);
    expect(document.activeElement).toBe(button("Inbox row"));
  });

  it("passes over a region that is hidden, inert or not displayed", () => {
    const root = frame();
    the(".slot-left").setAttribute("inert", "");
    the(".slot-right").style.display = "none";
    the(".panel-half").setAttribute("hidden", "");
    expect(drawnCycle(root, ["left", "centre", "right", "panel-left"])).toEqual(["centre"]);
  });
});
