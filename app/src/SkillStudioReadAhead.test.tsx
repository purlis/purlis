// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { PLANE, forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { SkillStudioTab } from "./SkillStudioTab";
import { skill, studio } from "./skillsListing.testkit";
import { readStudioAhead } from "./studioReadAhead";

/**
 * **The studio draws the read asked as it was opened, and nothing older after that** (#4351): the
 * tab takes the read its opening asked for, and anything that asks it to read again once it is
 * open asks the core afresh.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

/** A core whose each read of the studio answers one skill named for the read it was. */
function core() {
  let reads = 0;
  mockIPC((cmd) => {
    if (cmd === "skill_studio") {
      reads += 1;
      return studio([skill(`read-${reads}`)]);
    }
    if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
    return [];
  });
  return { reads: () => reads };
}

describe("the studio's opening read (#4351)", () => {
  it("draws the read asked as it was opened, without asking again", async () => {
    const { reads } = core();
    readStudioAhead(PLANE, null, null);
    renderStudio();

    await screen.findByTestId("skill-read-1");
    expect(reads()).toBe(1);
  });

  it("asks the core afresh when a skill changes after it opened", async () => {
    const { reads } = core();
    readStudioAhead(PLANE, null, null);
    const { rerender } = renderStudio();
    await screen.findByTestId("skill-read-1");

    rerender(<SkillStudioTab plane={PLANE} changed={1} />);

    await screen.findByTestId("skill-read-2");
    expect(reads()).toBe(2);
  });
});
