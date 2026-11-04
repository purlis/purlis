// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { OWN_TABS_SAYS } from "./EndingChat";

/**
 * **An end ends no task in a tab of its own without the answer** (#1489): End… says how many
 * of the chat's tasks have a tab of their own, since those tabs go either way. A close ends
 * nothing at all (#4756), so it asks nothing and says nothing of tasks.
 */
describe("what End… says of the chat's tasks in tabs of their own", () => {
  it("says that one task's tab closes with it, and that the task stays in the Chats list", () => {
    expect(OWN_TABS_SAYS(1)).toBe(
      "1 of its tasks has a tab of its own. That tab closes with this one: a task that goes on working stays in the Chats list.",
    );
  });

  it("says how many tasks' tabs close with it, in one sentence for them all", () => {
    expect(OWN_TABS_SAYS(2)).toBe(
      "2 of its tasks have tabs of their own. Those tabs close with this one: tasks that go on working stay in the Chats list.",
    );
  });
});
