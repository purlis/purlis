// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What React did in its commits, counted the way the React profiler counts it**: through the
 * hook React looks for the DevTools on. It imports nothing, React least of all, because the hook
 * has to be in place before `react-dom` is first loaded: `test-setup.ts` loads this file first,
 * which puts the hook up, and it counts nothing until a test calls `countReactsCommits`.
 */

/** What React's commits said, since the last `reset`. */
export type Counts = { rendered: number; mounted: number; unmounted: number; commits: number };

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

export const counts: Counts = { rendered: 0, mounted: 0, unmounted: 0, commits: 0 };

/**
 * **Which components were drawn outside one component, by name**, for a test that holds an
 * update to a part of the window (`drawnOutside`). Empty, and nothing is named, until a test
 * says which component the update belongs to.
 */
export const outside = new Map<string, number>();
/** The component `outside` is counted around, or none. */
let inside: string | undefined;

/**
 * Names, from now on, every component React draws that is not `name` nor drawn inside it, and
 * forgets what was named before. Each one drawn again counts once a commit.
 */
export function drawnOutside(name: string): Map<string, number> {
  inside = name;
  outside.clear();
  return outside;
}

/** A component's name, as the React profiler shows it: through `memo` and `forwardRef`. */
function nameOf(type: unknown): string {
  const one = type as
    { displayName?: string; name?: string; type?: unknown; render?: unknown } | undefined;
  if (one === undefined || one === null) return "?";
  if (one.displayName) return one.displayName;
  if (one.type !== undefined) return nameOf(one.type);
  if (one.render !== undefined) return nameOf(one.render);
  return one.name || "?";
}

/** Whether `fiber`, or anything above it, is the component `outside` is counted around. */
function within(fiber: Fiber, above: boolean): boolean {
  return (
    above || (inside !== undefined && COMPONENTS.has(fiber.tag) && nameOf(fiber.type) === inside)
  );
}

/** Counts one component drawn, and names it when it was drawn outside. */
function drawn(fiber: Fiber, isInside: boolean): void {
  counts.rendered += 1;
  if (inside === undefined || isInside) return;
  const name = nameOf(fiber.type);
  outside.set(name, (outside.get(name) ?? 0) + 1);
}

function countMounted(fiber: Fiber | null, above: boolean): void {
  for (let one = fiber; one; one = one.sibling) {
    const isInside = within(one, above);
    if (COMPONENTS.has(one.tag)) {
      counts.mounted += 1;
      drawn(one, isInside);
    }
    countMounted(one.child, isInside);
  }
}

/** One fiber that was there before: drawn again or not, then its children the same way. A
 *  fiber whose child is its alternate's child bailed out, and nothing below it ran. */
function countUpdated(fiber: Fiber, above: boolean): void {
  const before = fiber.alternate;
  const isInside = within(fiber, above);
  if (before === null) {
    if (COMPONENTS.has(fiber.tag)) {
      counts.mounted += 1;
      drawn(fiber, isInside);
    }
    countMounted(fiber.child, isInside);
    return;
  }
  if (COMPONENTS.has(fiber.tag) && (fiber.flags & PERFORMED_WORK) !== 0) drawn(fiber, isInside);
  if (fiber.child === before.child) return;
  for (let child = fiber.child; child; child = child.sibling) countUpdated(child, isInside);
}

/** Whether the hook counts, which it does once a test has asked it to. */
let counting = false;

/**
 * Counts React's commits from now on, in this test file.
 *
 * **The hook is put up when this file is loaded, not here** (below). React looks for it once,
 * as `react-dom` is first loaded, and `test-setup.ts` loads Testing Library, and `react-dom`
 * with it, before any test file's own code runs, `vi.hoisted` included. A hook put up from a
 * test was never seen, and every count read zero.
 */
export function countReactsCommits(): void {
  counting = true;
}

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
    countUpdated(root.current, false);
  },
  onPostCommitFiberRoot: () => {},
  onCommitFiberUnmount: (_id: number, fiber: Fiber) => {
    if (counting && COMPONENTS.has(fiber.tag)) counts.unmounted += 1;
  },
};
