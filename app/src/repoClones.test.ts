// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cloneRepos, forgetRepoClones } from "./repoClones";

/**
 * **A repo asked for while another call is already cloning it** (#3649's review, note 4): the
 * later call is not answered "cloned" before that clone is done. It waits its turn in the
 * workspace's chain and answers with how the clone it waited on ended.
 */

afterEach(() => {
  forgetRepoClones();
  clearMocks();
});

function heldCore() {
  const held: { land: () => void; refuse: (why: string) => void }[] = [];
  const asked: string[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "take_repos") return null;
    if (cmd === "workspace_panels") return { absent: [] };
    if (cmd === "clone_repo") {
      asked.push(String((args as { repo: string }).repo));
      return new Promise((land, refuse) => held.push({ land: () => land([`✓ cloned`]), refuse }));
    }
    return null;
  });
  return { held, asked };
}

describe("a repo already being cloned, asked for again", () => {
  it("answers with the failure of the clone it waited on, never a false success", async () => {
    const { held, asked } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api"]);
    const again = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(1);
    held[0].refuse("api: no access.");
    expect(await first).toEqual([{ repo: "api", said: "api: no access." }]);
    expect(await again).toEqual([{ repo: "api", said: "api: no access." }]);
    // It was asked once: the second call waited on the first rather than cloning again.
    expect(asked).toEqual(["api"]);
  });

  it("answers nothing failed when the clone it waited on landed", async () => {
    const { held } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api"]);
    const again = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(1);
    held[0].land();
    expect(await first).toEqual([]);
    expect(await again).toEqual([]);
  });
});

describe("a repo waited on that is not finished when the wait ends (#3898)", () => {
  it("is a failure when a later call asked for it again while this one waited", async () => {
    const { held } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api"]);
    // Waits on api, and clones web of its own.
    const waiting = cloneRepos("/p", "gamma", ["api", "web"]);
    await expect.poll(() => held.length).toBe(1);
    held[0].refuse("api: no access.");
    await first;
    // web is cloning now; api, failed, is asked for again by a third call.
    await expect.poll(() => held.length).toBe(2);
    const third = cloneRepos("/p", "gamma", ["api"]);
    held[1].land();
    expect(await waiting).toEqual([{ repo: "api", said: "api: no access." }]);
    await expect.poll(() => held.length).toBe(3);
    held[2].land();
    expect(await third).toEqual([]);
  });

  it("is a failure when the call that was cloning it stopped part way", async () => {
    const { held } = heldCore();
    // The first call's listener throws after api, so web is left waiting, never cloned.
    const first = cloneRepos("/p", "gamma", ["api", "web"], () => {
      throw new Error("a listener broke");
    });
    const waiting = cloneRepos("/p", "gamma", ["web"]);
    await expect.poll(() => held.length).toBe(1);
    held[0].land();
    await expect(first).rejects.toThrow("a listener broke");
    const failed = await waiting;
    expect(failed.map((one) => one.repo)).toEqual(["web"]);
  });
});
