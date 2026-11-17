// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { PolicyState } from "./bindings";
import { PolicyMark } from "./PolicyMark";
import { forgetPolicies, holdPolicy, policySentence } from "./projectPolicy";

/** The project's policy on a chat's pane head (Q308a, instruction 45). */

afterEach(() => {
  cleanup();
  forgetPolicies();
});

const PLANE = "/home/dev/plane";
const WATCH: PolicyState = {
  waiting: [],
  refused: [],
  unsealed: false,
  digest: "d1",
  template: "watch",
  template_default: true,
  overrides: [],
  locked: [],
};

describe("the policy's mark on a pane head", () => {
  it("draws nothing until the project's policy has been read", () => {
    const { container } = render(<PolicyMark plane={PLANE} />);
    expect(container.textContent).toBe("");
  });

  it("says the template, and counts the families set apart from it", () => {
    render(<PolicyMark plane={PLANE} />);
    act(() =>
      holdPolicy(PLANE, {
        ...WATCH,
        template: "team",
        template_default: false,
        overrides: [
          { family: "guards.release", said: "log", runs: "log" },
          { family: "network", said: "off", runs: "enforce" },
        ],
      }),
    );
    const mark = screen.getByRole("img", {
      name: "Policy: team; set apart: guards.release log, network off, enforced for now.",
    });
    expect(mark.textContent).toBe("team +2");
  });

  it("says another project's policy for that project alone", () => {
    render(<PolicyMark plane={PLANE} />);
    act(() => holdPolicy("/home/dev/other", { ...WATCH, template: "strict" }));
    expect(screen.queryByRole("img")).toBeNull();
  });
});

describe("the policy's sentence", () => {
  it("names the default, and a loosening that waits for Accept", () => {
    expect(policySentence(WATCH)).toBe("Policy: watch, the default.");
    expect(
      policySentence({
        ...WATCH,
        waiting: [{ family: "guards.merge", accepted: "enforce", committed: "log" }],
      }),
    ).toBe("Policy: watch, the default. A loosening waits for your Accept in the project's view.");
  });
});
