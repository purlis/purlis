// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The page navigation** (the `view-nav` family, #3666): a view's named pages, each with its
 * count, one Tab stop. The view frame draws it beside its list, and Settings beside its groups
 * (#2930). A module of its own, so a page that draws one (the theme specimen draws Settings'
 * pieces) loads nothing of the view frame's keys or the window's actions.
 */
import { useRef, useState, type KeyboardEvent } from "react";
import { mergeProps, useFocusRing, useHover, usePress } from "react-aria";
import { Count } from "../badge/Badge";
import { Twist } from "../disclosure/Disclosure";

/** **One page of a paged view**: its id, its name, and what its count says, if it has one. */
export interface ViewPage {
  id: string;
  label: string;
  /** How many it holds, after its name: a waiting page's in the waiting tone. */
  count?: number;
  /** The count is of what waits for the person (Needs you): the count family's `waiting`. */
  waiting?: boolean;
  /** What a screen reader hears in place of the number ("2 need you"). */
  countSaid?: string;
  /** A rule before it: it starts a group of its own (the studio's Harness skills, Routing). */
  startsGroup?: boolean;
  /** A page of the one above it, set in under it (a harness profile's, under Harness &
   *  profiles in Settings, ST-4). */
  sub?: boolean;
  /** **It heads a group of the pages under it** (Settings' nav, #2919, R42): whether the group
   *  is open, said by its toggle at the row's end. Unset for a page that heads none. */
  expanded?: boolean;
}

/** The keys that walk the navigation, and where each goes from `at` of `last + 1` pages. */
function walked(key: string, at: number, last: number): number | undefined {
  if (key === "ArrowDown") return Math.min(last, at + 1);
  if (key === "ArrowUp") return Math.max(0, at - 1);
  if (key === "Home") return 0;
  if (key === "End") return last;
  return undefined;
}

/**
 * **One page in the navigation**: a native button with the `view-nav` family's attributes and
 * states, its name, its count and the mark that says it is the current page. The current page is
 * `aria-current="page"` and drawn with the mark as well as its colour (WCAG 1.4.1).
 */
function ViewNavItem({
  page,
  current,
  stop,
  onChoose,
  onFocusHere,
  onKeyDown,
  refFor,
}: {
  page: ViewPage;
  current: boolean;
  /** The navigation's one Tab stop. */
  stop: boolean;
  onChoose: () => void;
  onFocusHere: () => void;
  onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
  refFor: (element: HTMLButtonElement | null) => void;
}) {
  // The pointer, a press and the keyboard's ring as React Aria marks them on every control.
  const { hoverProps, isHovered } = useHover({});
  const { pressProps, isPressed } = usePress({ onPress: onChoose });
  const { focusProps, isFocusVisible } = useFocusRing();
  const counted = page.count !== undefined && page.count > 0;
  return (
    <button
      type="button"
      {...mergeProps(hoverProps, pressProps, focusProps, { onKeyDown, onFocus: onFocusHere })}
      ref={refFor}
      tabIndex={stop ? 0 : -1}
      aria-current={current ? "page" : undefined}
      data-scope="view-nav"
      data-part="item"
      data-selected={current || undefined}
      data-hovered={isHovered || undefined}
      data-pressed={isPressed || undefined}
      data-focus-visible={isFocusVisible || undefined}
      data-starts-group={page.startsGroup || undefined}
      data-sub={page.sub || undefined}
      data-heads-group={page.expanded !== undefined || undefined}
      // → reaches its group's toggle (R42), said to a reader.
      aria-keyshortcuts={page.expanded !== undefined ? "ArrowRight" : undefined}
    >
      <span data-scope="view-nav" data-part="label">
        {page.label}
      </span>
      {/* A space between the name and its count, so a reader says two words; none without one,
          so the page is named by its name alone. */}
      {counted && " "}
      {counted && (
        <Count variant={page.waiting ? "waiting" : "neutral"} said={page.countSaid}>
          {page.count}
        </Count>
      )}
      <span data-scope="view-nav" data-part="indicator" aria-hidden="true" />
    </button>
  );
}

/**
 * **A group's toggle** (#2919, R42, R43): a 26 px button at the end of the row of the page that
 * heads the group, apart from the page's own button, so opening the group chooses no page.
 * "Expand ‹page›" or "Collapse ‹page›", with `aria-expanded`; its twist turns as it opens. It is
 * no Tab stop of its own: → reaches it from its page and ← goes back.
 */
