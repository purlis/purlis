// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * Reading a list the app is drawing — a strip's tabs, their names, their paths — in ONE pass
 * inside the page, so the read cannot fall between two renders.
 *
 * **Why not `$$(…).getElements()` and then a read of each** (charter#506): that is two trips,
 * and a list that changes between them leaves the second holding an element that is gone.
 * WebdriverIO does not fail such a read — it recovers it: it finds the element again by the
 * selector and the index it was found at, and when the list is now shorter it waits for that
 * index to come back. It never does. Measured on macOS and seen on Linux CI, one read of a tab
 * that went between the find and the read held the caller for 80 s, and then failed with the
 * stale element anyway. Inside a `waitUntil` of 30 s that is a timeout whose reason names a
 * stale element — while the strip had long since shown exactly what was being waited for.
 *
 * A read done by one `browser.execute` sees the document as it is at that moment and returns
 * plain values, which cannot go stale. A helper that polls a list which changes while it is
 * polled reads it through here.
 */

/** Each element matching `selector`: the attributes named, read together, in document order. */
export async function attributesOfEach<Name extends string>(
  selector: string,
  names: readonly Name[],
): Promise<Record<Name, string | null>[]> {
  return browser.execute(
    (within: string, wanted: readonly string[]) =>
      [...document.querySelectorAll(within)].map((one) =>
        Object.fromEntries(wanted.map((name) => [name, one.getAttribute(name)])),
      ),
    selector,
    names,
  ) as Promise<Record<Name, string | null>[]>;
}

/**
 * Each element matching `selector`: the text it draws, in document order.
 *
 * `innerText`, trimmed, because that is what WebdriverIO's `getText` answered for the same
 * element: the rendered text, not the markup's.
 */
export async function textOfEach(selector: string): Promise<string[]> {
  return browser.execute(
    (within: string) =>
      [...document.querySelectorAll(within)].map((one) => (one as HTMLElement).innerText.trim()),
    selector,
  );
}

/** What the Inbox answers about one Notice: drawn (with its text), not there, or being opened. */
export type BandNotice =
  { state: "shown"; text: string } | { state: "absent" } | { state: "opening" };

/**
 * The Notice whose cause is `cause`, in the project's Inbox (#1695: the band under the strip
 * folded into it), which this brings on screen when it is put away or showing another view.
 *
 * **A spec must not depend on how many other Notices stand** (#1250): the Inbox lists every one,
 * so a Notice that sorts before the one a spec looks for moves it down and never hides it. A
 * Notice drawn in a view that is not shown is not `shown`: its view's tab is pressed and the
 * answer is `opening`, and the caller's next poll reads it. One `browser.execute`, for the
 * reason the rest of this file gives.
 */
export async function bandNotice(cause: string): Promise<BandNotice> {
  return browser.execute((wanted: string): BandNotice => {
    const found = [...document.querySelectorAll<HTMLElement>("[data-cause]")].find(
      (one) => one.dataset.cause === wanted,
    );
    if (found === undefined) return { state: "absent" };
    if (found.closest("[hidden]") === null) return { state: "shown", text: found.innerText.trim() };
    const inbox = document.querySelector<HTMLButtonElement>(
      '.activity-bar [role="tab"][aria-label="Inbox"]',
    );
    if (inbox === null) return { state: "absent" };
    inbox.click();
    return { state: "opening" };
  }, cause);
}

/** The workspace strip as the window draws it now, for a failure that says what it found. */
export type StripSeen = {
  /** Each workspace tab's drawn name, in strip order. */
  names: string[];
  /** The same tabs' names as the markup holds them, drawn or not. */
  held: string[];
  /** The project tab in front, by its title. */
  project: string | null;
  /** The tab in front on the Tabs strip, by its drawn text. */
  tab: string | null;
  /** The window's inner size, `width×height`. */
  size: string;
};

/** What the workspace strip draws now, read in one pass. */
export async function stripSeen(): Promise<StripSeen> {
  return browser.execute(() => {
    const tabs = [
      ...document.querySelectorAll<HTMLElement>(
        '[data-strip="Workspaces"] [role="tab"] .workspace-name',
      ),
    ];
    const selected = (strip: string) =>
      document.querySelector<HTMLElement>(
        `[data-strip="${strip}"] [role="tab"][aria-selected="true"]`,
      );
    return {
      names: tabs.map((one) => one.innerText.trim()),
      held: tabs.map((one) => (one.textContent ?? "").trim()),
      project: selected("Projects")?.getAttribute("data-directory") ?? null,
      tab: selected("Tabs")?.innerText.trim() ?? null,
      size: `${window.innerWidth}×${window.innerHeight}`,
    };
  });
}

/**
 * **Waits until the workspace strip draws exactly `names`**, in order, and fails saying what it
 * drew instead: the names, the project and tab in front, and the window's size. One app process
 * serves the whole scenario run, so a strip that is not the fixture plane's is a spec before this
 * one that left the window elsewhere, and the failure names where.
 */
export async function untilTheStripLists(
  names: readonly string[],
  timeout = 30_000,
): Promise<void> {
  let last: StripSeen | undefined;
  const wanted = names.join(",");
  await browser
    .waitUntil(
      async () => {
        last = await stripSeen();
        return last.names.join(",") === wanted;
      },
      { timeout, interval: 250 },
    )
    .catch(async () => {
      throw new Error(
        `the strip never listed the fixture plane (${wanted}); it drew ` +
          JSON.stringify(last ?? "nothing it could read") +
          `; the core's pins for it: ${await pinsSaid(last?.project ?? null)}`,
      );
    });
}

/** What the core answers `plane_pins` with for `plane`, as text for a failure's message. */
export async function pinsSaid(plane: string | null): Promise<string> {
  if (plane === null) return "no project in front";
  const said = await browser
    .executeAsync((id: string, done: (out: string) => void) => {
      void window.__TAURI__.core
        .invoke("plane_pins", { plane: id })
        .then((ok) => done(JSON.stringify(ok)))
        .catch((e: unknown) => done(`refused: ${String(e)}`));
    }, plane)
    .catch((e: unknown) => `not asked: ${String(e)}`);
  return said;
}
