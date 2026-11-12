// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, expect } from "@wdio/globals";
import process from "node:process";
import { openThroughTheTrustQuestion } from "../opening.js";
import {
  MAKING,
  TRYING,
  on,
  press,
  pressTheOneOf,
  tabsAdded,
  theStepSays,
  watchTheTabs,
} from "../poolsetup.js";
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
    // Identity was skipped, so it reads "works, except for this", never "is ready".
    await $(`aria/${POOL} works, except for this`)
      .waitForDisplayed({ timeout: TRYING, interval: 1_000 })
      .catch(async () => {
        throw new Error(`Try it never passed: ${await theStepSays()}`);
      });
    await expect($("aria/" + `Open a chat on ${POOL}`)).toBeClickable();

    // No step opened a shell, or any tab but its own and Try it's chat.
    // The watch saw the walk: Pool setup's own tab is among what it saw added.
    const added = await tabsAdded();
    expect(added).toContain("Pool setup");
    expect(added.filter((tab) => tab !== "Pool setup" && !tab.startsWith("Try it on"))).toEqual([]);
  });
});
