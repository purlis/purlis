// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";

import type { StudioSkill } from "./bindings";
import { skillFileOf } from "./skillFile";

const repoSkill = (folder: string) => ({ level: "repo", folder }) as unknown as StudioSkill;

describe("a repo skill's file", () => {
  it("is in its clone, at the workspace's root or in its repos folder (ADR 0114)", () => {
    expect(skillFileOf(repoSkill("workspaces/ops/web/.claude/skills/fmt"))).toEqual({
      place: { workspace: "ops", repo: "web", piece: null },
      path: ".claude/skills/fmt/SKILL.md",
    });
    expect(skillFileOf(repoSkill("workspaces/ops/repos/api/.claude/skills/lint"))).toEqual({
      place: { workspace: "ops", repo: "api", piece: null },
      path: ".claude/skills/lint/SKILL.md",
    });
  });

  it("is in the branch folder its copy is in (#4326)", () => {
    expect(skillFileOf(repoSkill("workspaces/ops/.worktrees/web/fix/.claude/skills/fmt"))).toEqual({
      place: { workspace: "ops", repo: "web", piece: "fix" },
      path: ".claude/skills/fmt/SKILL.md",
    });
  });
});
