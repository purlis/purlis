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

/**
 * **Gives the Pool step the image `image`, by its digest, as a team's own** (#7780): where the
 * build offers an image of its own (the scenario build does, `PURLIS_E2E_REMOTE_POOL_IMAGE`), the
 * Image box is hidden until **My team's own image** is chosen under Options. That choice is a radio
 * whose input React Aria draws hidden, so its row is pressed, as `pickTheMachine` presses a
 * machine's.
 */
export async function typeTheTeamsImage(image: string, timeout = 30_000): Promise<void> {
  const box = await $("aria/Image, by digest");
  if (!(await box.isDisplayed().catch(() => false))) {
    const options = await $("aria/Options");
    if ((await options.getAttribute("aria-expanded")) !== "true") await options.click();
    await browser
      .waitUntil(
        async () =>
          browser.execute(() => {
            const row = Array.from(document.querySelectorAll(".pool-setup-page label")).find(
              (one) => (one.textContent ?? "").trim().startsWith("My team's own image"),
            );
            const radio = row?.querySelector<HTMLInputElement>('input[type="radio"]');
            if (!row || !radio) return false;
            if (!radio.checked) (row as HTMLElement).click();
            return radio.checked;
          }),
        { timeout, interval: 500 },
      )
      .catch(async () => {
        throw new Error(`no My team's own image to choose: ${await theStepSays()}`);
      });
  }
  await box.waitForDisplayed({ timeout }).catch(async () => {
    throw new Error(`no Image, by digest to type in: ${await theStepSays()}`);
  });
  await box.setValue(image);
}

/**
 * **Trusts the machine Connect just read, on the merged page** (#7191): where Connect and Pool are
 * one, the check of the key (or of the CA, over TLS) sits beside "Name the pool" and "Who can sign
 * in to <host>?". The pool is named there, the machine is trusted with **Trust this machine only**,
 * and the Pool step goes on from it, where the runner image is given by its digest.
 */
export async function trustTheMachineOnTheMergedPage(pool: string): Promise<void> {
  await nameThePoolOnTheMergedPage(pool);
  await $("aria/Trust and create the pool").waitForDisplayed({ timeout: 30_000 });
  await press("Trust this machine only", MAKING);
}

/**
 * **Trusts the machine and makes the pool in one press, on the merged page** (#7191, #7780):
 * **Trust and create the pool** takes the runner image the build names. The scenario build names
 * the one its run pushed to the fixture (`PURLIS_E2E_REMOTE_POOL_IMAGE`, read by the app's `e2e`
 * feature alone, `poolfollow::scenario_image`); a build anyone is given names its release's.
 */
export async function trustAndCreateThePoolOnTheMergedPage(pool: string): Promise<void> {
  await nameThePoolOnTheMergedPage(pool);
  await press("Trust and create the pool", MAKING);
}

/** Names the pool on the merged page, once Connect shows it. */
async function nameThePoolOnTheMergedPage(pool: string): Promise<void> {
  const name = await $("aria/Name the pool");
  await name.waitForDisplayed({ timeout: MAKING }).catch(async () => {
    throw new Error(`Connect never showed the merged page: ${await theStepSays()}`);
  });
  await name.setValue(pool);
}

/**
 * How Connect's merged page is gone through: **Trust this machine only** then the Pool step, where
 * the image is typed by its digest, or the one press of **Trust and create the pool**.
 */
export type Trusting = "then the Pool step" | "and create the pool";

/**
 * **Pool setup's walk to Docker on another machine, to a real chat** (#6266, #7188): from Add a
 * pool… to Try it's last screen against the fixture of `tests/remote-docker/up.sh`, past the
 * merged page either way (`trusting`, the two steps where not said). Shared by the walk's own spec
 * and the link-down notice's, which needs the pool made and a chat running before it puts a fault
 * on the link. The window must be open and past the trust question.
 */
export async function addAndTryARemoteDockerPool(of: {
  pool: string;
  image: string;
  address: string;
  trusting?: Trusting;
}): Promise<void> {
  // Where chats run: another machine with Docker.
  await on("Where chats run");
  await press("Another machine with Docker");
  await press("Set up Docker on another machine");

  // Connect: the address typed, the key read, then trusted on the merged page (the pool named there).
  await on("Connect");
  // Another machine than those this one's SSH settings or Podman connections name, where it
  // names any.
  await pickTheMachine("Another machine");
  const address = await $("aria/Its address");
  await address.waitForDisplayed();
  await address.setValue(of.address);
  await press("Connect to 127.0.0.1", MAKING);

  if (of.trusting === "and create the pool") {
    // One press: every check from first contact to a linked runner, of the image the build names.
    await trustAndCreateThePoolOnTheMergedPage(of.pool);
  } else {
    await trustTheMachineOnTheMergedPage(of.pool);
    // Every check passed, the machine is trusted, and setup goes on to the pool.
    // Pool: its name and the image, made on the other machine.
    await on("Pool", MAKING);
    const name = await $("aria/Name");
    await name.waitForDisplayed();
    await name.setValue(of.pool);
    await typeTheTeamsImage(of.image);
    await press("Create the pool");
  }

  await on("Sign-ins", MAKING);
  await pressTheOneOf(["Sign in 1 profile", "Save and continue", "Continue"], MAKING);

  await on("Agent identity", MAKING);
  await press("Skip for now");

  await on("Try it");
  await press("Start a test chat");
  await $(`aria/${of.pool} works, except for this`)
    .waitForDisplayed({ timeout: TRYING, interval: 1_000 })
    .catch(async () => {
      throw new Error(`Try it never passed: ${await theStepSays()}`);
    });
}
