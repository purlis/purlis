// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { renderHook } from "@testing-library/react";
import type { Shown } from "./bindings";
import { arrivedSaid, useAsksArrival } from "./asksArrival";

/**
 * **A Blocking ask arriving** (ADR 0121 decision 25, #6020): said once, politely, and counted
 * when it arrives in this project, for the ✋'s bounce (#6343).
 */

const ask = (session: number, says = "Run cargo test"): Shown => ({
  session,
  ask: `perm:${session}:${says}`,
  says,
  options: [],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

type Props = { asks: readonly Shown[] | undefined; elsewhere: number };

function arrival(first: Props) {
  return renderHook((props: Props) => useAsksArrival(props.asks, props.elsewhere), {
    initialProps: first,
  });
}

describe("what a new ask says (ADR 0121 decision 25)", () => {
  it("names the chat and what it asks, or how many arrived together", () => {
    expect(arrivedSaid([ask(3)], 0)).toBe("New ask from steward 3: Run cargo test");
    expect(arrivedSaid([ask(3), ask(4)], 0)).toBe("2 new asks");
    expect(arrivedSaid([], 1)).toBe("New ask in another project");
    expect(arrivedSaid([ask(3)], 2)).toBe("3 new asks");
  });
});

describe("useAsksArrival", () => {
  it("says nothing of what already waited when the asks were first read", () => {
    const { result, rerender } = arrival({ asks: undefined, elsewhere: 0 });
    rerender({ asks: [ask(1)], elsewhere: 0 });
    expect(result.current.said.said).toBe("");
    expect(result.current.here).toBe(0);
  });

  it("says a new ask once, and nothing more when the same asks are drawn again", () => {
    const { result, rerender } = arrival({ asks: [], elsewhere: 0 });
    rerender({ asks: [ask(1)], elsewhere: 0 });
    const said = result.current.said;
    expect(said.said).toBe("New ask from steward 1: Run cargo test");
    // Drawn again with the same asks, in a fresh list: the same line, not said again.
    rerender({ asks: [ask(1)], elsewhere: 0 });
    rerender({ asks: [ask(1)], elsewhere: 0 });
    expect(result.current.said).toBe(said);
  });

  it("says an ask that rose elsewhere", () => {
    const { result, rerender } = arrival({ asks: [], elsewhere: 0 });
    rerender({ asks: [], elsewhere: 1 });
    expect(result.current.said.said).toBe("New ask in another project");
    rerender({ asks: [], elsewhere: 0 });
    expect(result.current.said.said).toBe("New ask in another project");
  });

  it("counts each read that brought an ask to this project, and not one that rose elsewhere", () => {
    const { result, rerender } = arrival({ asks: [], elsewhere: 0 });
    expect(result.current.here).toBe(0);
    rerender({ asks: [ask(1), ask(2)], elsewhere: 0 });
    expect(result.current.here).toBe(1);
    rerender({ asks: [ask(1), ask(2)], elsewhere: 0 });
    rerender({ asks: [ask(2)], elsewhere: 0 });
    expect(result.current.here).toBe(1);
    rerender({ asks: [ask(2)], elsewhere: 3 });
    expect(result.current.here).toBe(1);
    rerender({ asks: [ask(2), ask(4)], elsewhere: 3 });
    expect(result.current.here).toBe(2);
  });
});
