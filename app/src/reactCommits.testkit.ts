// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What React did in its commits, counted the way the React profiler counts it**: through the
 * hook React looks for the DevTools on. It imports nothing, React least of all, because the hook
 * has to be in place before `react-dom` is first loaded: `test-setup.ts` imports it first.
 */

/** What React's commits said, since the last `reset`. */
export type Counts = { rendered: number; mounted: number; unmounted: number; commits: number };

/** How many times each component, by its name, was drawn (mounted or drawn again), since it was
 *  last cleared: for a budget about one component, such as a chip that must not redraw. */
export const renderedBy = new Map<string, number>();

type Fiber = {
  tag: number;
  type: unknown;
  flags: number;
  alternate: Fiber | null;
  child: Fiber | null;
  sibling: Fiber | null;
};

/** Function, class, forwardRef, memo and simple-memo components: what "a component" means. */
const COMPONENTS = new Set([0, 1, 11, 14, 15]);
/** React's `PerformedWork` flag: the component's own function ran for this commit. */
const PERFORMED_WORK = 1;

/** A component fiber's name. A `memo` (14) is left out: its own function is the child fiber
 *  below it, which is counted, so each component is counted once. */
function nameOf(fiber: Fiber): string | undefined {
  if (fiber.tag === 14) return undefined;
  const type = fiber.type as { displayName?: string; name?: string; render?: { name?: string } };
  return type?.displayName ?? type?.name ?? type?.render?.name;
}

function drawn(fiber: Fiber): void {
  counts.rendered += 1;
  const name = nameOf(fiber);
  if (name) renderedBy.set(name, (renderedBy.get(name) ?? 0) + 1);
}

export const counts: Counts = { rendered: 0, mounted: 0, unmounted: 0, commits: 0 };

function countMounted(fiber: Fiber | null): void {
  for (let one = fiber; one; one = one.sibling) {
    if (COMPONENTS.has(one.tag)) {
      counts.mounted += 1;
      drawn(one);
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
      drawn(fiber);
    }
    countMounted(fiber.child);
    return;
  }
  if (COMPONENTS.has(fiber.tag) && (fiber.flags & PERFORMED_WORK) !== 0) drawn(fiber);
  if (fiber.child === before.child) return;
  for (let child = fiber.child; child; child = child.sibling) countUpdated(child);
}

/** Whether a test file asked for React's commits to be counted (`countReactsCommits`). */
let counting = false;

/**
 * Puts the counting hook where React looks for the DevTools, which it does once, as
 * `react-dom` is first loaded. `test-setup.ts` imports this module before anything else, since
 * it loads `react-dom` itself (through Testing Library): a hook put in place by a test file,
 * even from `vi.hoisted`, came after it and was never called, and every count read 0.
 */
(globalThis as Record<string, unknown>).__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
  isDisabled: false,
  supportsFiber: true,
  renderers: new Map(),
  inject: () => 1,
  checkDCE: () => {},
  onScheduleFiberRoot: () => {},
  onCommitFiberRoot: (_id: number, root: { current: Fiber }) => {
    if (!counting) return;
    counts.commits += 1;
    countUpdated(root.current);
  },
  onPostCommitFiberRoot: () => {},
  onCommitFiberUnmount: (_id: number, fiber: Fiber) => {
    if (counting && COMPONENTS.has(fiber.tag)) counts.unmounted += 1;
  },
};

/** Counts React's commits from here on, in this test file. Nothing is counted until it is
 *  called, so a test that does not ask pays one check a commit. */
export function countReactsCommits(): void {
  counting = true;
}
