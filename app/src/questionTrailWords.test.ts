// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { QuestionStep } from "./bindings";
import { trailSaid } from "./questionTrailWords";

/** A step as the core hands it, at `hh:mm` on one day. */
function step(at: string, over: Partial<QuestionStep> & Pick<QuestionStep, "step">): QuestionStep {
  return {
    at: `2026-11-02T${at}:00Z`,
    noted: false,
    why: null,
    by: null,
    what: null,
    ...over,
  };
}

const said = (steps: QuestionStep[]) =>
  trailSaid(steps, "developer 7", "steward 4").map((one) => `${one.time} ${one.said}`);

/** The prototype's trails (#4561, screenshots 44, 45 and 47), word for word. */
describe("a question's trail, in the window's words", () => {
  it("says an asking chat passed it on with a note, and then answered", () => {
    expect(
      said([
        step("14:27", { step: "asked" }),
        step("14:27", { step: "shown" }),
        step("14:28", { step: "passed_on", noted: true }),
        step("14:28", { step: "reached", why: "passed_on" }),
        step("14:30", { step: "answered", by: "asking_chat" }),
        step("14:30", { step: "closed", by: "asking_chat" }),
      ]),
    ).toEqual([
      "14:27 developer 7 asked",
      "14:27 Handed to steward 4",
      "14:28 steward 4 passed it on, with a note",
      "14:28 Reached you",
      "14:30 steward 4 answered",
    ]);
  });

  it("says the asking chat did not reply, and then you answered", () => {
    expect(
      said([
        step("14:28", { step: "asked" }),
        step("14:28", { step: "shown" }),
        step("14:29", { step: "reached", why: "no_reply" }),
        step("14:34", { step: "answered", by: "person" }),
        step("14:34", { step: "closed", by: "person" }),
      ]),
    ).toEqual([
      "14:28 developer 7 asked",
      "14:28 Handed to steward 4",
      "14:29 steward 4 did not reply in 100 seconds",
      "14:29 Reached you",
      "14:34 You answered",
    ]);
  });

  it("says the task ended unanswered, the asking chat closed, and purlis could not hand it on", () => {
    expect(
      said([
        step("14:28", { step: "asked" }),
        step("14:28", { step: "failed", what: "not_redirected" }),
        step("14:29", { step: "reached", why: "asker_gone" }),
        step("14:40", { step: "closed", by: "nobody" }),
      ]),
    ).toEqual([
      "14:28 developer 7 asked",
      "14:28 purlis could not hand it on",
      "14:29 steward 4 closed",
      "14:29 Reached you",
      "14:40 The task ended, unanswered",
    ]);
  });

  it("says the day where a step is on another day than the question was asked", () => {
    const steps = [
      step("23:59", { step: "asked" }),
      { ...step("00:01", { step: "answered", by: "person" }), at: "2026-11-03T00:01:00Z" },
    ];
    expect(said(steps)).toEqual(["23:59 developer 7 asked", "2026-11-03 00:01 UTC You answered"]);
  });
});
