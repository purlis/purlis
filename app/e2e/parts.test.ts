// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { specsOfPart } from "./parts.js";

/** A run's spec files in the order a directory listing may hand them over. */
const FILES = [
  "tab-chip.e2e.ts",
  "activity.e2e.ts",
  "projects.e2e.ts",
  "pane-notices.e2e.ts",
  "panes.e2e.ts",
  "skill-studio.e2e.ts",
  "skill-studio-drafts.e2e.ts",
  "your-repos.e2e.ts",
  "palette.e2e.ts",
  "project-switch.e2e.ts",
];

describe("the scenario specs a part runs", () => {
  it("runs every spec when the run is not split", () => {
    expect(specsOfPart(FILES, undefined)).toEqual([
      "activity.e2e.ts",
      "palette.e2e.ts",
      "pane-notices.e2e.ts",
      "panes.e2e.ts",
      "project-switch.e2e.ts",
      "projects.e2e.ts",
      "skill-studio-drafts.e2e.ts",
      "skill-studio.e2e.ts",
      "tab-chip.e2e.ts",
      "your-repos.e2e.ts",
    ]);
  });

  it("cuts four parts at the names chosen by measured time, each in name order", () => {
    expect(specsOfPart(FILES, "1/4")).toEqual(["activity.e2e.ts", "palette.e2e.ts"]);
    expect(specsOfPart(FILES, "2/4")).toEqual([
      "pane-notices.e2e.ts",
      "panes.e2e.ts",
      "project-switch.e2e.ts",
    ]);
    expect(specsOfPart(FILES, "3/4")).toEqual([
      "projects.e2e.ts",
      "skill-studio-drafts.e2e.ts",
      "skill-studio.e2e.ts",
    ]);
    expect(specsOfPart(FILES, "4/4")).toEqual(["tab-chip.e2e.ts", "your-repos.e2e.ts"]);
  });

  it("cuts two parts where the second starts at projects, as WebdriverIO's halves did", () => {
    expect(specsOfPart(FILES, "1/2")).toEqual([
      "activity.e2e.ts",
      "palette.e2e.ts",
      "pane-notices.e2e.ts",
      "panes.e2e.ts",
      "project-switch.e2e.ts",
    ]);
    expect(specsOfPart(FILES, "2/2")).toEqual([
      "projects.e2e.ts",
      "skill-studio-drafts.e2e.ts",
      "skill-studio.e2e.ts",
      "tab-chip.e2e.ts",
      "your-repos.e2e.ts",
    ]);
  });

  it("gives a new spec to exactly one part, whatever its name", () => {
    const withNew = [...FILES, "aaa.e2e.ts", "pane-zz.e2e.ts", "zzz.e2e.ts"];
    const parts = [1, 2, 3, 4].map((k) => specsOfPart(withNew, `${k}/4`));
    expect(parts.flat().sort()).toEqual([...withNew].sort());
    expect(parts[0]).toContain("aaa.e2e.ts");
    expect(parts[1]).toContain("pane-zz.e2e.ts");
    expect(parts[3]).toContain("zzz.e2e.ts");
  });

  it("refuses a split it has no cuts for, rather than run a wrong share", () => {
    expect(() => specsOfPart(FILES, "1/3")).toThrow(/no cuts for 3 parts/);
    expect(() => specsOfPart(FILES, "5/4")).toThrow(/part 5 of 4/);
    expect(() => specsOfPart(FILES, "half")).toThrow(/k\/n/);
  });
});
