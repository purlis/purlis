// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import type { Said } from "./actions";
import { KEPT_RESULTS, keep, resultsFor, useKeptResults } from "./keptResults";

const said = (words: string) => ({ from: "x", refused: false, words });

describe("the results the window keeps (#3344)", () => {
  it("keeps each answer once, newest first", () => {
    const a = said("a");
    const b = said("b");
    let kept = keep([], a, "/p", 0);
    kept = keep(kept, a, "/p", 1);
    kept = keep(kept, b, "/p", 2);
    expect(kept.map((one) => one.said.words)).toEqual(["b", "a"]);
  });

  it("keeps at most the newest few", () => {
    let kept = keep([], said("0"), undefined, 0);
    for (let n = 1; n <= KEPT_RESULTS + 5; n += 1) kept = keep(kept, said(String(n)), undefined, n);
    expect(kept).toHaveLength(KEPT_RESULTS);
    expect(kept[0].said.words).toBe(String(KEPT_RESULTS + 5));
  });

  it("lists a project's own results and the window's in its Inbox", () => {
    const kept = [
      { id: 1, said: said("mine"), plane: "/a" },
      { id: 2, said: said("theirs"), plane: "/b" },
      { id: 3, said: said("window"), plane: undefined },
    ];
    expect(resultsFor(kept, "/a").map((one) => one.said.words)).toEqual(["mine", "window"]);
  });

  it("never keeps a line that only says the core has not answered yet", () => {
    const late = { from: "answer.late.createProject", refused: true, words: "Not yet." };
    expect(keep([], late, "/p", 0)).toEqual([]);
  });
});

describe("the kept results, as the window holds them (#3344's review)", () => {
  it("does not keep again a result put away while it is still the newest", () => {
    const said = { from: "x", refused: false, words: "Done." };
    const { result, rerender } = renderHook(({ now }) => useKeptResults(now, "/p"), {
      initialProps: { now: said as Said | undefined },
    });
    expect(result.current.kept).toHaveLength(1);
    act(() => result.current.drop(result.current.kept[0].id));
    rerender({ now: said });
    expect(result.current.kept).toEqual([]);
  });

  it("keeps no progress line", () => {
    const cloning = { from: "workspace.create", refused: false, words: "Cloning…", progress: true };
    expect(keep([], cloning, "/p", 0)).toEqual([]);
  });
});
