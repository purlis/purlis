// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

const ran: string[] = [];
const fails = { chats: false, tabs: false };

vi.mock("./chatsLeft.js", () => ({
  chatHooks: {
    beforeAll: async () => void ran.push("chats before"),
    afterAll: async () => {
      ran.push("chats after");
      if (fails.chats) throw new Error("left chats");
    },
  },
}));

vi.mock("./tabsLeft.js", () => ({
  tabHooks: {
    beforeAll: async () => void ran.push("tabs before"),
    afterAll: async () => {
      ran.push("tabs after");
      if (fails.tabs) throw new Error("left tabs");
    },
  },
}));

const { mochaHooks } = await import("./leftBehind.js");
const context = {} as Mocha.Context;

beforeEach(() => {
  ran.length = 0;
  fails.chats = false;
  fails.tabs = false;
});

describe("what a spec file left behind", () => {
  it("is written down chats first, then tabs", async () => {
    await mochaHooks.beforeAll.call(context);
    expect(ran).toEqual(["chats before", "tabs before"]);
  });

  it("is judged for its tabs even when the chats guard failed it, and fails on the chats", async () => {
    fails.chats = true;
    fails.tabs = true;
    await expect(mochaHooks.afterAll.call(context)).rejects.toThrow("left chats");
    expect(ran).toEqual(["chats after", "tabs after"]);
  });

  it("fails on the tabs when only they were left", async () => {
    fails.tabs = true;
    await expect(mochaHooks.afterAll.call(context)).rejects.toThrow("left tabs");
  });

  it("passes when nothing was left", async () => {
    await expect(mochaHooks.afterAll.call(context)).resolves.toBeUndefined();
  });
});
