// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
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
  TABS,
  bringForward,
  closeTheViewTabsOpenedSince,
  runFromThePalette,
  tabNames,
} from "../reach.js";
import { LOGS } from "../processes.js";
import { ask } from "../switching.js";

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
    .waitUntil(
      async () => {
        const at = await showing();
        if (at === next) return true;
        if (at === done) {
          const go = await $("aria/Continue");
          if (await go.isClickable().catch(() => false)) await go.click();
        }
        return false;
      },
      { timeout, interval: 1_000 },
    )
    .catch(async () => {
      throw new Error(
        `Pool setup never got past ${done}; it shows ${await showing()}: ${await theStepSays()}`,
      );
    });
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

/** Settings › Pools' switch of the one cluster pool kept here, by its role and its label. */
const MY_OWN = '//*[@role="switch"][ancestor::label[normalize-space()="This is my own"]]';
/** Whether This is my own says on. */
async function myOwnIsOn(): Promise<boolean> {
  const control = await $(MY_OWN);
  if (!(await control.isExisting())) return false;
  return (await control.getAttribute("aria-checked")) === "true" || (await control.isSelected());
}

/**
 * **Turns This is my own on**, in Settings › Pools, for the one cluster pool kept here, as a
 * keyboard does: the switch focused and Space pressed, once. React Aria draws its input hidden,
 * and a pointer press at where it is drawn lands on whatever the row is redrawn under, as the
 * pools are read again; Space is the switch's own.
 */
async function markAsMyOwn(): Promise<void> {
  await $(MY_OWN)
    .waitForExist({ timeout: 60_000 })
    .catch(async () => {
      throw new Error(`no This is my own in Settings: ${await settingsSay()}`);
    });
  let pressed = false;
  let pressedAt = 0;
  let clicked = false;
  await browser
    .waitUntil(
      async () => {
        if (await myOwnIsOn().catch(() => false)) return true;
        if (!pressed) {
          const focused = await browser.execute((path: string) => {
            const found = document.evaluate(
              path,
              document,
              null,
              XPathResult.FIRST_ORDERED_NODE_TYPE,
              null,
            ).singleNodeValue;
            if (!(found instanceof HTMLElement)) return false;
            found.focus();
            return document.activeElement === found;
          }, MY_OWN);
          if (focused) {
            await browser.keys(" ");
            pressed = true;
            pressedAt = Date.now();
          }
        } else if (!clicked && Date.now() - pressedAt > 15_000 && !(await coreKeepsItMarked())) {
          // Space reached nothing the core heard of: one press of the switch, by its role.
          clicked = await $(MY_OWN)
            .click()
            .then(
              () => true,
              () => false,
            );
        }
        return false;
      },
      { timeout: 60_000, interval: 1_000 },
    )
    .catch(async () => {
      throw new Error(
        `This is my own never turned on (${clicked ? "Space, then a press" : pressed ? "Space" : "never pressed"}): ${await settingsSay()}`,
      );
    });
}

/** Whether the core keeps a cluster pool here marked as the person's own. */
async function coreKeepsItMarked(): Promise<boolean> {
  return ask<{ clusters?: { owned: boolean }[] }>("docker_pools").then(
    (pools) => (pools.clusters ?? []).some((one) => one.owned),
    () => false,
  );
}

/**
 * What the cluster pool's row in Settings › Pools says, what is in front and has the keys, and
 * what the core keeps of the pool's mark, for a failure; a picture of the window beside the
 * run's logs.
 */
async function settingsSay(): Promise<string> {
  mkdirSync(LOGS, { recursive: true });
  await browser.saveScreenshot(join(LOGS, "pool-setup-cluster-my-own.png")).catch(() => undefined);
  const kept = await ask<{ clusters?: { name: string; owned: boolean }[] }>("docker_pools").then(
    (pools) => JSON.stringify(pools.clusters ?? []),
    (why: unknown) => `the core couldn't say: ${String(why)}`,
  );
  const shown = await browser.execute((strip: string) => {
    const front = document.querySelector(`${strip} [role="tab"][aria-selected="true"] .tab-name`);
    const active = document.activeElement;
    const keys = active
      ? `${active.tagName.toLowerCase()}[role=${active.getAttribute("role") ?? ""}] ${(active.textContent ?? "").slice(0, 40)}`
      : "nothing";
    const row = document.querySelector(".pools-about")?.textContent?.replace(/\s+/g, " ");
    return `the tab in front is ${front?.textContent ?? "none"}, the keys go to ${keys}; ${row ? `its row says ${row.slice(0, 300)}` : "no cluster pool's row is on screen"}`;
  }, TABS);
  return `${shown}; the core keeps ${kept}`;
}

describe("Pool setup on a kind cluster", () => {
  let wereOpen: string[] | null = null;
  let walked = false;

  after(async () => {
    // A walk that stopped part way leaves its setup unfinished, which the next Add a pool goes
    // on with, with Start over: pressed, so the other specs of the run begin at Where.
    if (wereOpen === null) return;
    await closeTheViewTabsOpenedSince(wereOpen);
    if (walked) return;
    await runFromThePalette("Add a pool", "pool.add")
      .then(() => press("Start over", 30_000))
      .catch(() => undefined);
    await closeTheViewTabsOpenedSince(wereOpen);
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

    walked = true;

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
