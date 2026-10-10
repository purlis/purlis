import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { browser, expect, $, $$ } from "@wdio/globals";
import { textOfEach } from "../reading.js";

/**
 * **charter's status line**, against the real app (`app/src/StatusLine.tsx`).
 *
 * The operator asked for the project's directory to come out of the top-right corner and into
 * a one-line bar at the very bottom of the window. Every claim in that sentence is about
 * pixels — *one* line, at the *very* bottom, under the regions — and **jsdom lays no
 * panel group out at all**, so `FourRegions.test.tsx` can only assert document order. This is
 * where the geometry is asked, the same split charter-app#149 made for the regions' widths.
 *
 * It changes nothing it does not put back: one app process serves the whole scenario run.
 */

/** Where the bottom edge of an element is, in the window's own coordinates. */
async function bottomOf(selector: string): Promise<number> {
  const element = await $(selector);
  await element.waitForExist({ timeout: 20_000 });
  const y = (await element.getLocation("y")) as number;
  const height = (await element.getSize("height")) as number;
  return y + height;
}

/** How tall the window's viewport is, which is what "the very bottom" is measured against. */
const viewport = (): Promise<number> =>
  browser.execute(() => document.documentElement.clientHeight);

/** Waits until the plane has been read, so a workspace can be focused. */
async function untilTheStripIsRead(): Promise<void> {
  await browser.waitUntil(
    async () => {
      // In one pass: the strip is drawn while it is polled (charter#506, `reading.ts`).
      const names = await textOfEach('[data-strip="Workspaces"] [role="tab"] .workspace-name');
      return names.join(",") === "alpha,beta";
    },
    { timeout: 30_000, interval: 250, timeoutMsg: "the strip never listed the fixture plane" },
  );
}

/** Focuses a workspace from the strip, by the tab's own name rather than by its whole text —
 *  a strip tab carries counts beside its name. */
async function focus(workspace: string): Promise<void> {
  const tabs = await $$(
    // Not the plane root's icon tab (SI-1), which has no drawn name to compare.
    '[data-strip="Workspaces"] [role="tab"]:not(.plane-root)',
  ).getElements();
  for (const tab of tabs) {
    if ((await tab.$(".workspace-name").getText()) === workspace) {
      await tab.click();
      return;
    }
  }
  throw new Error(`no ${workspace} on the workspace strip`);
}

/** The plane the app says it is acting on — the copy this run was given, never the repo's. */
async function planeRoot(): Promise<string> {
  const said = await $("span.plane code");
  await said.waitForDisplayed({ timeout: 30_000 });
  return (await said.getText()).trim();
}

/** Waits for the Notices button's name, and says what it was when it never gets there. */
async function untilTheButtonSays(want: string): Promise<void> {
  await $('[data-testid="status-alerts"]').waitForExist({ timeout: 20_000 });
  let last: string | null = "";
  try {
    await browser.waitUntil(
      async () => {
        last = await $('[data-testid="status-alerts"]').getAttribute("aria-label");
        return last === want;
      },
      { timeout: 30_000, interval: 250 },
    );
  } catch {
    throw new Error(`the Notices button never said ${want}; it said ${JSON.stringify(last)}`);
  }
}

/** What the Notices button counts once it has a number: none is 0. Other Notices than the
 *  alerts can stand on the runner's machine (a doctor finding, an offer), so a spec counts from
 *  what is there and never assumes none. */
async function noticesCounted(): Promise<number> {
  await $('[data-testid="status-alerts"]').waitForExist({ timeout: 20_000 });
  let said: string | null = null;
  await browser.waitUntil(
    async () => {
      said = await $('[data-testid="status-alerts"]').getAttribute("aria-label");
      return said !== null && said !== "Notices: not counted";
    },
    { timeout: 30_000, interval: 250, timeoutMsg: "the Notices button never counted" },
  );
  const text = String(said);
  return text === "Notices: none" ? 0 : Number(text.replace("Notices: ", ""));
}

/** The Notices button's name for a count. */
const noticesSaid = (count: number) => (count === 0 ? "Notices: none" : `Notices: ${count}`);

/** Presses the status line's Notices button and waits for the Inbox it opens (#1695), asked
 *  for afresh — an element looked up before the view was shown is not the view. */
async function openTheInbox(): Promise<WebdriverIO.Element> {
  await $('[data-testid="status-alerts"]').click();
  const inbox = await $('.region-view[data-view="inbox"]');
  await inbox.waitForDisplayed({ timeout: 20_000 });
  return inbox.getElement();
}

/** Waits for the status line to say something, and says what it did say when it never does. */
async function untilItSays(want: string | RegExp): Promise<void> {
  const line = await $('[data-testid="status-line"]');
  let last = "";
  try {
    await browser.waitUntil(
      async () => {
        last = await line.getText();
        return typeof want === "string" ? last.includes(want) : want.test(last);
      },
      { timeout: 30_000, interval: 250 },
    );
  } catch {
    throw new Error(
      `the status line never said ${want.toString()}; it said ${JSON.stringify(last)}`,
    );
  }
}

