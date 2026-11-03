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

/** Whether `fiber`, or anything above it, is the component `outside` is counted around. */
function within(fiber: Fiber, above: boolean): boolean {
  return above || (inside !== undefined && nameOf(fiber) === inside);
}

function drawn(fiber: Fiber, isInside: boolean): void {
  counts.rendered += 1;
  const name = nameOf(fiber);
  if (name) renderedBy.set(name, (renderedBy.get(name) ?? 0) + 1);
  // A memo is its child's wrapper, counted there; a component with no name is still named.
  if (inside === undefined || isInside || fiber.tag === 14) return;
  const named = name ?? "(anonymous)";
  outside.set(named, (outside.get(named) ?? 0) + 1);
}

export const counts: Counts = { rendered: 0, mounted: 0, unmounted: 0, commits: 0 };

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
    countUpdated(root.current, false);
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
