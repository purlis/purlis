// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import type { Shown } from "./bindings";
import { arrivedSaid, PULSED_KEY, useAsksArrival } from "./asksArrival";

/**
 * **A Blocking ask arriving** (ADR 0121 decisions 24 and 25, #6020): said once, politely, and
 * the first one on this machine that arrives while Asks is closed pulses the ✋, once.
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

beforeEach(() => localStorage.clear());
afterEach(() => localStorage.clear());

type Props = { asks: readonly Shown[] | undefined; elsewhere: number; shown: boolean };

function arrival(first: Props) {
  return renderHook((props: Props) => useAsksArrival(props.asks, props.elsewhere, props.shown), {
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
    const { result, rerender } = arrival({ asks: undefined, elsewhere: 0, shown: false });
    rerender({ asks: [ask(1)], elsewhere: 0, shown: false });
    expect(result.current.said.said).toBe("");
    expect(result.current.pulse).toBe(false);
  });

  it("says a new ask once, and nothing more when the same asks are drawn again", () => {
    const { result, rerender } = arrival({ asks: [], elsewhere: 0, shown: true });
    rerender({ asks: [ask(1)], elsewhere: 0, shown: true });
    const said = result.current.said;
    expect(said.said).toBe("New ask from steward 1: Run cargo test");
    // Drawn again with the same asks, in a fresh list: the same line, not said again.
    rerender({ asks: [ask(1)], elsewhere: 0, shown: true });
    rerender({ asks: [ask(1)], elsewhere: 0, shown: false });
    expect(result.current.said).toBe(said);
  });

  it("says an ask that rose elsewhere", () => {
    const { result, rerender } = arrival({ asks: [], elsewhere: 0, shown: true });
    rerender({ asks: [], elsewhere: 1, shown: true });
    expect(result.current.said.said).toBe("New ask in another project");
    rerender({ asks: [], elsewhere: 0, shown: true });
    expect(result.current.said.said).toBe("New ask in another project");
  });

  it("pulses the ✋ for the first ask that arrives while Asks is closed, and keeps that on this machine", () => {
    const { result, rerender, unmount } = arrival({ asks: [], elsewhere: 0, shown: false });
    rerender({ asks: [ask(1)], elsewhere: 0, shown: false });
    expect(result.current.pulse).toBe(true);
    expect(localStorage.getItem(PULSED_KEY)).not.toBeNull();
    act(() => result.current.pulsed());
    expect(result.current.pulse).toBe(false);
    // Another ask, and another window: once per machine.
    rerender({ asks: [ask(1), ask(2)], elsewhere: 0, shown: false });
    expect(result.current.pulse).toBe(false);
    unmount();
    const again = arrival({ asks: [], elsewhere: 0, shown: false });
    again.rerender({ asks: [ask(3)], elsewhere: 0, shown: false });
    expect(again.result.current.pulse).toBe(false);
  });

  it("does not pulse for an ask that arrives while Asks is open, and pulses for the next one closed", () => {
    const { result, rerender } = arrival({ asks: [], elsewhere: 0, shown: true });
    rerender({ asks: [ask(1)], elsewhere: 0, shown: true });
    expect(result.current.pulse).toBe(false);
    expect(localStorage.getItem(PULSED_KEY)).toBeNull();
    rerender({ asks: [ask(1)], elsewhere: 0, shown: false });
    expect(result.current.pulse).toBe(false);
    rerender({ asks: [ask(1), ask(2)], elsewhere: 0, shown: false });
    expect(result.current.pulse).toBe(true);
  });

  it("stops the pulse once Asks is opened", () => {
    const { result, rerender } = arrival({ asks: [], elsewhere: 0, shown: false });
    rerender({ asks: [ask(1)], elsewhere: 0, shown: false });
    expect(result.current.pulse).toBe(true);
    rerender({ asks: [ask(1)], elsewhere: 0, shown: true });
    expect(result.current.pulse).toBe(false);
  });
});
