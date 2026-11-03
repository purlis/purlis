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
  it("counts one", () => {
    expect(OWN_TABS_SAYS(1)).toMatch(/^1 /);
  });

  it("counts several", () => {
    expect(OWN_TABS_SAYS(2)).toMatch(/^2 /);
  });
});
