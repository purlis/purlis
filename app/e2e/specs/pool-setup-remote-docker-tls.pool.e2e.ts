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
 * **Pool setup's path to Docker on another machine over Docker's TLS API, walked in the real
 * window** (#6267, ADR 0123 §2–§4): from Add a pool… to Try it's last screen against the fixture of
 * `tests/remote-docker/up.sh`, docker:dind serving TLS on its port 2376 with the CA it generated.
 * Where takes Another machine with Docker; Connect takes Docker's API over TLS, the machine's
 * address and its CA, certificate and key under Options, reads the CA with Connect to and trusts
 * it; the pool's runner is made there and a real chat starts on it. Nothing is typed into a
 * terminal.
 *
 * **In Pool setup's own run** (`wdio.pool.conf.ts`), on Linux's scenario job, which serves the
 * fixture. Without it the spec says so and fails: that job always serves one.
 */

const POOL = "scenario-remote-tls";
/** The runner image, by digest, in the registry inside dind (`tests/remote-docker/up.sh`). */
const IMAGE = process.env.PURLIS_E2E_REMOTE_POOL_IMAGE ?? "";
/** dind's client files, as `up.sh` copied them to this machine. */
const FILES = process.env.REMOTE_DOCKER_TEST_TLS_DIR ?? "";
/** The machine, as a person types it: dind's TLS port. */
const ADDRESS = "localhost:2376";

describe("Pool setup on Docker on another machine, over TLS", () => {
  let wereOpen: string[] | null = null;

  after(async () => {
    if (wereOpen !== null) await closeTheViewTabsOpenedSince(wereOpen);
  });

  it("walks from Add a pool to a real chat on a machine reached over TLS, with no terminal", async () => {
    expect(IMAGE).toMatch(/@sha256:[0-9a-f]{64}$/);
    expect(FILES).not.toBe("");
    await openThroughTheTrustQuestion({ within: 5_000, ifAsked: true });
    wereOpen = await tabNames();
    await watchTheTabs();

    await runFromThePalette("Add a pool", "pool.add");

    await on("Where chats run");
    await press("Another machine with Docker");
    await press("Set up Docker on another machine");

    // Connect: Docker's API over TLS, the address, then the files under Options.
    await on("Connect");
    const tls = await $("aria/Docker's API over TLS");
    await tls.waitForDisplayed();
    await tls.click();
    const address = await $("aria/The machine's address");
    await address.waitForDisplayed();
    await address.setValue(ADDRESS);
    const ca = await $("aria/Authority (CA) file");
    if (!(await ca.isDisplayed().catch(() => false))) await press("Options");
    await ca.waitForDisplayed();
    await ca.setValue(`${FILES}/ca.pem`);
    await (await $("aria/Certificate")).setValue(`${FILES}/cert.pem`);
    await (await $("aria/Key")).setValue(`${FILES}/key.pem`);
    await press("Connect to localhost", MAKING);
    await press("Trust this machine", MAKING);

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
