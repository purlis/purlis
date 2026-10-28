// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The view frame** (#3666, spec #2897, the Skill studio frame's variant A): a member of the
 * panel family that lays a view tab out as pages. A screen draws a paged view from these and
 * never lays its panes out itself (`purlis/panel-family`).
 *
 * - **Three panes in one view tab.** The page navigation ({@link ViewNav}), the page's list
 *   (`list`: its heading and lead line, {@link ViewListHead}, then a list or table family
 *   member), and the item chosen in it beside it (`item`: its page header,
 *   {@link ViewPageHeader}, then its banner and its body). A page with no item beside its list
 *   hands no `item`, and its list takes the room.
 * - **Each pane scrolls on its own**, and the page header stays in view over the item's body.
 *   The panes keep their room when the item changes or a count does: their sizes are the frame's,
 *   never their content's, so nothing moves (#2800).
 * - **Resizable** between the list and the item, through the panel family's `Separator`.
 * - **Narrow** (a narrow tab, or a split pane, under {@link NARROW_BELOW}): page-and-back. The
 *   navigation is a `Select`, the list is drawn first, and an item opened from it (`opened`) is
 *   drawn alone with a way back to the list (`onBack`). Nothing scrolls sideways.
 * - **Drawn by its parts**: `data-scope="view-frame"` with `data-part` `root`, `nav`, `list`,
 *   `item`, `header` and `lead`; a page in the navigation is the `view-nav` family's `item`, its
 *   mark that it is the current page its `indicator`. `panel.css` draws the defaults and a
 *   theme's parts reach the same attributes (`purlis theme parts`).
 *
 * - **Its own back and forward** (#3667): handed the tab's `history`, ⌘[ and ⌘] (Alt+← and Alt+→
 *   off a Mac) and the mouse's back and forward buttons walk it anywhere in the view, and its
 *   `bar` (`ViewBar.tsx`) draws the two buttons and the path over the panes.
 *
 * Which pages there are is the owner's: a page with nothing in it (Needs you, Drafts) is left
 * out of `pages`, never drawn empty.
 */
import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { mergeProps, useFocusRing, useHover, usePress } from "react-aria";
import { ChevronLeft } from "lucide-react";
import { Group, Panel } from "react-resizable-panels";
import { Count } from "../badge/Badge";
import { Button, ButtonRow } from "../button/Button";
import { Select } from "../select/Select";
import { aModalIsUp, inADialog, inAMenu } from "../../keys/contexts";
import { pressed } from "../../keys/registry";
import { Separator } from "./Separator";
import type { HistorySteps } from "./viewHistory";

/** **Below this width the frame is page-and-back**, in px: the navigation's column, the least a
 *  list keeps and the least an item keeps, side by side (the prototype's 170, 260 and 290). */
export const NARROW_BELOW = 720;

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
    >
      <span data-scope="view-nav" data-part="label">
        {page.label}
      </span>{" "}
      {page.count !== undefined && page.count > 0 && (
        <Count variant={page.waiting ? "waiting" : "neutral"} said={page.countSaid}>
          {page.count}
        </Count>
      )}
      <span data-scope="view-nav" data-part="indicator" aria-hidden="true" />
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
}: {
  /** What the navigation is called (`aria-label`): "Skill studio pages". */
  label: string;
  pages: readonly ViewPage[];
  page: string;
  onPage: (id: string) => void;
}) {
  const items = useRef(new Map<string, HTMLButtonElement>());
  const [aimed, setAimed] = useState<string | undefined>(undefined);
  const stop = pages.some((one) => one.id === aimed)
    ? aimed
    : pages.some((one) => one.id === page)
      ? page
      : pages[0]?.id;
  const keyed = (at: number) => (event: KeyboardEvent<HTMLButtonElement>) => {
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
          </li>
        ))}
      </ul>
    </nav>
  );
}

/** **A page's words in the navigation's picker**: its name and its count, as one option. */
function pickerLabel(one: ViewPage): string {
  return one.count !== undefined && one.count > 0 ? `${one.label} (${one.count})` : one.label;
}

