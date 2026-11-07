// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { act, render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { countReactsCommits, counts } from "./reactCommits.testkit";

countReactsCommits();

function Counter() {
  const [n, setN] = useState(0);
  return (
    <button type="button" onClick={() => setN(n + 1)}>
      {n}
    </button>
  );
}

describe("the commit counter the cost tests hold their budgets with (#5043)", () => {
  it("counts a render that certainly commits, so a budget can fail on any machine", () => {
    // Installed after `react-dom` loads, React never calls the hook and every count reads 0:
    // a cost test then holds every budget it is given, and only a run that loaded it first can
    // fail. `test-setup.ts` imports it before anything else.
    Object.assign(counts, { rendered: 0, mounted: 0, unmounted: 0, commits: 0 });
    const { getByRole, unmount } = render(<Counter />);
    expect(counts.commits).toBe(1);
    expect(counts.mounted).toBe(1);

    act(() => getByRole("button").click());
    expect(counts.commits).toBe(2);

    unmount();
    expect(counts.unmounted).toBe(1);
  });
});
