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
import { useCallback, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { ChevronLeft } from "lucide-react";
import { Group, Panel } from "react-resizable-panels";
import { Button, ButtonRow } from "../button/Button";
import { Select } from "../select/Select";
import { aModalIsUp, inADialog, inAMenu, VIEW_HOLDER } from "../../keys/contexts";
import { pressed } from "../../keys/registry";
import { Separator } from "./Separator";
import type { HistorySteps } from "./viewHistory";
import { ViewNav, type ViewPage } from "./ViewNav";

export { ViewNav, type ViewPage } from "./ViewNav";

/** **Below this width the frame is page-and-back**, in px: the navigation's column, the least a
 *  list keeps and the least an item keeps, side by side (the prototype's 170, 260 and 290). */
export const NARROW_BELOW = 720;

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
  action,
  children,
}: {
  title: ReactNode;
  lead?: ReactNode;
  /** One small button at the end of the heading's line (#5332): the Library's Select. */
  action?: ReactNode;
  /** The page's own controls under the lead line. */
  children?: ReactNode;
}) {
  const heading = <h2 className="ui-text-body ui-text-strong">{title}</h2>;
  return (
    <div data-scope="view-frame" data-part="head">
      {action === undefined ? (
        heading
      ) : (
        <div data-scope="view-frame" data-part="heading">
          {heading}
          {action}
        </div>
      )}
      {lead !== undefined && (
        <p data-scope="view-frame" data-part="lead">
          {lead}
        </p>
      )}
      {children}
    </div>
  );
}

/** **The custom property the page header writes its height to**, on the item's pane, which keeps
 *  that much room at its top when it scrolls the keyboard's place into view (#3944). */
export const HEADER_SIZE = "--view-frame-header-size";

/**
 * **The page header of the item beside a list**: its name, its tag and one mark after the name,
 * the one-line description under it, and up to three actions then a "…" menu (`more`). It stays
 * in view, on its panel's fill, while the item's body scrolls under it; and it tells the item's
 * pane how tall it is, so what the keyboard reaches there is scrolled clear of it (WCAG 2.4.11).
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
  const header = useRef<HTMLElement>(null);
  useLayoutEffect(() => {
    const it = header.current;
    const pane = it?.closest<HTMLElement>('[data-scope="view-frame"][data-part="item"]');
    if (!it || !pane) return;
    // Its height changes as its description wraps, or as the pane is narrowed or widened.
    // Its exact height, not `offsetHeight`'s whole pixels, so the room kept is never short of it.
    const write = () =>
      pane.style.setProperty(HEADER_SIZE, `${it.getBoundingClientRect().height}px`);
    write();
    if (typeof ResizeObserver === "undefined") return () => pane.style.removeProperty(HEADER_SIZE);
    const watch = new ResizeObserver(write);
    watch.observe(it);
    return () => {
      watch.disconnect();
      pane.style.removeProperty(HEADER_SIZE);
    };
  }, []);
  return (
    <header ref={header} data-scope="view-frame" data-part="header">
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
      // Its Back and Forward keys are its own off a Mac, before the window's pane keys.
      {...{ [VIEW_HOLDER]: "" }}
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
