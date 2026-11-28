// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Profiler, memo, useState } from "react";
import { act, render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { countReactsCommits, counts, drawnIn, renderedIn } from "./reactCommits.testkit";

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

/** Held, so it is drawn again only when its props move. */
const Part = memo(function Part({ n }: { n: number }) {
  return <span>{n}</span>;
});

/** Two of one component, each in a measured region of its own, and one outside both. */
function TwoRegions() {
  const [n, setN] = useState(0);
  return (
    <>
      <button type="button" onClick={() => setN(n + 1)}>
        more
      </button>
      <Profiler id="left" onRender={() => {}}>
        <Part n={n} />
      </Profiler>
      <Profiler id="right" onRender={() => {}}>
        <Profiler id="inner" onRender={() => {}}>
          <Part n={0} />
        </Profiler>
      </Profiler>
      <Part n={n} />
    </>
  );
}

describe("draws counted by the measured region they are in (#7894)", () => {
  it("counts a component in the nearest region around it, and only where it was drawn", () => {
    renderedIn.clear();
    const { getByRole, unmount } = render(<TwoRegions />);
    expect(drawnIn("left", "Part")).toBe(1);
    expect(drawnIn("inner", "Part")).toBe(1);
    expect(drawnIn("right", "Part")).toBe(0);

    renderedIn.clear();
    act(() => getByRole("button").click());
    // The left one's props moved; the inner one's did not, and it is not drawn again.
    expect(drawnIn("left", "Part")).toBe(1);
    expect(drawnIn("inner", "Part")).toBe(0);
    unmount();
  });
});
