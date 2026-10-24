// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vite/client" />

/**
 * **Which of the two ways to end a task a breadcrumb's line draws** (#1488): each whole or
 * not at all, and only after everything that gives way before them has.
 *
 * The order a short line gives up its things in is: the harness's name first, cut to its
 * padding (the stylesheet: it shrinks a million times faster than anything else); then the
 * breadcrumb's names, each to an ellipsis; then the ways, the last first; then what padding the
 * line's other items kept, the last first; and the state word last of all. The first two are the stylesheet's, and this is the third. It is not left to
 * shrinking and wrapping: a line is made up by everything on it at once, so the group gave
 * a sliver while the harness still had room to give, and text is a fraction of a pixel
 * wider than any constant kept beside it. Measured in CI: a group 0.03px short of "Close
 * now", which a wrapping line then never drew. So the line is measured, as the tab strip is
 * (`fits.ts`), and a way that does not fit whole is not drawn.
 *
 * A way that is not drawn is set aside (`data-away`): out of the line's flow, below the
 * group's one line where its own clip hides it, and out of the tab sequence.
 */
export function fitWays(group: HTMLElement): void {
  const line = group.parentElement;
  const ways = [...group.querySelectorAll<HTMLElement>(":scope > .task-end")];
  if (line === null) return;
  const crumbs = line.querySelector<HTMLElement>(":scope > .pane-crumbs");
  const state = crumbs?.querySelector<HTMLElement>(".shown-state") ?? null;
  // The line's other items (the gauge, the harness's name, Brief), already cut to their padding
  // by the stylesheet before a way is given up.
  const others = ([...line.children] as HTMLElement[]).filter(
    (one) => one !== crumbs && one !== group,
  );
  // Something that must not be given up for a way is: the state word is cut, or pushed out
  // of its breadcrumb, or a way runs past the line's end.
  const short = () => {
    if (state !== null && crumbs !== null) {
      const word = state.querySelector<HTMLElement>(".word") ?? state;
      if (word.scrollWidth > word.clientWidth + 1) return true;
      if (state.getBoundingClientRect().right > crumbs.getBoundingClientRect().right + 0.5)
        return true;
    }
    return runsPast(ways, line);
  };
  for (const one of others) delete one.dataset.away;
  setAsideUntilWhole(ways, short);
  // **Then their padding too, the last first** (#3144): in the narrowest pane, with every way
  // set aside, what is left of those items is what the state word still lacks. Set aside, an
  // item keeps its place on the line at no width, drawn as nothing and out of the tab sequence
  // (`App.css`), and comes back with the room.
  if (short()) setAsideUntilWhole(others, short);
}

/**
 * **Which counts a tab's chip draws** (#1487): every one, until its tab's name has given up
 * all it can, and then whole and from the end (finished, then failed, then waiting), so the
 * count of what is still working is the last one standing. The name gives way first by the
 * stylesheet (it shrinks to its floor, and the chip does not shrink); a count goes only where
 * the cell still runs over after that. Measured for the reason {@link fitWays} gives: a chip
 * that shrank with the name gave a sliver while the name still had room, and a count a
 * fraction of a pixel short was not drawn (CI: "done" missing with room to spare).
 *
 * **And where not even that one count fits, the whole chip goes, never the tab's title**
 * (#1867): a narrow centre column, at 900 px with both sidebars open, draws tabs at their floor,
 * and a chip that stayed ran the tab's close out of its cell. A tab is a name to aim at first;
 * its tasks are a glance, and they come back with the room.
 */
export function fitCounts(counts: HTMLElement): void {
  const cell = counts.closest<HTMLElement>(".tab");
  const chip = counts.closest<HTMLElement>(".tab-tasks");
  const each = [...counts.querySelectorAll<HTMLElement>(":scope > .count")];
  if (cell === null) return;
  if (chip !== null) delete chip.dataset.away;
  const name = cell.querySelector<HTMLElement>(':scope > [role="tab"] .tab-name');
  // Something the chip must not take: a part of the cell runs past its end, or the tab's title
  // is not drawn at all, where the button's floor holds its mark and no letter of it.
  const short = () => runsPast([...cell.children] as HTMLElement[], cell) || undrawn(name);
  // The first, what is still working, is never set aside on its own: a cell too narrow even
  // for it is the stylesheet's (the hand alone, under 7.5rem), and then the chip's as a whole.
  setAsideUntilWhole(each, short, 1);
  if (chip !== null && short()) chip.dataset.away = "";
}

/** Whether the tab's title `name` has a name to draw and is drawn not even a pixel wide
 *  (#1867): on a narrow strip the button's own floor can hold its mark and none of its name.
 *  Measured by its box, as the strip is laid out; a button not laid out says nothing. */
function undrawn(name: HTMLElement | null): boolean {
  if (name === null || !name.textContent) return false;
  const tab = name.closest<HTMLElement>('[role="tab"]');
  if (tab === null || tab.getBoundingClientRect().width === 0) return false;
  return name.getBoundingClientRect().width < 1;
}

