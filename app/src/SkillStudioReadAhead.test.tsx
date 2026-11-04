// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { PLANE, STUDIO_HISTORY, forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { SkillStudioTab } from "./SkillStudioTab";
import { skill, studio } from "./skillsListing.testkit";
import { AT_THE_ROOT } from "./skillStudio";
import { forgetStudioReadAhead, readStudioAhead } from "./studioReadAhead";

/**
 * **The studio draws the read asked as it was opened, and nothing older after that** (#4351): the
 * tab takes the read its opening asked for, and anything that asks it to read again once it is
 * open asks the core afresh.
 */

afterEach(() => {
  cleanup();
  // Before the mocks go: it stops listening through them.
  forgetStudioReadAhead();
  clearMocks();
  forgetStudio();
});

/** A core whose each read of the studio answers one skill named for the read it was. */
function core() {
  let reads = 0;
  mockIPC(
    (cmd) => {
      if (cmd === "skill_studio") {
        reads += 1;
        return studio([skill(`read-${reads}`)]);
      }
      if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
      return [];
    },
    { shouldMockEvents: true },
  );
  return { reads: () => reads };
}

describe("the studio's opening read (#4351)", () => {
  it("draws the read asked as it was opened, without asking again", async () => {
    const { reads } = core();
    readStudioAhead(PLANE, STUDIO_HISTORY, AT_THE_ROOT);
    renderStudio();

    await screen.findByTestId("skill-read-1");
    expect(reads()).toBe(1);
  });

  it("asks the core afresh when a skill changes after it opened", async () => {
    const { reads } = core();
    readStudioAhead(PLANE, STUDIO_HISTORY, AT_THE_ROOT);
    const { rerender } = renderStudio();
    await screen.findByTestId("skill-read-1");

    rerender(<SkillStudioTab plane={PLANE} changed={1} />);

    await screen.findByTestId("skill-read-2");
    expect(reads()).toBe(2);
  });

  it("reads for itself when the skills changed on disk before it drew the opening's read", async () => {
    const { reads } = core();
    readStudioAhead(PLANE, STUDIO_HISTORY, AT_THE_ROOT);
    await new Promise((done) => setTimeout(done, 0));
    await emit("plane-changed", { plane: PLANE, answers: null, changes: null });

    renderStudio();

    await screen.findByTestId("skill-read-2");
    expect(reads()).toBe(2);
  });
});
