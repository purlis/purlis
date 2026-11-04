// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import process from "node:process";
import { openThroughTheTrustQuestion } from "../opening.js";
import { closeTheViewTabsOpenedSince, runFromThePalette, tabNames } from "../reach.js";

/**
 * **Pool setup's Docker path, walked in the real window** (#4985, #4484's first acceptance
 * criterion): from Add a pool… to Try it's last screen, each step's main button pressed, against
 * this machine's Docker and a pool of the runner image this build makes. Nothing is typed into a
 * terminal, and no tab is opened but Pool setup itself and Try it's own chat: a step that needed a
 * shell would fail here.
 *
 * Agent identity is the one step not set up: its main button signs in to GitHub, which no run
 * reaches, so it is skipped, as a project with no repo skips it (ruling C on #4469). Its skip is
 * what makes the last screen read "works, except for this".
 *
 * **A run of its own** (`wdio.pool.conf.ts`), on Linux's scenario job alone, which has Docker.
 */

const POOL = "scenario-pool";
/** The runner image, by digest, in the job's own registry (`wdio.pool.conf.ts`). */
const IMAGE = process.env.PURLIS_E2E_POOL_IMAGE ?? "";
const RAIL = '[aria-label="Steps of Pool setup"]';

/** A runner made from the job's registry, and a profile kept on it: a minute or two on CI. */
const MAKING = 300_000;
/** Try it waits two minutes for its chat's start, and its runner may be woken first. */
const TRYING = 420_000;

/** The step Pool setup is showing, by the name its rail gives it. */
async function showing(): Promise<string> {
  return browser.execute(
    (rail: string) =>
      document.querySelector(`${rail} [aria-current="step"] [data-part="name"]`)?.textContent ?? "",
    RAIL,
  );
}

/** What the step on show says, for a failure to name. */
async function theStepSays(): Promise<string> {
  return browser.execute(
    () => document.querySelector(".pool-setup-page")?.textContent?.replace(/\s+/g, " ") ?? "",
  );
}

/** Waits until Pool setup shows step `name`, at most `timeout`. */
async function on(name: string, timeout = 60_000): Promise<void> {
  await browser
    .waitUntil(async () => (await showing()) === name, { timeout, interval: 500 })
    .catch(async () => {
      throw new Error(
        `Pool setup never reached ${name}; it shows ${await showing()}: ${await theStepSays()}`,
      );
    });
}

/** Presses the control named `name` once it can be pressed, at most `timeout`. */
async function press(name: string, timeout = 30_000): Promise<void> {
  const control = await $(`aria/${name}`);
  await control.waitForClickable({ timeout }).catch(async () => {
    throw new Error(`no ${name} to press: ${await theStepSays()}`);
  });
  await control.click();
}

/** Presses whichever control named one of `names` can be pressed first, at most `timeout`. */
async function pressTheOneOf(names: readonly string[], timeout = 30_000): Promise<void> {
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

/** Every tab the strip has shown since `watch` began, by name. */
async function tabsSeen(): Promise<string[]> {
  return browser.execute(
    () => (window as unknown as { poolSetupTabsSeen?: string[] }).poolSetupTabsSeen ?? [],
  );
}

/** Notes every tab the strip shows from now on, however briefly. */
async function watchTheTabs(): Promise<void> {
  await browser.execute(() => {
    const seen: string[] = [];
    (window as unknown as { poolSetupTabsSeen: string[] }).poolSetupTabsSeen = seen;
    const look = () => {
      for (const tab of document.querySelectorAll('[data-strip="Tabs"] [role="tab"]')) {
        const name = tab.querySelector(".tab-name")?.textContent ?? "";
        if (!seen.includes(name)) seen.push(name);
      }
    };
    look();
    new MutationObserver(look).observe(document.body, {
      subtree: true,
      childList: true,
      characterData: true,
    });
  });
}

describe("Pool setup on Docker", () => {
  let wereOpen: string[] | null = null;

  after(async () => {
    if (wereOpen !== null) await closeTheViewTabsOpenedSince(wereOpen);
  });

  it("walks from Add a pool to a real chat started on the pool, with no terminal", async () => {
    expect(IMAGE).toMatch(/@sha256:[0-9a-f]{64}$/);
    await openThroughTheTrustQuestion({ within: 5_000, ifAsked: true });
    wereOpen = await tabNames();
    await watchTheTabs();

    await runFromThePalette("Add a pool", "pool.add");

    // Where chats run: Docker, as purlis recommends.
    await on("Where chats run");
    await press("Continue with Docker");

    // Pool: its name, the runner image this build makes, and Create.
    await on("Pool");
    const name = await $("aria/Name");
    await name.waitForDisplayed();
    await name.setValue(POOL);
    const image = await $("aria/Image, by digest");
    if (!(await image.isDisplayed().catch(() => false))) {
      // This build names an image of its own: the team's is chosen under Options.
      const options = await $("aria/Options");
      if ((await options.getAttribute("aria-expanded")) !== "true") await options.click();
      await press("My team's own image");
    }
    await image.waitForDisplayed();
    await image.setValue(IMAGE);
    await press("Create the pool");

    // Sign-ins, once the pool and its runner are made: the project's profile, ticked for it,
    // kept on the runner, which says it is signed in. The button reads as what it will do.
    await on("Sign-ins", MAKING);
    await pressTheOneOf(["Sign in 1 profile", "Save and continue", "Continue"], MAKING);

    // Agent identity: skipped (above).
    await on("Agent identity", MAKING);
    await press("Skip for now");

    // Try it: a real chat on the pool, seen to start by its harness's start hook, then closed.
    await on("Try it");
    await press("Start a test chat");
    await browser
      .waitUntil(
        async () =>
          browser.execute(
            (pool: string) =>
              [...document.querySelectorAll(".pool-setup-page h1")].some((heading) =>
                [`${pool} works, except for this`, `${pool} is ready`].includes(
                  heading.textContent ?? "",
                ),
              ),
            POOL,
          ),
        { timeout: TRYING, interval: 1_000 },
      )
      .catch(async () => {
        throw new Error(`Try it never passed: ${await theStepSays()}`);
      });
    // Identity was skipped, so it doesn't read as everything passed.
    await expect($("aria/" + `${POOL} works, except for this`)).toBeDisplayed();
    await expect($("aria/" + `Open a chat on ${POOL}`)).toBeClickable();

    // No step opened a shell, or any tab but its own and Try it's chat.
    const opened = (await tabsSeen()).filter(
      (tab) => !wereOpen?.includes(tab) && tab !== "Pool setup" && !tab.startsWith("Try it on"),
    );
    expect(opened).toEqual([]);
  });
});
