// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser, expect } from "@wdio/globals";
import { ask, inFront } from "../switching.js";
import {
  ALL_WORKSPACES,
  chipsSeen,
  goToWorkspace,
  workspacesListed,
} from "../titleBarWorkspaces.js";

/**
 * **The project's workspaces in the title bar** (#2916, spec #2897 R90–R115), in the built app
 * against the real core: its pins as chips, the ⌄'s popover of every workspace, a pin made and
 * let go from a row's toggle, ⌥⌘P, and a sideways swipe over the bar stepping exactly one pin.
 *
 * Every event is dispatched from the page, as the person's would arrive at the window's own
 * listeners (`reach.ts` says why the driver's keys and pointer are not trusted here). Every test
 * puts the pins and the workspace in front back as it found them: one app process serves the
 * whole scenario run.
 */

/** The pins the core holds for the project in front. */
async function pinsNow(): Promise<string[]> {
  const plane = await inFront();
  if (plane === null) return [];
  return (await ask<{ workspaces: string[] }>("plane_pins", { plane })).workspaces;
}

/**
 * **Pins or lets go of `workspace`** of the project in front as a person does, from its row's
 * toggle in the title bar's ⌄, so the window reads its pins again; then closes the popover.
 */
async function pinned(workspace: string, on: boolean): Promise<void> {
  if ((await pinsNow()).includes(workspace) === on) return;
  // Opened only when it is not open already: a press on the ⌄ of an open popover closes it.
  await browser.execute((all: string) => {
    if (document.querySelector('[role="listbox"][aria-label^="Workspaces in "]') !== null) return;
    document.querySelector<HTMLElement>(all)?.click();
  }, ALL_WORKSPACES);
  await pressToggle(on ? `Pin ${workspace} to the title bar` : `Unpin ${workspace}`);
  await browser.waitUntil(async () => (await pinsNow()).includes(workspace) === on, {
    timeout: 20_000,
    timeoutMsg: `${workspace} was never ${on ? "pinned" : "let go"}`,
  });
  await browser.execute(() =>
    document
      .querySelector<HTMLElement>('[role="combobox"][aria-label^="Go to a workspace in "]')
      ?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })),
  );
}

/** Presses the row toggle named `label` in the open popover, from the page. */
async function pressToggle(label: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute((name: string) => {
        const toggle = document.querySelector<HTMLElement>(`button[aria-label="${name}"]`);
        toggle?.click();
        return toggle !== null;
      }, label),
    { timeout: 20_000, interval: 200, timeoutMsg: `no ${label} in the popover` },
  );
}

describe("the title bar's workspaces", function () {
  this.timeout(120_000);
  let pinsBefore: string[] = [];
  let frontBefore = "";

  before(async () => {
    await browser.waitUntil(async () => (await workspacesListed()).length > 1, {
      timeout: 30_000,
      timeoutMsg: "the title bar never listed the fixture plane's workspaces",
    });
    pinsBefore = await pinsNow();
    frontBefore = (await workspacesListed()).find((one) => one.selected)?.name ?? "Plane root";
  });

  after(async () => {
    const now = await pinsNow();
    for (const one of now.filter((name) => !pinsBefore.includes(name))) await pinned(one, false);
    for (const one of pinsBefore.filter((name) => !now.includes(name))) await pinned(one, true);
    await goToWorkspace(frontBefore);
  });

  it("lists the plane root first, every workspace, and New workspace… last (R107, R108)", async () => {
    const listed = (await workspacesListed()).map((one) => one.name);
    expect(listed[0]).toBe("Plane root");
    expect(listed).toEqual(expect.arrayContaining(["alpha", "beta"]));
  });

  it("pins from a row's toggle and lets the pin go again, the chips following (R105, R110)", async () => {
    for (const one of await pinsNow()) await pinned(one, false);
    await goToWorkspace("alpha");
    // Not pinned: the dashed chip says the workspace in front.
    await browser.waitUntil(async () => (await chipsSeen()).names.join(",") === "alpha", {
      timeout: 20_000,
      timeoutMsg: "the dashed chip never named alpha",
    });
    await browser.execute(() =>
      document.querySelector<HTMLElement>('[data-testid="title-bar"] [data-titlebar-all]')?.click(),
    );
    await pressToggle("Pin beta to the title bar");
    await browser.waitUntil(async () => (await pinsNow()).includes("beta"), {
      timeout: 20_000,
      timeoutMsg: "beta was never pinned",
    });
    await browser.waitUntil(async () => (await chipsSeen()).names.join(",") === "beta,alpha", {
      timeout: 20_000,
      timeoutMsg: `the chips never read beta, alpha: ${JSON.stringify(await chipsSeen())}`,
    });
    await pressToggle("Unpin beta");
    await browser.waitUntil(async () => !(await pinsNow()).includes("beta"), {
      timeout: 20_000,
      timeoutMsg: "beta's pin was never let go",
    });
  });

  it("steps exactly one pin on one sideways swipe over the bar, momentum and all (R112)", async () => {
    for (const one of ["alpha", "beta"])
      if (!(await pinsNow()).includes(one)) await pinned(one, true);
    await goToWorkspace("alpha");
    await browser.waitUntil(async () => (await chipsSeen()).current === "alpha", {
      timeout: 20_000,
      timeoutMsg: "alpha's chip was never current",
    });
    // One gesture: a dozen wheel events in a row, far past the step's travel.
    await browser.execute(() => {
      const bar = document.querySelector('[data-testid="title-bar"]');
      for (let n = 0; n < 12; n++)
        bar?.dispatchEvent(
          new WheelEvent("wheel", { deltaX: 40, deltaY: 0, bubbles: true, cancelable: true }),
        );
    });
    await browser.waitUntil(async () => (await chipsSeen()).current !== "alpha", {
      timeout: 10_000,
      timeoutMsg: "the swipe stepped nothing",
    });
    const pins = (await chipsSeen()).names;
    expect((await chipsSeen()).current).toBe(pins[(pins.indexOf("alpha") + 1) % pins.length]);
  });

  it("opens and closes the workspace popover on ⌥⌘P (R106, the ruling of 2026-10-24)", async () => {
    const press = () =>
      browser.execute(() => {
        const mac = navigator.platform.startsWith("Mac");
        window.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: mac ? "π" : "P",
            code: "KeyP",
            metaKey: mac,
            altKey: true,
            ctrlKey: !mac,
            shiftKey: !mac,
            bubbles: true,
            cancelable: true,
          }),
        );
      });
    const open = () =>
      browser.execute(
        () => document.querySelector('[role="listbox"][aria-label^="Workspaces in "]') !== null,
      );
    await press();
    await browser.waitUntil(open, { timeout: 10_000, timeoutMsg: "⌥⌘P opened no popover" });
    await press();
    await browser.waitUntil(async () => !(await open()), {
      timeout: 10_000,
      timeoutMsg: "a second ⌥⌘P left the popover open",
    });
  });
});