/** Fits `counts` now, and again whenever its tab's cell changes size or what it holds. */
export function fitCountsOn(counts: HTMLElement): () => void {
  const cell = counts.closest<HTMLElement>(".tab");
  return refitOn(counts, cell, cell, fitCounts);
}

/**
 * **Whether a tab draws its count pill** (qw337, by D-1867-1's rule): whole, until its tab's
 * title would be cut to less than reads as a title, and then not at all. The title gives way
 * first by the stylesheet (it shrinks, and the pill does not); the pill goes only where the
 * title is already down to {@link TITLE_KEPT} of its own text, or where the pill would run past
 * the end of a tab that has no title to give way. A tab is a name to aim at first; its counts
 * are a glance, the title bar's hand still says what needs you, and the pill comes back with the
 * room.
 *
 * Nothing is asked of a pill outside a tab (a row of a show-more menu), or of a tab that is
 * not laid out.
 */
export function fitPill(pill: HTMLElement): void {
  delete pill.dataset.away;
  const tab = pill.closest<HTMLElement>('[role="tab"]');
  if (tab === null || tab.getBoundingClientRect().width === 0) return;
  const name = tab.querySelector<HTMLElement>(".workspace-name, .project-name");
  if (cutShort(name) || runsPast([pill], tab)) pill.dataset.away = "";
}

/** How much of a title a pill may cost it, in the title's own text size: three or four letters
 *  and the ellipsis. A title shorter than that is kept whole. */
const TITLE_KEPT = 2.5;

/** Whether a tab's title is cut to less than {@link TITLE_KEPT} of its text, or to nothing. */
function cutShort(name: HTMLElement | null): boolean {
  if (name === null || !name.textContent) return false;
  const drawn = name.getBoundingClientRect().width;
  const text = Number.parseFloat(getComputedStyle(name).fontSize);
  const kept = Number.isFinite(text) ? Math.min(name.scrollWidth, TITLE_KEPT * text) : 0;
  return drawn < Math.max(1, kept - 0.5);
}

/** Fits `pill` now, and again whenever its tab changes size or what it holds. */
export function fitPillOn(pill: HTMLElement): () => void {
  const tab = pill.closest<HTMLElement>('[role="tab"]');
  return refitOn(pill, tab, tab, fitPill);
}

/** Draws every one of `items`, then sets them aside from the last while `short()` says so,
 *  keeping the first `keep` whatever it says. */
function setAsideUntilWhole(items: readonly HTMLElement[], short: () => boolean, keep = 0): void {
  for (const item of items) delete item.dataset.away;
  for (let at = items.length - 1; at >= keep && short(); at--) items[at].dataset.away = "";
}

/** Whether any of `items` still drawn runs past the end of `box`. */
function runsPast(items: readonly HTMLElement[], box: HTMLElement): boolean {
  const end = box.getBoundingClientRect().right;
  return items.some(
    (item) => item.dataset.away === undefined && item.getBoundingClientRect().right > end + 0.5,
  );
}

/**
 * Fits `group` now, and again whenever its pane changes size or its line what it holds.
 * Answers how to stop.
 */
export function fitWaysOn(group: HTMLElement): () => void {
  const line = group.parentElement;
  return refitOn(group, group.closest<HTMLElement>(".pane-frame") ?? line, line, fitWays);
}

/**
 * Runs `fit` on `what` now, and once a frame later whenever `sized` changes size or `held`
 * what it holds. Not on an attribute: setting a thing aside is one, and must not set off
 * another fit.
 *
 * **Not while `what` is in no document**, as everything of a project kept off the window is
 * (`projectShown.tsx`): nothing there has a size to fit to, so what was set aside stays as it
 * is, and it is fitted again as it comes back, which is a change of size.
 */
function refitOn(
  what: HTMLElement,
  sized: HTMLElement | null,
  held: HTMLElement | null,
  fit: (what: HTMLElement) => void,
): () => void {
  fit(what);
  if (sized === null || held === null) return () => undefined;
  let queued = 0;
  const again = () => {
    if (queued !== 0) return;
    queued = requestAnimationFrame(() => {
      queued = 0;
      if (what.isConnected) fit(what);
    });
  };
  const resized = new ResizeObserver(again);
  resized.observe(sized);
  const changed = new MutationObserver(again);
  changed.observe(held, { childList: true, subtree: true, characterData: true });
  return () => {
    resized.disconnect();
    changed.disconnect();
    if (queued !== 0) cancelAnimationFrame(queued);
  };
}

// **The scenario specs draw a breadcrumb's line by hand** (`pane-crumbs.e2e.ts`): they fit
// the two ways they draw with this same function, so what they measure is what a pane draws.
// Only in the `e2e` build (`e2eTasks.ts` says how that is set).
if (import.meta.env.VITE_E2E)
  (window as unknown as { purlisE2eFitWays?: (group: HTMLElement) => void }).purlisE2eFitWays =
    fitWays;
