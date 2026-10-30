// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  cloneRepos,
  clonesInAll,
  cloningWords,
  forgetRepoClones,
  stopRepoClones,
} from "./repoClones";

/**
 * **A repo asked for while another call is already cloning it** (#3649's review, note 4): the
 * later call is not answered "cloned" before that clone is done. It waits its turn in the
 * workspace's chain and answers with how the clone it waited on ended.
 */

afterEach(() => {
  forgetRepoClones();
  clearMocks();
});

type Told = { onmessage: (now: { stage: string; percent: number }) => void };

function heldCore({ holdPanels = false }: { holdPanels?: boolean } = {}) {
  const held: {
    land: () => void;
    refuse: (why: unknown) => void;
    stop: () => void;
    tell: (stage: string, percent: number) => void;
  }[] = [];
  const asked: string[] = [];
  const cancelled: string[] = [];
  /** Each held read of the workspace's panels, answered by calling it (`holdPanels`). */
  const panels: (() => void)[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "take_repos") return null;
    if (cmd === "workspace_panels")
      return holdPanels
        ? new Promise((answer) => panels.push(() => answer({ absent: [] })))
        : { absent: [] };
    if (cmd === "clone_repo") {
      const { repo, progress } = args as { repo: string; progress: Told };
      asked.push(repo);
      return new Promise((land, refuse) =>
        held.push({
          land: () => land([`✓ cloned`]),
          refuse,
          stop: () => land(null),
          tell: (stage, percent) => progress.onmessage({ stage, percent }),
        }),
      );
    }
    if (cmd === "cancel_clone_repo") cancelled.push(String((args as { repo: string }).repo));
    return null;
  });
  return { held, asked, cancelled, panels };
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
    // D-3898-2's sentence: what the waiting call says of a clone left unfinished.
    expect(await waiting).toEqual([
      { repo: "web", said: "web was not finished cloning. Ask for it again." },
    ]);
  });
});

describe("a call that stops part way (#4003)", () => {
  it("leaves no repo stuck waiting: a later ask clones it", async () => {
    const { held, asked } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api", "web"], () => {
      throw new Error("a listener broke");
    });
    await expect.poll(() => held.length).toBe(1);
    held[0].land();
    await expect(first).rejects.toThrow("a listener broke");
    // web was never reached. Asked again, it is cloned, not answered "not finished".
    const again = cloneRepos("/p", "gamma", ["web"]);
    await expect.poll(() => held.length).toBe(2);
    held[1].land();
    expect(await again).toEqual([]);
    expect(asked).toEqual(["api", "web"]);
  });
});

describe("a repo waited on and asked for again before the wait ends (#4003)", () => {
  it("is answered with the clone the call waited on, not the later ask's", async () => {
    const { held } = heldCore();
    // A clones api, then x.
    const a = cloneRepos("/p", "gamma", ["api", "x"]);
    // B waits on api.
    const b = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(1);
    held[0].land();
    // A is on x now; C asks for api again, which sets it waiting.
    await expect.poll(() => held.length).toBe(2);
    const c = cloneRepos("/p", "gamma", ["api"]);
    held[1].land();
    expect(await a).toEqual([]);
    // B reports how A's clone of api ended: cloned.
    expect(await b).toEqual([]);
    await expect.poll(() => held.length).toBe(3);
    held[2].land();
    expect(await c).toEqual([]);
  });
});

describe("a clone's progress (#4079)", () => {
  it("reaches the repo's state, and the listener, as it comes", async () => {
    const { held } = heldCore();
    const moves: [string, string, number][] = [];
    const states: unknown[] = [];
    const first = cloneRepos("/p", "gamma", ["api"], undefined, (repo, now) => {
      moves.push([repo, now.stage, now.percent]);
      states.push(cloningWords({ state: "cloning", ...now }));
    });
    await expect.poll(() => held.length).toBe(1);
    held[0].tell("Downloading", 45);
    expect(moves).toEqual([["api", "Downloading", 45]]);
    expect(states).toEqual(["Downloading 45%"]);
    held[0].land();
    expect(await first).toEqual([]);
  });

  it("says nothing of a stage before git has said one", () => {
    expect(cloningWords({ state: "cloning" })).toBe("");
  });

  it("is ignored once the clone has answered, so a late line never makes it cloning again", async () => {
    const { held } = heldCore();
    const moves: number[] = [];
    const first = cloneRepos("/p", "gamma", ["api"], undefined, (_, now) => {
      moves.push(now.percent);
    });
    await expect.poll(() => held.length).toBe(1);
    held[0].land();
    await first;
    held[0].tell("Writing files", 100);
    expect(moves).toEqual([]);
    // Not under way: it can be asked for again.
    const again = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(2);
    held[1].land();
    expect(await again).toEqual([]);
  });

  it("says a batch's overall percent, whichever repo it is on", () => {
    expect(clonesInAll(0, 2, 50)).toBe(25);
    expect(clonesInAll(1, 2, 0)).toBe(50);
    expect(clonesInAll(1, 2, 50)).toBe(75);
    expect(clonesInAll(2, 2, 0)).toBe(100);
  });
});

