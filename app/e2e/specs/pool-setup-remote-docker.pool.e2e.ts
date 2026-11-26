// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { expect } from "@wdio/globals";
import process from "node:process";
import { openThroughTheTrustQuestion } from "../opening.js";
import { addAndTryARemoteDockerPool, tabsAdded, watchTheTabs } from "../poolsetup.js";
import { closeTheViewTabsOpenedSince, runFromThePalette, tabNames } from "../reach.js";

/**
 * **Pool setup's path to Docker on another machine, walked in the real window** (#6266, ADR 0123
 * §6): from Add a pool… to Try it's last screen against the fixture of `tests/remote-docker/up.sh`,
 * docker:dind behind a dockerised sshd with latency and loss. Where takes Another machine with
 * Docker; Connect types the machine's address, reads its key with Connect to, and trusts it; the
 * pool's runner is made there and a real chat starts on it. Nothing is typed into a terminal.
 * Walked twice, past Connect's merged page each way (#7191): **Trust this machine only** then the
 * Pool step, and the one press of **Trust and create the pool** (#7780), which takes the image the
 * scenario build names, the fixture's (`PURLIS_E2E_REMOTE_POOL_IMAGE`). Each Add a pool… is a
 * setup of its own, so the second meets the machine's key to trust again.
 *
 * **In Pool setup's own run** (`wdio.pool.conf.ts`), on Linux's scenario job, which serves the
 * fixture. Without it the spec says so and fails: that job always serves one.
 */

const POOL = "scenario-remote";
/** The pool the one press makes, on the same machine. */
const ONE_PRESS_POOL = "scenario-one-press";
/** The runner image, by digest, in the registry inside dind (`tests/remote-docker/up.sh`). */
const IMAGE = process.env.PURLIS_E2E_REMOTE_POOL_IMAGE ?? "";
/** The machine, as a person types it: the fixture's sshd. */
const ADDRESS = "pool@127.0.0.1:2222";

describe("Pool setup on Docker on another machine", () => {
  let wereOpen: string[] | null = null;

  after(async () => {
    if (wereOpen !== null) await closeTheViewTabsOpenedSince(wereOpen);
  });

  it("walks from Add a pool to a real chat on a machine reached over SSH, with no terminal", async () => {
    expect(IMAGE).toMatch(/@sha256:[0-9a-f]{64}$/);
    await openThroughTheTrustQuestion({ within: 5_000, ifAsked: true });
    wereOpen = await tabNames();
    await watchTheTabs();

    await runFromThePalette("Add a pool", "pool.add");

    await addAndTryARemoteDockerPool({ pool: POOL, image: IMAGE, address: ADDRESS });

    const added = await tabsAdded();
    expect(added.filter((tab) => tab !== "Pool setup" && !tab.startsWith("Try it on"))).toEqual([]);
  });

  it("makes the pool in the one press of Trust and create the pool, and a real chat starts on it", async () => {
    expect(IMAGE).toMatch(/@sha256:[0-9a-f]{64}$/);
    await openThroughTheTrustQuestion({ within: 5_000, ifAsked: true });
    wereOpen ??= await tabNames();
    await watchTheTabs();

    await runFromThePalette("Add a pool", "pool.add");

    await addAndTryARemoteDockerPool({
      pool: ONE_PRESS_POOL,
      image: IMAGE,
      address: ADDRESS,
      trusting: "and create the pool",
    });

    const added = await tabsAdded();
    expect(added.filter((tab) => tab !== "Pool setup" && !tab.startsWith("Try it on"))).toEqual([]);
  });
});