/**
 * **The heading of a page's list, and its lead line**: the page's name (`h2`), the one line that
 * says what is on it, and the page's own controls after it (a filter, chips).
 */
export function ViewListHead({
  title,
  lead,
  children,
}: {
  title: ReactNode;
  lead?: ReactNode;
  /** The page's own controls under the lead line. */
  children?: ReactNode;
}) {
  return (
    <div data-scope="view-frame" data-part="head">
      <h2 className="ui-text-body ui-text-strong">
        {title}
      </h2>
      {lead !== undefined && (
        <p data-scope="view-frame" data-part="lead">
          {lead}
        </p>
      )}
      {children}
    </div>
  );
}

/**
 * **The page header of the item beside a list**: its name, its tag and one mark after the name,
 * the one-line description under it, and up to three actions then a "…" menu (`more`). It stays
 * in view while the item's body scrolls under it.
 */
export function ViewPageHeader({
  name,
  tag,
  mark,
  description,
  actions,
  more,
}: {
  name: ReactNode;
  /** A badge after the name: where it lives. */
  tag?: ReactNode;
  /** One mark after the tag: a badge of its state. */
  mark?: ReactNode;
  description?: ReactNode;
  /** Up to three buttons, the suggested one first. */
  actions?: ReactNode;
  /** The "…" menu after them: the rest of what can be done. */
  more?: ReactNode;
}) {
  return (
    <header data-scope="view-frame" data-part="header">
      <h2 className="ui-text-strong">
        <span data-scope="view-frame" data-part="name">
          {name}
        </span>
        {tag}
        {mark}
      </h2>
      {description !== undefined && (
        <p data-scope="view-frame" data-part="lead">
          {description}
        </p>
      )}
      {(actions !== undefined || more !== undefined) && (
        <ButtonRow size="sm">
          {actions}
          {more}
        </ButtonRow>
      )}
    </header>
  );
}

