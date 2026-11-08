// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { projectsLeft, sayWhatWasLeft } from "./projectsLeft";

/**
 * **What a spec file left of the projects it began with** (#5609): `make-it-a-project` once left
 * this machine's project open and in front, and ten files after it failed in it.
 */
describe("the projects a spec file left", () => {
  const LAUNCH = "/run/launch";
  const LOCAL = "/run/data/local-project";

  it("are none when it closed what it opened", () => {
    expect(projectsLeft([LAUNCH], [LAUNCH])).toEqual({ opened: [], closed: [] });
    expect(sayWhatWasLeft("make-it-a-project.e2e.ts", { opened: [], closed: [] })).toBe("");
  });

  it("are the project it opened and left open, to be closed and named", () => {
    const left = projectsLeft([LAUNCH], [LAUNCH, LOCAL]);
    expect(left).toEqual({ opened: [LOCAL], closed: [] });
    expect(sayWhatWasLeft("make-it-a-project.e2e.ts", left)).toContain(
      `make-it-a-project.e2e.ts left a project open: ${LOCAL}; the guard closed it.`,
    );
  });

  it("name the launch's project when the file closed it", () => {
    const left = projectsLeft([LAUNCH, LOCAL], [LOCAL]);
    expect(left).toEqual({ opened: [], closed: [LAUNCH] });
    expect(sayWhatWasLeft("onboarding.e2e.ts", left)).toContain(
      `onboarding.e2e.ts closed a project it found open: ${LAUNCH}.`,
    );
  });

  it("do not depend on the order the core lists them in", () => {
    expect(projectsLeft([LOCAL, LAUNCH], [LAUNCH, LOCAL])).toEqual({ opened: [], closed: [] });
  });
});