describe("a clone the person stopped (#4079)", () => {
  it("stops git, leaves the repo askable again, and is no failure of the forge's", async () => {
    const { held, cancelled } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(1);
    stopRepoClones("/p", "gamma", ["api"]);
    await expect.poll(() => cancelled).toEqual(["api"]);
    held[0].stop();
    expect(await first).toEqual([{ repo: "api", said: "api was stopped.", stopped: true }]);
    const again = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(2);
    held[1].land();
    expect(await again).toEqual([]);
  });

  it("never starts the repos still waiting behind it", async () => {
    const { held, asked } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api", "web"]);
    await expect.poll(() => held.length).toBe(1);
    stopRepoClones("/p", "gamma", ["api", "web"]);
    held[0].stop();
    expect(await first).toEqual([
      { repo: "api", said: "api was stopped.", stopped: true },
      { repo: "web", said: "web was stopped.", stopped: true },
    ]);
    expect(asked).toEqual(["api"]);
  });

  it("is told to a call that waited on it", async () => {
    const { held } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api"]);
    const waiting = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(1);
    stopRepoClones("/p", "gamma", ["api"]);
    held[0].stop();
    await first;
    expect(await waiting).toEqual([{ repo: "api", said: "api was stopped.", stopped: true }]);
  });
});

describe("a clone the forge refused a sign-in for (#4079)", () => {
  it("carries the server to sign in to, for the card's way to Accounts", async () => {
    const { held } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(1);
    held[0].refuse({ said: "api: Sign in to git.corp.example.", sign_in: "git.corp.example" });
    expect(await first).toEqual([
      { repo: "api", said: "api: Sign in to git.corp.example.", signIn: "git.corp.example" },
    ]);
  });
});

describe("a stop that comes too late for its run (#4176)", () => {
  it("is not kept for the next ask, when it came while the clone was being checked", async () => {
    const { held, asked, panels } = heldCore({ holdPanels: true });
    const first = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(1);
    held[0].land();
    // git has answered; the workspace is being read to check it. Cancel is pressed now.
    await expect.poll(() => panels.length).toBe(1);
    stopRepoClones("/p", "gamma", ["api"]);
    panels[0]();
    expect(await first).toEqual([]);
    // Asked again (a Retry, or a re-clone once it is forgotten): it is cloned, not "stopped".
    const again = cloneRepos("/p", "gamma", ["api"]);
    await expect.poll(() => held.length).toBe(2);
    held[1].land();
    await expect.poll(() => panels.length).toBe(2);
    panels[1]();
    expect(await again).toEqual([]);
    expect(asked).toEqual(["api", "api"]);
  });

  it("is not kept for the next ask, when its run stopped part way before reaching the repo", async () => {
    const { held, asked } = heldCore();
    const first = cloneRepos("/p", "gamma", ["api", "web"], () => {
      throw new Error("a listener broke");
    });
    await expect.poll(() => held.length).toBe(1);
    // web is still waiting: Cancel marks it, then the run stops part way before reaching it.
    stopRepoClones("/p", "gamma", ["web"]);
    held[0].land();
    await expect(first).rejects.toThrow("a listener broke");
    // The Retry the "not finished" sentence asks for clones it.
    const again = cloneRepos("/p", "gamma", ["web"]);
    await expect.poll(() => held.length).toBe(2);
    held[1].land();
    expect(await again).toEqual([]);
    expect(asked).toEqual(["api", "web"]);
  });
});
