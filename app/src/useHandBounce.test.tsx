// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useHandBounce } from "./useHandBounce";

/**
 * **The ✋'s bounce in a project's view** (#6343): the schedule fed by its timer, a fake clock,
 * and by what the view is told. The schedule itself is `handBounce.test.ts`'s.
 */

const quiet = vi.hoisted(() => ({ now: false }));
vi.mock("./quiet", () => ({ quietNow: () => ({ quiet: quiet.now, by_hand: false, ended: null }) }));

const MINUTE = 60_000;

type Props = Parameters<typeof useHandBounce>[0];
const NOTHING: Props = {
  waiting: false,
  arrived: 0,
  answered: undefined,
  opened: false,
  inFront: true,
};

function view(first: Partial<Props> = {}) {
  let props: Props = { ...NOTHING, ...first };
  const hook = renderHook((p: Props) => useHandBounce(p), { initialProps: props });
  return {
    ...hook,
    tell(change: Partial<Props>) {
      props = { ...props, ...change };
      hook.rerender(props);
    },
    /** The bounce plays out: its animation ends. */
    ends() {
      act(() => hook.result.current.bounced());
    },
    after(minutes: number) {
      act(() => void vi.advanceTimersByTime(minutes * MINUTE));
    },
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  quiet.now = false;
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useHandBounce", () => {
  it("bounces as an ask arrives, until its animation ends, and again 5 minutes on", () => {
    const one = view();
    expect(one.result.current.bouncing).toBe(false);
    one.tell({ waiting: true, arrived: 1 });
    expect(one.result.current.bouncing).toBe(true);
    one.ends();
    expect(one.result.current.bouncing).toBe(false);
    one.after(4);
    expect(one.result.current.bouncing).toBe(false);
    one.after(1);
    expect(one.result.current.bouncing).toBe(true);
  });

  it("does not bounce for what already waited as it was first drawn, and reminds of it", () => {
    const one = view({ waiting: true, arrived: 3 });
    expect(one.result.current.bouncing).toBe(false);
    one.after(5);
    expect(one.result.current.bouncing).toBe(true);
  });

  it("restarts the backoff as Asks opens, and as an answer is noted", () => {
    const one = view({ waiting: true });
    one.after(4);
    one.tell({ opened: true });
    one.after(4);
    expect(one.result.current.bouncing).toBe(false);
    one.tell({ answered: { at: 1 } });
    one.after(4);
    expect(one.result.current.bouncing).toBe(false);
    one.after(1);
    expect(one.result.current.bouncing).toBe(true);
  });

  it("stops once nothing waits", () => {
    const one = view({ waiting: true });
    one.tell({ waiting: false });
    one.after(200);
    expect(one.result.current.bouncing).toBe(false);
  });

  it("holds while Quiet, with the window behind, or the project behind, making none up", () => {
    const one = view({ waiting: true });
    quiet.now = true;
    one.after(5);
    expect(one.result.current.bouncing).toBe(false);
    quiet.now = false;
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    one.after(10);
    expect(one.result.current.bouncing).toBe(false);
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    one.tell({ inFront: false });
    one.after(20);
    expect(one.result.current.bouncing).toBe(false);
    one.tell({ inFront: true });
    // Due at 75 minutes: the three held are not played now.
    one.after(39);
    expect(one.result.current.bouncing).toBe(false);
    one.after(1);
    expect(one.result.current.bouncing).toBe(true);
  });
});
