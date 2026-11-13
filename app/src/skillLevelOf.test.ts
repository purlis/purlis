// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { levelOf } from "./skillChanges";
import { AT_THE_ROOT, asPersona, asPersonaIn, inWorkspace } from "./skillStudio";

describe("the level a studio row's changes act on", () => {
  it("is the workspace the studio shows for a workspace's own row", () => {
    const row = { level: "workspace", local: true, copy_workspace: null } as const;
    expect(levelOf(row, inWorkspace("ops"))).toEqual({ at: "workspace", name: "ops", local: true });
    expect(levelOf(row, asPersona("steward"))).toBeNull();
    expect(levelOf(row, AT_THE_ROOT)).toBeNull();
  });

  it("is the copy's own workspace for one a persona names by its workspace (SK-5c, #6041)", () => {
    const row = { level: "workspace", local: false, copy_workspace: "ops" } as const;
    for (const scope of [asPersona("steward"), inWorkspace("smart-ide"), AT_THE_ROOT])
      expect(levelOf(row, scope)).toEqual({ at: "workspace", name: "ops", local: false });
  });

  it("is the workspace a persona was dispatched into, for that workspace's own row (#2935)", () => {
    const row = { level: "workspace", local: true, copy_workspace: null } as const;
    expect(levelOf(row, asPersonaIn("steward", "ops"))).toEqual({
      at: "workspace",
      name: "ops",
      local: true,
    });
    const own = { level: "persona", local: false, copy_workspace: null } as const;
    expect(levelOf(own, asPersonaIn("steward", "ops"))).toEqual({
      at: "persona",
      name: "steward",
      local: false,
    });
  });
});
