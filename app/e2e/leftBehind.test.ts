// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

const ran: string[] = [];
const fails = { chats: false, projects: false, tabs: false };

vi.mock("./chatsLeft.js", () => ({
  chatHooks: {
    beforeAll: async () => void ran.push("chats before"),
    afterAll: async () => {
      ran.push("chats after");
      if (fails.chats) throw new Error("left chats");
    },
  },
}));

vi.mock("./projectsLeft.js", () => ({
  projectHooks: {
    beforeAll: async () => void ran.push("projects before"),
    afterAll: async () => {
      ran.push("projects after");
      if (fails.projects) throw new Error("left projects");
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
  fails.projects = false;
  fails.tabs = false;
});

describe("what a spec file left behind", () => {
  it("is written down chats first, then projects, then tabs", async () => {
    await mochaHooks.beforeAll.call(context);
    expect(ran).toEqual(["chats before", "projects before", "tabs before"]);
  });

  it("is judged for its projects and tabs even when the chats guard failed it, and fails on the chats", async () => {
    fails.chats = true;
    fails.projects = true;
    fails.tabs = true;
    await expect(mochaHooks.afterAll.call(context)).rejects.toThrow("left chats");
    expect(ran).toEqual(["chats after", "projects after", "tabs after"]);
  });

  it("is judged for its tabs even when the projects guard failed it, and fails on the projects", async () => {
    fails.projects = true;
    fails.tabs = true;
    await expect(mochaHooks.afterAll.call(context)).rejects.toThrow("left projects");
    expect(ran).toEqual(["chats after", "projects after", "tabs after"]);
  });

  it("fails on the tabs when only they were left", async () => {
    fails.tabs = true;
    await expect(mochaHooks.afterAll.call(context)).rejects.toThrow("left tabs");
  });

  it("passes when nothing was left", async () => {
    await expect(mochaHooks.afterAll.call(context)).resolves.toBeUndefined();
  });
});