describe("the status line", () => {
  it("is the last thing in the window, under the regions", async () => {
    await untilTheStripIsRead();
    await $('.region-views[data-region="aside"]').waitForExist({ timeout: 20_000 });

    const line = await $('[data-testid="status-line"]');
    const top = (await line.getLocation("y")) as number;
    // Nothing is below it: its bottom edge is the viewport's. A pixel of slack, because a
    // fractional layout rounds.
    expect(
      Math.abs((await bottomOf('[data-testid="status-line"]')) - (await viewport())),
    ).toBeLessThanOrEqual(1);
    // And the regions end where it begins, rather than overlapping it or running past it —
    // which is the failure a document-order assertion cannot see. The terminals' centre is
    // the lowest thing above it since the bottom region went (#1676).
    expect(await bottomOf('[data-panel][id="region-centre"]')).toBeLessThanOrEqual(top + 1);
  });

  it("is one line tall, and takes that height from the regions rather than floating over them", async () => {
    await untilTheStripIsRead();

    const line = await $('[data-testid="status-line"]');
    const height = (await line.getSize("height")) as number;
    // One line of 0.8rem text and two small paddings. The bound is deliberately loose — this
    // is a guard against it becoming a panel, not a pin on the font metrics.
    expect(height).toBeGreaterThan(8);
    expect(height).toBeLessThanOrEqual(32);

    // The panel group stops above it. A status line laid over the regions would leave the
    // group's bottom below the line's top, and the terminals' last row unreadable.
    const group = (await (await $(".regions")).getSize("height")) as number;
    expect(group + height).toBeLessThanOrEqual(await viewport());
  });

  it("is outside the panel group, so it is not a region", async () => {
    // `StatusLine.tsx` argues why: a slot is sized as a percentage of its group and this is
    // one line of text; a region can be put away and this must not be.
    await untilTheStripIsRead();

    const inside = await browser.execute(
      () =>
        document
          .querySelector(".regions")
          ?.contains(document.querySelector("[data-testid='status-line']")) ?? null,
    );
    expect(inside).toBe(false);
  });

  it("carries the project's directory, which is no longer on the bar", async () => {
    await untilTheStripIsRead();

    const said = await $('[data-testid="status-line"] .plane code');
    await said.waitForDisplayed({ timeout: 30_000 });
    // The run's own copy of the fixture plane, spelled as an absolute path.
    expect((await said.getText()).trim()).toMatch(/^[/\\]/);

    const onTheBar = await $("header.bar .plane");
    expect(await onTheBar.isExisting()).toBe(false);
  });

  it("says which workspace the window is on, and follows the strip", async () => {
    await untilTheStripIsRead();
    await focus("alpha");
    await untilItSays("alpha");

    await focus("beta");
    await untilItSays("beta");

    // Put back, because one app process serves every spec after this one.
    await focus("alpha");
    await untilItSays("alpha");
  });

  it("counts what the workspace holds, next to the thing it counts", async () => {
    await untilTheStripIsRead();
    await focus("alpha");

    // The fixture's `alpha` holds one todo and one real piece cut off `svc`, on a plane of two
    // workspaces. Every count is the focused workspace's except `ws`, which is the project's —
    // charter's own footer rule: a count lives next to the thing it counts.
    await untilItSays(/todo\s*1/);
    await untilItSays(/branches\s*1/);
    await untilItSays(/ws\s*2/);
  });

  it("drops a count rather than drawing a zero", async () => {
    // `beta` holds no repos at all, so there are no worktrees to count. charter's own footer
    // drops the cell at zero — presence is the signal — and a `branches 0` sitting there every
    // day is furniture by the end of the week.
    await untilTheStripIsRead();
    await focus("beta");
    await untilItSays("beta");

    const line = await $('[data-testid="status-line"]');
    await expect(line).not.toHaveText(expect.stringContaining("branches"));
    await expect(line).not.toHaveText(expect.stringContaining("todo"));

    await focus("alpha");
    await untilItSays("alpha");
  });

  it("opens the Inbox from the Notices button, and moves the window not at all", async () => {
    // The fixture plane is healthy, so purlis has read the project to the end and has a number.
    await untilTheStripIsRead();
    await noticesCounted();

    const inbox = await openTheInbox();
    // The window is not scrolled and is no bigger than itself: a box hanging out of a view made
    // the page taller once, and a click that opened something scrolled the whole window
    // (train 36). Said with the sizes, so a failure names which way it grew.
    const page = await browser.execute(() => {
      const it = document.scrollingElement ?? document.documentElement;
      return `scrolled ${it.scrollLeft},${it.scrollTop} of ${it.scrollWidth}x${it.scrollHeight} in ${it.clientWidth}x${it.clientHeight}`;
    });
    expect(page).toMatch(/^scrolled 0,0 of (\d+)x(\d+) in \1x\2$/);
    // No alert here, so no alert's row.
    expect(await inbox.$$('.notice-list [data-cause^="alert:"]').length).toBe(0);
  });

  it("lists an alert the plane has as a Notice, counts it, and stops counting it once it is fixed", async () => {
    // A workspace behind the current layout: purlis's `reinit` alert, with its Reinit button
    // (NO-6: a row's way out is a button, not a command to type), in the Inbox (#1695).
    await untilTheStripIsRead();
    const plane = await planeRoot();
    // The stamp under whichever name it has: a plane charter laid out carries
    // `.charter-structure` until a write moves it to `.purlis-structure`.
    const stamps = [".purlis-structure", ".charter-structure"].map((n) =>
      join(plane, "workspaces", "beta", n),
    );
    const marker = stamps.find((p) => existsSync(p)) ?? stamps[0];
    const was = readFileSync(marker, "utf8");
    const before = await noticesCounted();
    try {
      writeFileSync(marker, "4\n");
      // Opening the Inbox from the button reads the alerts again, as the drawer's opening did.
      const inbox = await openTheInbox();
      await untilTheButtonSays(noticesSaid(before + 1));
      const row = await inbox.$('.notice-list [data-cause="alert:reinit"]');
      await row.waitForDisplayed({ timeout: 20_000 });
      await expect(row).toHaveText(expect.stringContaining("behind the current layout"));
      await expect(row.$("button=Reinit")).toBeExisting();
      await expect(row).toHaveText(expect.stringContaining("beta"));
    } finally {
      // Put back what this spec changed: one app process serves the whole run.
      writeFileSync(marker, was);
    }
    await openTheInbox();
    await untilTheButtonSays(noticesSaid(before));
  });

  it("draws no pin item for a plane that pins nothing", async () => {
    // The fixture plane pins no purlis version, so `charter version` reports no drift and the
    // pin item — which is drawn only on drift — is not there at all.
    //
    // **The updater used to be asserted here beside it and has moved** to `title-bar.e2e.ts`
    // with the item itself. The two read as one pair of "version facts" and only one of them
    // is about a project: the pin is `charter version`'s verdict on THIS plane's
    // `[charter] version` (ADR 0030) and belongs on the line that names the project,
    // while an offer is about the app and the line is drawn once per open project.
    await untilTheStripIsRead();

    expect(await $('[data-testid="status-pin"]').isExisting()).toBe(false);
  });

  /**
   * **The region toggles are on this line, and they are icons** (charter-app#193).
   *
   * The operator asked twice — *"show hide buttons can be movet to bottom status bar — again
   * like ZED"*, and then *"you dont moved this 3 hide/show bottons to bottom status bar — and
   * let make them without labels, just small icons without texts, texts only with tooltips"*.
   * `FourRegions.test.tsx` owns which regions get one and in what order, where jsdom can read
   * the arrangement; what only a scenario can say is where they ended up in the shipped window
   * and that the accessible name survived the build with no text under it.
   */
  it("carries every region's way back, as icons named for the region", async () => {
    await untilTheStripIsRead();
    await $('[data-testid="explorer"]').waitForExist({ timeout: 20_000 });

    const found = await browser.execute(() => {
      const line = document.querySelector('[data-testid="status-line"]');
      const toggles = [...(line?.querySelectorAll("button[aria-pressed]") ?? [])];
      return {
        named: toggles.map((one) => one.getAttribute("aria-label")),
        // No words in any of them, a mark in every one, and a tooltip that says what pressing
        // does rather than repeating the name.
        worded: toggles.map((one) => (one.textContent ?? "").trim()).filter(Boolean),
        marked: toggles.filter((one) => one.querySelector("svg")).length,
        told: toggles.map((one) => one.getAttribute("title")),
        // And gone from the bar they used to be on.
        onTheBar: document.querySelectorAll("header.bar button[aria-pressed]").length,
      };
    });

    // Two since #1676: the State region along the bottom is the left side's Changes view.
    expect(found.named).toEqual(["Navigation", "Attention"]);
    expect(found.worded).toEqual([]);
    expect(found.marked).toBe(2);
    expect(found.told).toEqual(["Put the Navigation region away", "Put the Attention region away"]);
    expect(found.onTheBar).toBe(0);
  });

  it("stays at the bottom when every region is put away", async () => {
    // It is not in the arrangement, so nothing about it changes when the arrangement does —
    // and the window that is left is the panes and this line.
    await untilTheStripIsRead();
    await $('[data-testid="explorer"]').waitForExist({ timeout: 20_000 });

    const names = ["Navigation", "Attention"];
    for (const name of names)
      await (await $(`button[aria-pressed="true"][aria-label="${name}"]`)).click();
    // Hidden, never unmounted (#1673, #1678): the right side's views are still there.
    await browser.waitUntil(
      async () => !(await $('.region-views[data-region="aside"]').isDisplayed()),
      {
        timeout: 20_000,
        timeoutMsg: "the right-hand region did not go away when it was put away",
      },
    );

    expect(await $('[data-testid="status-line"]').isExisting()).toBe(true);
    expect(
      Math.abs((await bottomOf('[data-testid="status-line"]')) - (await viewport())),
    ).toBeLessThanOrEqual(1);

    for (const name of names)
      await (await $(`button[aria-pressed="false"][aria-label="${name}"]`)).click();
    await $('.region-views[data-region="aside"]').waitForDisplayed({ timeout: 20_000 });
  });
});
