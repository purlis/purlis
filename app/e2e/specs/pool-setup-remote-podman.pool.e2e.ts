// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, expect } from "@wdio/globals";
import process from "node:process";
import { openThroughTheTrustQuestion } from "../opening.js";
import {
  MAKING,
  TRYING,
  on,
  pickTheMachine,
  press,
  pressTheOneOf,
  tabsAdded,
  theStepSays,
  watchTheTabs,
} from "../poolsetup.js";
import { closeTheViewTabsOpenedSince, runFromThePalette, tabNames } from "../reach.js";

/**
 * **Pool setup's path to rootless Podman on another machine, walked in the real window** (#6269,
 * ADR 0123 §2 and §6): from Add a pool… to Try it's last screen against the fixture of
 * `tests/remote-podman/up.sh`, rootless Podman's API service behind a dockerised sshd with latency
 * and loss, kept as a Podman connection of this machine's. Where takes Another machine with
 * Docker; Connect picks that Podman connection, reads its key with Connect to, and trusts it; the
 * pool's runner is made there with Podman's own client and a real chat starts on it. Nothing is
 * typed into a terminal.
 *
 * **In Pool setup's own run** (`wdio.pool.conf.ts`), on Linux's scenario job, which serves the
 * fixture. Without it the spec says so and fails: that job always serves one.
 */

const POOL = "scenario-podman";
/** The runner image, by digest, pulled into that Podman (`tests/remote-podman/up.sh`). */
const IMAGE = process.env.PURLIS_E2E_REMOTE_PODMAN_IMAGE ?? "";
/** The fixture's machine, as `podman system connection list` names it. */
const CONNECTION = "parity-podman";

describe("Pool setup on rootless Podman on another machine", () => {
  let wereOpen: string[] | null = null;

  after(async () => {
    if (wereOpen !== null) await closeTheViewTabsOpenedSince(wereOpen);
  });

  it("walks from Add a pool to a real chat on a Podman reached over SSH, with no terminal", async () => {
    expect(IMAGE).toMatch(/@sha256:[0-9a-f]{64}$/);
    await openThroughTheTrustQuestion({ within: 5_000, ifAsked: true });
    wereOpen = await tabNames();
    await watchTheTabs();

    await runFromThePalette("Add a pool", "pool.add");

    // Where chats run: another machine with Docker.
    await on("Where chats run");
    await press("Another machine with Docker");
    await press("Set up Docker on another machine");

    // Connect: the Podman connection picked, the key read, then trusted.
    await on("Connect");
    await pickTheMachine(CONNECTION);
    await press("Connect to 127.0.0.1", MAKING);
    await press("Trust this machine", MAKING);

    // Every check passed, the machine is trusted, and setup goes on to the pool.
    // Pool: its name and the image, made on the other machine.
    await on("Pool", MAKING);
    const name = await $("aria/Name");
    await name.waitForDisplayed();
    await name.setValue(POOL);
    const image = await $("aria/Image, by digest");
    if (!(await image.isDisplayed().catch(() => false))) {
      const options = await $("aria/Options");
      if ((await options.getAttribute("aria-expanded")) !== "true") await options.click();
      await press("My team's own image");
    }
    await image.waitForDisplayed();
    await image.setValue(IMAGE);
    await press("Create the pool");

    await on("Sign-ins", MAKING);
    await pressTheOneOf(["Sign in 1 profile", "Save and continue", "Continue"], MAKING);

    await on("Agent identity", MAKING);
    await press("Skip for now");

    await on("Try it");
    await press("Start a test chat");
    await $(`aria/${POOL} works, except for this`)
      .waitForDisplayed({ timeout: TRYING, interval: 1_000 })
      .catch(async () => {
        throw new Error(`Try it never passed: ${await theStepSays()}`);
      });

    const added = await tabsAdded();
    expect(added.filter((tab) => tab !== "Pool setup" && !tab.startsWith("Try it on"))).toEqual([]);
  });
});
