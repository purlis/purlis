// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import process from "node:process";
import { openThroughTheTrustQuestion } from "../opening.js";
import {
  MAKING,
  TRYING,
  on,
  press,
  pressTheOneOf,
  showing,
  tabsAdded,
  theStepSays,
  watchTheTabs,
} from "../poolsetup.js";
import {
  bringForward,
  closeTheViewTabsOpenedSince,
  runFromThePalette,
  tabNames,
} from "../reach.js";

/**
 * **Pool setup's cluster path, walked in the real window** (#4494): from Add a cluster pool… to
 * Try it's last screen, on a bare kind cluster this job made, through the run's own kubeconfig
 * as a person's own sign-in. Where, Check the cluster, the four parts of Install (agent-sandbox,
 * purlis's definitions, the signing key and the operator, of this build's image by digest), a Pool
 * of this build's runner image made and its runner claimed, Sign-ins once the pool is marked as
 * the person's own in Settings › Pools, and a real chat started on the runner's pod, seen to start
 * by its harness's start hook there. Nothing is typed into a terminal, and no tab is opened but
 * Pool setup, Settings and Try it's own chat: a step that needed a shell would fail here.
 *
 * Agent identity is skipped, as on Docker (`pool-setup-docker.pool.e2e.ts`): its main button
 * signs in to GitHub, which no run reaches.
 *
 * **In Pool setup's own run** (`wdio.pool.conf.ts`), on Linux's scenario job, which makes the
 * kind cluster, its registry and the operator's image (`.github/workflows/ci.yml`).
 */

const POOL = "kind-team";
/** The runner image, by digest, in the job's own registry, which the cluster's nodes pull from. */
const IMAGE = process.env.PURLIS_E2E_POOL_IMAGE ?? "";
/** This build's cluster operator, by digest, in the same registry. */
const OPERATOR = process.env.PURLIS_E2E_CLUSTER_OPERATOR_IMAGE ?? "";
/** agent-sandbox's controller and the operator are pulled and started: a minute or two each. */
const INSTALLING = 360_000;

/**
 * Goes on from step `done` to step `next`: the guide moves on by itself from a step that is done,
 * and where it waits on `done` instead, its Continue is pressed.
 */
async function goOnFrom(done: string, next: string, timeout = MAKING): Promise<void> {
  await browser
    .waitUntil(async () => [done, next].includes(await showing()), { timeout, interval: 500 })
    .catch(async () => {
      throw new Error(`Pool setup never got past ${done}; it shows ${await showing()}`);
    });
  if ((await showing()) === done) await press("Continue", timeout);
  await on(next, timeout);
}

/** Opens the Options under the step on show, unless they are open already. */
async function openTheOptions(): Promise<void> {
  const options = await $("aria/Options");
  await options.waitForClickable({ timeout: 30_000 }).catch(async () => {
    throw new Error(`no Options to open: ${await theStepSays()}`);
  });
  if ((await options.getAttribute("aria-expanded")) !== "true") await options.click();
}

/** Types `value` into the box called `name`, once it shows. */
async function type(name: string, value: string): Promise<void> {
  const box = await $(`aria/${name}`);
  await box.waitForDisplayed({ timeout: 30_000 }).catch(async () => {
    throw new Error(`no ${name} box: ${await theStepSays()}`);
  });
  await box.setValue(value);
}

/**
 * **Turns This is my own on**, in Settings › Pools, for the one cluster pool kept here. A React
 * Aria switch's input is drawn hidden, so its row is pressed, by its role.
 */
async function markAsMyOwn(): Promise<void> {
  const on = () =>
    browser.execute(() => {
      const control = Array.from(document.querySelectorAll('[role="switch"]')).find(
        (one) => (one.closest("label")?.textContent ?? "").trim() === "This is my own",
      );
      if (!control) return "none";
      const checked =
        control.getAttribute("aria-checked") === "true" ||
        (control instanceof HTMLInputElement && control.checked);
      if (!checked) control.closest("label")?.click();
      return checked ? "on" : "pressed";
    });
  await browser
    .waitUntil(async () => (await on()) === "on", { timeout: 60_000, interval: 1_000 })
    .catch(async () => {
      throw new Error(`This is my own never turned on: ${await on()}`);
    });
}

describe("Pool setup on a kind cluster", () => {
  let wereOpen: string[] | null = null;

  after(async () => {
    if (wereOpen !== null) await closeTheViewTabsOpenedSince(wereOpen);
  });

  it("walks from Add a cluster pool to a real chat started on the cluster, with no terminal", async () => {
    expect(IMAGE).toMatch(/@sha256:[0-9a-f]{64}$/);
    expect(OPERATOR).toMatch(/@sha256:[0-9a-f]{64}$/);
    await openThroughTheTrustQuestion({ within: 5_000, ifAsked: true });
    wereOpen = await tabNames();
    await watchTheTabs();

    await runFromThePalette("Add a cluster pool", "pool.add-cluster");

    // Where chats run: the kubeconfig's one context, as purlis recommends.
    await on("Where chats run");
    await press("Continue with this cluster");

    // Check the cluster: doctor finds only agent-sandbox missing, which Install adds, so Check is
    // done and the guide goes on to Install by itself.
    await goOnFrom("Check the cluster", "Install");

    // Install, a part at a time, each offered once the one before it is there.
    await on("Install");
    // The operator's image, by digest, under Options: no release names one yet.
    await openTheOptions();
    await type("Operator image", OPERATOR);
    await press("Install agent-sandbox");
    await press("Apply the definitions", INSTALLING);
    await press("Make the signing key", INSTALLING);
    await press("Install the operator", INSTALLING);
    await goOnFrom("Install", "Pool", INSTALLING);

    // Pool: one made in the cluster, of this build's runner image, then this person's runner of
    // it claimed and paired here, once its cluster operator key is shown.
    await openTheOptions();
    await type("Name", POOL);
    await type("Runner image", IMAGE);
    await press("Create the pool");
    await press("Add the pool here", MAKING);
    await expect($("aria/Cluster operator key")).toBeDisplayed();
    await press("Pin the key and add the pool", MAKING);

    // Sign-ins: the pool is no one's own until the person says so, in Settings › Pools.
    await on("Sign-ins", MAKING);
    await press("Open Settings › Pools");
    await markAsMyOwn();
    await bringForward("Pool setup");
    // Then the project's profile, kept on the runner, which says it is signed in.
    await pressTheOneOf(["Sign in 1 profile", "Save and continue", "Continue"], MAKING);

    // Agent identity: skipped (above).
    await on("Agent identity", MAKING);
    await press("Skip for now");

    // Try it: a real chat on the runner's pod, seen to start by its harness's start hook there.
    await on("Try it");
    await press("Start a test chat");
    await $(`aria/${POOL} works, except for this`)
      .waitForDisplayed({ timeout: TRYING, interval: 1_000 })
      .catch(async () => {
        throw new Error(`Try it never passed: ${await theStepSays()}`);
      });
    await expect($("aria/" + `Open a chat on ${POOL}`)).toBeClickable();

    // No step opened a shell, or any tab but its own, Settings and Try it's chat.
    const added = await tabsAdded();
    expect(added).toContain("Pool setup");
    expect(
      added.filter(
        (tab) => tab !== "Pool setup" && tab !== "Settings" && !tab.startsWith("Try it on"),
      ),
    ).toEqual([]);
  });
});
