// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { withNote } from "./ChatNotices";

describe("a line purlis says on a chat's tab while it runs (#2013)", () => {
  it("is said after its start's own lines, once", () => {
    const held = { 3: { notes: ["started"], agentsMd: [] } };

    const once = withNote(held, 3, "its browsers ended");
    const twice = withNote(once, 3, "its browsers ended");

    expect(once[3]?.notes).toEqual(["started", "its browsers ended"]);
    expect(twice).toBe(once);
  });

  it("is said on a tab that had nothing to say", () => {
    expect(withNote({}, 4, "its browsers ended")).toEqual({
      4: { notes: ["its browsers ended"], agentsMd: [] },
    });
  });
});