function ViewNavToggle({
  page,
  onToggle,
  onKeyDown,
  refFor,
}: {
  page: ViewPage;
  onToggle: () => void;
  onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
  refFor: (element: HTMLButtonElement | null) => void;
}) {
  const { hoverProps, isHovered } = useHover({});
  const { pressProps, isPressed } = usePress({ onPress: onToggle });
  const { focusProps, isFocusVisible } = useFocusRing();
  return (
    <button
      type="button"
      {...mergeProps(hoverProps, pressProps, focusProps, { onKeyDown })}
      ref={refFor}
      tabIndex={-1}
      aria-expanded={page.expanded === true}
      aria-label={`${page.expanded ? "Collapse" : "Expand"} ${page.label}`}
      data-scope="view-nav"
      data-part="toggle"
      data-hovered={isHovered || undefined}
      data-pressed={isPressed || undefined}
      data-focus-visible={isFocusVisible || undefined}
    >
      <Twist open={page.expanded} />
    </button>
  );
}

/**
 * **The page navigation**: named pages, each with its count, one Tab stop. ↑ and ↓ move along
 * it and stop at its ends, Home and End go to the first and the last, and Enter, Space or a
 * press chooses the page. The Tab stop is the current page until the keyboard has been on
 * another one, which it stays on while the keyboard is in the navigation.
 */
export function ViewNav({
  label,
  pages,
  page,
  onPage,
  onExpand,
  rowWhenNarrow = false,
  edge = true,
}: {
  /** **One sideways-scrolling row where its container is narrow** (under 728 px; Settings'
   *  pages, R75), in place of a column. */
  rowWhenNarrow?: boolean;
  /** Whether it draws its own line after it: none where what holds it draws that edge
   *  (Settings' side column). */
  edge?: boolean;
  /** What the navigation is called (`aria-label`): "Skill studio pages". */
  label: string;
  pages: readonly ViewPage[];
  page: string;
  onPage: (id: string) => void;
  /** Opens or folds the group the page `id` heads, from its toggle (`expanded`). */
  onExpand?: (id: string, expanded: boolean) => void;
}) {
  const items = useRef(new Map<string, HTMLButtonElement>());
  const toggles = useRef(new Map<string, HTMLButtonElement>());
  const [aimed, setAimed] = useState<string | undefined>(undefined);
  const stop = pages.some((one) => one.id === aimed)
    ? aimed
    : pages.some((one) => one.id === page)
      ? page
      : pages[0]?.id;
  const keyed = (at: number) => (event: KeyboardEvent<HTMLButtonElement>) => {
    const here = pages[at];
    // → from a page that heads a group reaches its toggle (R42).
    if (event.key === "ArrowRight" && here !== undefined && toggles.current.has(here.id)) {
      event.preventDefault();
      toggles.current.get(here.id)?.focus();
      return;
    }
    const to = walked(event.key, at, pages.length - 1);
    if (to === undefined || event.altKey || event.ctrlKey || event.metaKey) return;
    event.preventDefault();
    const one = pages[to];
    if (one !== undefined) items.current.get(one.id)?.focus();
  };
  return (
    <nav
      aria-label={label}
      data-scope="view-frame"
      data-part="nav"
      data-row-when-narrow={rowWhenNarrow || undefined}
      data-edge={edge ? undefined : "none"}
      onBlur={(event) => {
        // The keyboard has left the navigation: its stop goes back to the current page.
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setAimed(undefined);
      }}
    >
      <ul>
        {pages.map((one, at) => (
          <li key={one.id}>
            <ViewNavItem
              page={one}
              current={one.id === page}
              stop={one.id === stop}
              onChoose={() => onPage(one.id)}
              onFocusHere={() => setAimed(one.id)}
              onKeyDown={keyed(at)}
              refFor={(element) => {
                if (element === null) items.current.delete(one.id);
                else items.current.set(one.id, element);
              }}
            />
            {one.expanded !== undefined && onExpand !== undefined && (
              <ViewNavToggle
                page={one}
                onToggle={() => onExpand(one.id, !one.expanded)}
                onKeyDown={(event) => {
                  if (event.key === "ArrowLeft") {
                    event.preventDefault();
                    items.current.get(one.id)?.focus();
                    return;
                  }
                  keyed(at)(event);
                }}
                refFor={(element) => {
                  if (element === null) toggles.current.delete(one.id);
                  else toggles.current.set(one.id, element);
                }}
              />
            )}
          </li>
        ))}
      </ul>
    </nav>
  );
}