/** Whether `element` is under {@link NARROW_BELOW} wide, followed as it is resized. */
function useNarrow(element: HTMLElement | null, given: boolean | undefined): boolean {
  const [narrow, setNarrow] = useState(false);
  useLayoutEffect(() => {
    if (given !== undefined || element === null) return;
    // A frame not laid out yet (no width at all) is drawn wide, as it is meant to be seen first.
    const measure = () => {
      const width = element.getBoundingClientRect().width;
      setNarrow(width > 0 && width < NARROW_BELOW);
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const watch = new ResizeObserver(measure);
    watch.observe(element);
    return () => watch.disconnect();
  }, [element, given]);
  return given ?? narrow;
}

/** The ids the library's panels are laid out by, which the frame keeps between items. */
const LIST_PANEL = "view-frame-list";
const ITEM_PANEL = "view-frame-item";

/**
 * **A view tab laid out as pages.** The owner holds which page is current and which item is
 * chosen; the frame lays them out, and hands back a press on a page, and on the way back from an
 * item while it is narrow.
 */
export function ViewFrame({
  label,
  pages,
  page,
  onPage,
  list,
  item,
  opened = false,
  onBack,
  narrow: given,
  bar,
  history,
}: {
  /** What the page navigation is called: "Skill studio pages". */
  label: string;
  pages: readonly ViewPage[];
  /** The current page's id. */
  page: string;
  onPage: (id: string) => void;
  /** The current page's list: its {@link ViewListHead}, then the list itself. */
  list: ReactNode;
  /** The item chosen in the list: its {@link ViewPageHeader}, its banner and its body. A page
   *  with no item beside its list hands none. */
  item?: ReactNode;
  /** Narrow, an item was opened from the list: it is drawn alone, with a way back. */
  opened?: boolean;
  /** Back from an item to the list, while narrow. */
  onBack?: () => void;
  /** Narrow or wide whatever the width: for a story or a test. Measured when left out. */
  narrow?: boolean;
  /** The view's top bar over its panes: its {@link ViewBar}. */
  bar?: ReactNode;
  /** The tab's own history (`useViewHistory`): ⌘[ and ⌘] (Alt+← and Alt+→ off a Mac) and the
   *  mouse's back and forward buttons walk it anywhere in the view. */
  history?: HistorySteps;
}) {
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const narrow = useNarrow(root, given);
  const current = pages.find((one) => one.id === page);
  const back = useRef<HTMLButtonElement>(null);
  const listPane = useRef<HTMLElement>(null);
  const alone = narrow && opened && item !== undefined;
  // **The keyboard follows page-and-back**: onto the way back as an item opens alone, and onto
  // the list as the person comes back from it, so it is never left on what is no longer drawn.
  const was = useRef(alone);
  useLayoutEffect(() => {
    if (was.current === alone) return;
    was.current = alone;
    // Only the keyboard that was in the frame (or on nothing) follows: a resize across the
    // narrow width never takes it from a chat beside the view.
    const on = document.activeElement;
    if (on !== null && on !== document.body && root?.contains(on) !== true) return;
    if (alone) back.current?.focus();
    else listPane.current?.focus();
  }, [alone, root]);
  const goBack = useCallback(() => onBack?.(), [onBack]);

  const listDrawn = (
    <section
      ref={listPane}
      tabIndex={-1}
      // Narrow, while an item is drawn alone, the list stays drawn and hidden, so coming back
      // finds it where it was.
      hidden={alone || undefined}
      aria-label={current?.label}
      data-scope="view-frame"
      data-part="list"
    >
      {list}
    </section>
  );

  const body = narrow ? (
      <div data-scope="view-frame" data-part="body" data-narrow="">
        <div data-scope="view-frame" data-part="picker">
          <Select
            aria-label={label}
            size="sm"
            value={page}
            options={pages.map((one) => ({ value: one.id, label: pickerLabel(one) }))}
            onValueChange={onPage}
          />
        </div>
        {listDrawn}
        {alone && (
          <section data-scope="view-frame" data-part="item">
            <div data-scope="view-frame" data-part="back">
              <Button
                ref={back}
                variant="ghost"
                size="sm"
                icon={<ChevronLeft aria-hidden="true" />}
                onPress={goBack}
              >
                {`Back to ${current?.label ?? "the list"}`}
              </Button>
            </div>
            {item}
          </section>
        )}
      </div>
  ) : (
    <div data-scope="view-frame" data-part="body">
      <ViewNav label={label} pages={pages} page={page} onPage={onPage} />
      {/* The list is always the group's first panel, so choosing the first item, or the last
          going, never draws the list again: its scroll, its field and its focus stay. */}
      <Group orientation="horizontal" data-scope="view-frame" data-part="panes">
        <Panel id={LIST_PANEL} defaultSize={item === undefined ? "100%" : "42%"} minSize="25%">
          {listDrawn}
        </Panel>
        {item !== undefined && (
          <>
            <Separator aria-label="Resize the list and the item beside it" />
            <Panel id={ITEM_PANEL} minSize="30%">
              <section data-scope="view-frame" data-part="item">
                {item}
              </section>
            </Panel>
          </>
        )}
      </Group>
    </div>
  );

  return (
    <div
      ref={setRoot}
      // The keyboard's place when a crumb it was on goes (`ViewBar.tsx`); never a Tab stop.
      tabIndex={-1}
      data-scope="view-frame"
      data-part="root"
      data-narrow={narrow ? "" : undefined}
      onKeyDown={(event) => {
        // A key something inside already answered (a tab's arrows) is its own.
        if (history === undefined || event.defaultPrevented) return;
        const act = pressed(event.nativeEvent, ["view.back", "view.forward"]);
        if (act === undefined) return;
        event.preventDefault();
        if (act.id === "view.back") history.back();
        else history.forward();
      }}
      onMouseUp={(event) => {
        // The mouse's own back and forward buttons, anywhere in the view.
        if (history === undefined || (event.button !== 3 && event.button !== 4)) return;
        // As the keys do, they leave a dialog or a menu opened from the view to itself.
        if (aModalIsUp() || inADialog() || inAMenu(event.target)) return;
        event.preventDefault();
        if (event.button === 3) history.back();
        else history.forward();
      }}
    >
      {bar}
      {body}
    </div>
  );
}
