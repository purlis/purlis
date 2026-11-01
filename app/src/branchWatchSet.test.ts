// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { WatchedBranch } from "./bindings";
import { listenToBranches, newestListen } from "./branchWatchSet";

const branch = (repo: string, piece: string | null = null) =>
  ({ plane: "p", workspace: "ops", repo, piece }) as WatchedBranch;

function sets(): unknown[] {
  const sent: unknown[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "branch_watch") sent.push((args as { branches: unknown }).branches);
    return null;
  });
  return sent;
}

describe("the branches a window listens to (#4377)", () => {
  const one = Symbol("one");
  const other = Symbol("other");
  afterEach(async () => {
    await listenToBranches(one, []);
    await listenToBranches(other, []);
    clearMocks();
  });

  it("tells the core every owner's, each once, so neither replaces the other's", () => {
    const sent = sets();
    void listenToBranches(one, [branch("svc"), branch("web")]);
    void listenToBranches(other, [branch("svc")]);
    expect(sent.at(-1)).toEqual([branch("svc"), branch("web")]);

    void listenToBranches(other, [branch("api", "fix")]);
    expect(sent.at(-1)).toEqual([branch("svc"), branch("web"), branch("api", "fix")]);
  });

  it("keeps the other owner's when one lets go", () => {
    const sent = sets();
    void listenToBranches(one, [branch("svc")]);
    void listenToBranches(other, [branch("web")]);
    void listenToBranches(one, []);
    expect(sent.at(-1)).toEqual([branch("web")]);
  });

  it("never asks to listen to nothing where nothing was listened to", () => {
    const sent = sets();
    void listenToBranches(one, []);
    expect(sent).toEqual([]);
  });

  it("names the newest set from any owner, which the core answers last", () => {
    sets();
    const first = listenToBranches(one, [branch("svc")]);
    const second = listenToBranches(other, [branch("web")]);
    expect(newestListen()).toBe(second);
    expect(newestListen()).not.toBe(first);
  });
});
