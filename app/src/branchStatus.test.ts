import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, renderHook } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PlaneId } from "./bindings";
import { WATCH_WAIT_MS } from "./branchFolders";
import { useBranchStatus, type BranchRef } from "./branchStatus";

/**
 * **A branch's first status read waits on its watch, but not forever**, as a folder's first read
 * does (`branchFolders.test.ts`, #1427). A change is told (`branch-changed`) only once the branch
 * is watched, so a status read that finished before the watch held misses a file an agent wrote
 * in between, and nothing reads it again: the file is drawn without its mark until something
 * else moves in the branch. The e2e spec "marks a file an agent adds or changes without a
 * refresh" met exactly that on Linux, where watching a branch lists its folders through the
 * bounded reader and can take longer than the status read beside it.
 */

const PLANE = "/plane" as unknown as PlaneId;

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

const BRANCH: BranchRef = { repo: "svc", piece: "one" };

const STATUS = { base: null, changes: [], folders: [], more: 0 };

/** The core: `branch_watch` answered after `watchTakes` ms, or never when it is `undefined`;
 *  `branch_status` answered at once, each read's time recorded, as is the watch's answer. */
function core(watchTakes: number | undefined) {
  const reads: number[] = [];
  const watched: number[] = [];
  mockIPC((cmd) => {
    if (cmd === "branch_status") {
      reads.push(Date.now());
      return STATUS;
    }
    if (cmd === "branch_watch")
      return new Promise<null>((answer) => {
        if (watchTakes !== undefined)
          setTimeout(() => {
            watched.push(Date.now());
            answer(null);
          }, watchTakes);
      });
    return null;
  });
  return { reads, watched };
}

describe("a branch's first status read", () => {
  it("is read once its watch holds, so a change made before then is in it", async () => {
    const { reads, watched } = core(300);
    renderHook(() => useBranchStatus(PLANE, "alpha", [BRANCH]));

    await vi.advanceTimersByTimeAsync(WATCH_WAIT_MS * 3);

    expect(watched).toHaveLength(1);
    // Every read went out once the watch held: none of them can have missed a change the
    // watch did not tell.
    expect(reads.length).toBeGreaterThan(0);
    expect(reads.every((at) => at >= watched[0])).toBe(true);
    // And only one: a watch in time costs no second read.
    expect(reads).toHaveLength(1);
  });

  it("goes ahead after the bound when the watch is never answered", async () => {
    const { reads } = core(undefined);
    renderHook(() => useBranchStatus(PLANE, "alpha", [BRANCH]));

    await vi.advanceTimersByTimeAsync(WATCH_WAIT_MS - 100);
    expect(reads).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(200);
    expect(reads).toHaveLength(1);
  });

  it("is read once more when a watch slower than the bound answers", async () => {
    const { reads, watched } = core(WATCH_WAIT_MS + 1_000);
    renderHook(() => useBranchStatus(PLANE, "alpha", [BRANCH]));

    await vi.advanceTimersByTimeAsync(WATCH_WAIT_MS + 100);
    expect(reads).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(reads).toHaveLength(2);
    expect(reads[1]).toBeGreaterThanOrEqual(watched[0]);
  });
});
