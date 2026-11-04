// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { beforeEach, describe, expect, it } from "vitest";
import { forgetStandIn, watched, whyNoPane } from "./storedLayoutGuard.testkit";

describe("the stored-layout guard on window tests' stand-ins (#4758)", () => {
  beforeEach(() => forgetStandIn());

  it("answers exactly what the stand-in answers, at once or later", async () => {
    const core = watched((cmd: string) =>
      cmd === "later" ? Promise.resolve([1, 2]) : cmd === "now" ? 7 : null,
    );

    expect(core("now")).toBe(7);
    await expect(core("later")).resolves.toEqual([1, 2]);
    expect(core("other")).toBeNull();
  });

  it("names the fix when chats were reopened and no layout was answered", async () => {
    const core = watched((cmd: string) =>
      cmd === "opened_chats" ? Promise.resolve([{ session: 1 }]) : null,
    );
    await core("opened_chats");
    core("reopened_layout");

    expect(whyNoPane()).toMatch(/opened_chats with 1 chat.*aTabForEachChat/s);
  });

  it("says nothing once a layout was answered, or when no chat was reopened", () => {
    const laid = watched((cmd: string) =>
      cmd === "opened_chats" ? [{ session: 1 }] : cmd === "reopened_layout" ? { tabs: [] } : null,
    );
    laid("opened_chats");
    laid("reopened_layout");
    expect(whyNoPane()).toBeUndefined();

    forgetStandIn();
    watched((cmd: string) => (cmd === "opened_chats" ? [] : null))("opened_chats");
    expect(whyNoPane()).toBeUndefined();
  });
});
