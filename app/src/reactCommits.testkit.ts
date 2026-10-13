// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What React did in its commits, counted the way the React profiler counts it**: through the
 * hook React looks for the DevTools on. It imports nothing, React least of all, because the hook
 * has to be in place before `react-dom` is first loaded (`countReactsCommits`).
 */

/** What React's commits said, since the last `reset`. */
export type Counts = { rendered: number; mounted: number; unmounted: number; commits: number };

type Fiber = {
  tag: number;
  flags: number;
  alternate: Fiber | null;
  child: Fiber | null;
  sibling: Fiber | null;
};

/** Function, class, forwardRef, memo and simple-memo components: what "a component" means. */
const COMPONENTS = new Set([0, 1, 11, 14, 15]);
/** React's `PerformedWork` flag: the component's own function ran for this commit. */
const PERFORMED_WORK = 1;

export const counts: Counts = { rendered: 0, mounted: 0, unmounted: 0, commits: 0 };

function countMounted(fiber: Fiber | null): void {
  for (let one = fiber; one; one = one.sibling) {
    if (COMPONENTS.has(one.tag)) {
      counts.mounted += 1;
      counts.rendered += 1;
    }
    countMounted(one.child);
  }
}

/** One fiber that was there before: drawn again or not, then its children the same way. A
 *  fiber whose child is its alternate's child bailed out, and nothing below it ran. */
function countUpdated(fiber: Fiber): void {
  const before = fiber.alternate;
  if (before === null) {
    if (COMPONENTS.has(fiber.tag)) {
      counts.mounted += 1;
      counts.rendered += 1;
    }
    countMounted(fiber.child);
    return;
  }
  if (COMPONENTS.has(fiber.tag) && (fiber.flags & PERFORMED_WORK) !== 0) counts.rendered += 1;
  if (fiber.child === before.child) return;
  for (let child = fiber.child; child; child = child.sibling) countUpdated(child);
}

/**
 * Puts the counting hook where React looks for the DevTools, which it does once, as
 * `react-dom` is first loaded: so this is called from `vi.hoisted`, above every import.
 */
export function countReactsCommits(): void {
  (globalThis as Record<string, unknown>).__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
    isDisabled: false,
    supportsFiber: true,
    renderers: new Map(),
    inject: () => 1,
    checkDCE: () => {},
    onScheduleFiberRoot: () => {},
    onCommitFiberRoot: (_id: number, root: { current: Fiber }) => {
      counts.commits += 1;
      countUpdated(root.current);
    },
    onPostCommitFiberRoot: () => {},
    onCommitFiberUnmount: (_id: number, fiber: Fiber) => {
      if (COMPONENTS.has(fiber.tag)) counts.unmounted += 1;
    },
  };
}
