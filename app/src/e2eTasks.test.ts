import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import type { OpenChat } from "./bindings";
import { PRETEND_LIMITS, usePretendLimits, withPretendedLimits } from "./e2eTasks";

/**
 * The scenario specs' pretend seam for a session's own limit numbers (#1545): heard only in
 * the `e2e` build, and laid over the chat the core listed.
 */

const SESSION: OpenChat = {
  session: 4,
  name: "4",
  cwd: null,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: null,
  persona: null,
  unreported: null,
  card: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
  tasks_limit: null,
  tasks_running: null,
  at_limit: null,
};

const PAST_TOKENS = {
  limit: 1_000_000,
  row: "past its token limit (1.2M of 1M) · not enforced yet",
  said: "This session is past its token limit.",
  own: false,
};

function say(detail: unknown) {
  act(() => {
    window.dispatchEvent(new CustomEvent(PRETEND_LIMITS, { detail }));
  });
}

afterEach(() => {
  cleanup();
  vi.unstubAllEnvs();
});

describe("the limit numbers a spec pretends a session has", () => {
  it("lays them over the session they are said for, and leaves every other chat as listed", () => {
    const limits = new Map([[4, { tasks_limit: 6, tasks_running: 2, at_limit: PAST_TOKENS }]]);

    expect(withPretendedLimits(SESSION, limits)).toEqual({
      ...SESSION,
      tasks_limit: 6,
      tasks_running: 2,
      at_limit: PAST_TOKENS,
    });
    const other = { ...SESSION, session: 5 };
    expect(withPretendedLimits(other, limits)).toBe(other);
  });

  it("hears them in the e2e build, by session, and an empty list takes them away", () => {
    vi.stubEnv("VITE_E2E", "1");
    const { result } = renderHook(() => usePretendLimits());

    say([{ session: 4, tasks_limit: 6, tasks_running: 2, at_limit: PAST_TOKENS }]);
    expect(result.current.get(4)).toEqual({
      tasks_limit: 6,
      tasks_running: 2,
      at_limit: PAST_TOKENS,
    });

    say([]);
    expect(result.current.size).toBe(0);
  });

  it("hears nothing in any other build", () => {
    vi.stubEnv("VITE_E2E", "");
    const { result } = renderHook(() => usePretendLimits());

    say([{ session: 4, tasks_limit: 6, tasks_running: 2, at_limit: PAST_TOKENS }]);
    expect(result.current.size).toBe(0);
  });
});
