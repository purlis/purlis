// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { CheckResults, PlaneId } from "../bindings";
import { CheckResultsList } from "./CheckResults";

/**
 * **A Check reporter whose answer could not be recorded is a Notice naming it** (ADR 0116, #5410),
 * and the review goes on: the results recorded before it are listed beside it.
 */

const PLANE = "/plane" as unknown as PlaneId;
const CUT = { workspace: "alpha", repo: "svc", piece: "fix-it" };
const SAID =
  "The Check reporter 'mypy-sarif' answered something purlis cannot read as SARIF 2.1.0: it is " +
  "not JSON (expected value at line 1 column 1). Nothing it said was recorded, and the results " +
  "before stay. Run the check again once the extension is fixed";

const NONE: CheckResults = {
  sets: [],
  onLines: [],
  outside: [],
  elsewhere: [],
  moreOnLines: 0,
  moreOutside: 0,
  moreElsewhere: 0,
  trouble: null,
  faults: [],
};

const KEPT: CheckResults = {
  ...NONE,
  sets: [{ tool: "mypy", head: "32f2ddcb58188242", outOfDate: false, results: 1 }],
  outside: [
    {
      tool: "mypy",
      outOfDate: false,
      severity: "error",
      path: "a.py",
      line: 9,
      shownAt: null,
      rule: null,
      message: "kept from before",
    },
  ],
};

afterEach(cleanup);

describe("a Check reporter's fault", () => {
  it("is a Notice naming the extension, with the results before it still listed", () => {
    const { container } = render(
      <CheckResultsList
        plane={PLANE}
        cut={CUT}
        path="a.py"
        results={{ ...KEPT, faults: [{ extension: "mypy-sarif", said: SAID }] }}
      />,
    );
    const notice = container.querySelector('[data-cause="check-reporter-fault:mypy-sarif"]');
    expect(notice).not.toBeNull();
    expect(notice?.textContent).toContain("'mypy-sarif'");
    expect(screen.getByText("kept from before")).toBeTruthy();
  });

  it("is drawn for a branch with no results yet", () => {
    const { container } = render(
      <CheckResultsList
        plane={PLANE}
        cut={CUT}
        path="a.py"
        results={{ ...NONE, faults: [{ extension: "mypy-sarif", said: SAID }] }}
      />,
    );
    expect(
      container.querySelector('[data-cause="check-reporter-fault:mypy-sarif"]'),
    ).not.toBeNull();
  });

  it("is put away by Dismiss, and the results stay", async () => {
    const { container } = render(
      <CheckResultsList
        plane={PLANE}
        cut={CUT}
        path="a.py"
        results={{ ...KEPT, faults: [{ extension: "mypy-sarif", said: SAID }] }}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: /dismiss/i }));
    await waitFor(() =>
      expect(container.querySelector('[data-cause="check-reporter-fault:mypy-sarif"]')).toBeNull(),
    );
    expect(screen.getByText("kept from before")).toBeTruthy();
  });

  it("draws nothing for a branch with no results and no fault", () => {
    const { container } = render(
      <CheckResultsList plane={PLANE} cut={CUT} path="a.py" results={NONE} />,
    );
    expect(container.innerHTML).toBe("");
  });
});
