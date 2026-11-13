// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";
import { TABS } from "./reach.js";

/**
 * **What Pool setup's walks in the window share** (#4985, #6266): reading the step on show and
 * what it says, pressing a step's controls by name, and watching the tabs the walk adds. One copy
 * for each spec of a pool kind (`specs/pool-setup-*.pool.e2e.ts`).
 */

export const RAIL = '[aria-label="Steps of Pool setup"]';

/** A runner made from the job's registry, and a profile kept on it: a minute or two on CI. */
export const MAKING = 300_000;
/** Try it waits two minutes for its chat's start, and its runner may be woken first. */
export const TRYING = 420_000;

/** The step Pool setup is showing, by the name its rail gives it. */
export async function showing(): Promise<string> {
  return browser.execute(
    (rail: string) =>
      document.querySelector(`${rail} [aria-current="step"] [data-part="name"]`)?.textContent ?? "",
    RAIL,
  );
}

/** What the step on show says, for a failure to name. */
export async function theStepSays(): Promise<string> {
  return browser.execute(
    () => document.querySelector(".pool-setup-page")?.textContent?.replace(/\s+/g, " ") ?? "",
  );
}

/** Waits until Pool setup shows step `name`, at most `timeout`. */
export async function on(name: string, timeout = 60_000): Promise<void> {
  await browser
    .waitUntil(async () => (await showing()) === name, { timeout, interval: 500 })
    .catch(async () => {
      throw new Error(
        `Pool setup never reached ${name}; it shows ${await showing()}: ${await theStepSays()}`,
      );
    });
}

/** Presses the control named `name` once it can be pressed, at most `timeout`. */
export async function press(name: string, timeout = 30_000): Promise<void> {
  const control = await $(`aria/${name}`);
  await control.waitForClickable({ timeout }).catch(async () => {
    throw new Error(`no ${name} to press: ${await theStepSays()}`);
  });
  await control.click();
}

/** Presses whichever control named one of `names` can be pressed first, at most `timeout`. */
export async function pressTheOneOf(names: readonly string[], timeout = 30_000): Promise<void> {
  const look = async () => {
    for (const name of names) {
      const control = await $(`aria/${name}`);
      if (await control.isClickable().catch(() => false)) {
        await control.click();
        return true;
      }
    }
    return false;
  };
  await browser.waitUntil(look, { timeout, interval: 500 }).catch(async () => {
    throw new Error(`none of ${names.join(", ")} to press: ${await theStepSays()}`);
  });
}

/** The name of every tab the strip has added since `watchTheTabs` began, in order. */
export async function tabsAdded(): Promise<string[]> {
  return browser.execute(
    () => (window as unknown as { poolSetupTabsAdded?: string[] }).poolSetupTabsAdded ?? [],
  );
}

/**
 * Notes every tab the strip adds from now on, however briefly, each once: a second tab of a name
 * already there counts, as a tab of its own.
 */
export async function watchTheTabs(): Promise<void> {
  await browser.execute((strip: string) => {
    const tabs = () => document.querySelectorAll(`${strip} [role="tab"]`);
    const known = new WeakSet<Element>(tabs());
    const added: string[] = [];
    (window as unknown as { poolSetupTabsAdded: string[] }).poolSetupTabsAdded = added;
    new MutationObserver(() => {
      for (const tab of tabs()) {
        if (known.has(tab)) continue;
        known.add(tab);
        added.push(tab.querySelector(".tab-name")?.textContent ?? "");
      }
    }).observe(document.body, { subtree: true, childList: true });
  }, TABS);
}

/**
 * **Picks the machine called `name` on Connect** (#6269), where Connect lists the machines the
 * person's SSH settings and Podman connections name: the radio of that name in the group the words
 * "Which machine?" name, chosen unless it is chosen already. A React Aria radio's input is drawn
 * hidden, so its row is pressed. Where Connect lists no machine, there is nothing to pick.
 */
export async function pickTheMachine(name: string, timeout = 30_000): Promise<void> {
  await browser
    .waitUntil(
      async () =>
        browser.execute((wanted: string) => {
          const group = Array.from(document.querySelectorAll('[role="radiogroup"]')).find(
            (one) =>
              (one.getAttribute("aria-labelledby") ?? "")
                .split(" ")
                .map((id) => document.getElementById(id)?.textContent?.trim() ?? "")
                .join(" ") === "Which machine?",
          );
          if (!group) return true;
          const row = Array.from(group.querySelectorAll("label")).find((one) =>
            (one.textContent ?? "").startsWith(wanted),
          );
          const radio = row?.querySelector<HTMLInputElement>('input[type="radio"]');
          if (!row || !radio) return false;
          if (!radio.checked) row.click();
          return radio.checked;
        }, name),
      { timeout, interval: 500 },
    )
    .catch(async () => {
      throw new Error(`no machine ${name} to pick: ${await theStepSays()}`);
    });
}
