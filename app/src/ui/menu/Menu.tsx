// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The menu family** (#2909, spec #2897, ADR 0104): every menu in the window, a dropdown and a
 * context menu alike (`ContextMenu.tsx`), on React Aria's `MenuTrigger`, `Menu`, `MenuItem`,
 * `MenuSection`, `SubmenuTrigger` and `Separator`, wrapped here once. A screen imports these
 * and never `react-aria-components` (`eslint.config.js`).
 *
 * - **Drawn by its parts** (ADR 0103). A menu stands on the popover family's surface
 *   (`data-scope="popover"`, its `root`, its `list`, a group's `header` and the `rule` between
 *   groups). Each row is the menu family's root, `data-scope="menu" data-part="root"` with its
 *   `data-variant` (`default`, or `danger` for what cannot be undone, D-1719-3); inside it its
 *   `label` and the one `description` that may stand under it, its `shortcut`, and its `indicator`
 *   (a toggle's tick, a submenu's arrow). `menu.css` draws the family default and a theme's
 *   parts reach the same attributes.
 * - **Placed under its trigger, aligned to its start or its end**, never nearer the window's
 *   edge than 8 px (`EDGE`); a submenu opens beside its row.
 * - **Closed by a click outside, by Escape and by choosing a row**, and the keyboard goes back
 *   to its trigger (React Aria's `FocusScope`), or where `onClosed` puts it.
 * - **A row says its shortcut** where the act has one (the menus' rule 6): `keys`, or the key
 *   the window's key registry holds for `shortcut` (`MenuShortcuts`, #2921), in the row's
 *   `shortcut` part.
 * - **The keyboard is React Aria's**: the arrows move and stop at the ends, Right opens a
 *   submenu, Left and Escape close it, Enter and Space choose, and typing finds a row by its
 *   `textValue`.
 * - **A row a screen reader (or `element.click()`) presses takes the keyboard** (#3074), in a
 *   menu that takes the keyboard, so a submenu its row opened stays open when its menu's own
 *   hand-on of the keyboard comes after the press.
 */
import {
  createContext,
  useContext,
  useState,
  type ComponentPropsWithRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactElement,
  type ReactNode,
  type Ref,
  type RefObject,
} from "react";
import {
  Header as AriaHeader,
  MenuContext,
  MenuStateContext,
  OverlayTriggerStateContext,
  PopoverContext,
  Provider,
  RootMenuTriggerStateContext,
  Menu as AriaMenu,
  MenuItem as AriaMenuItem,
  MenuSection as AriaMenuSection,
  MenuTrigger as AriaMenuTrigger,
  Separator as AriaSeparator,
  SubmenuTrigger as AriaSubmenuTrigger,
  type Key,
  type MenuItemProps as AriaMenuItemProps,
  type MenuProps as AriaMenuProps,
  type MenuSectionProps as AriaMenuSectionProps,
  type RootMenuTriggerState,
  type Selection,
  useSlottedContext,
} from "react-aria-components";
import { useMenuTriggerState } from "react-stately/useMenuTriggerState";
import { Check, ChevronRight } from "lucide-react";
import {
  InPopoverList,
  OFFSET,
  Surface,
  SurfaceGone,
  useClosedFromOutside,
  useGone,
  type Opener,
  type SurfaceProps,
} from "../popover/Popover";
import { Tooltip } from "../tooltip/Tooltip";
import { pressableTrigger } from "../trigger";

export type { Key, Selection };

/** A row's variants: the plain row, and the row that cannot be undone (D-1719-3). */
export const MENU_VARIANTS = ["default", "danger"] as const;
export type MenuVariant = (typeof MENU_VARIANTS)[number];

// --- Shortcuts --------------------------------------------------------------------------------

/**
 * **Where a row's shortcut comes from**: the window's key registry (#2921) answers the key an
 * act has, by the act's id, and every menu says it at the row's end. Until the registry is
 * there, nothing answers and a row says the `keys` its caller knows.
 */
export const MenuShortcuts = createContext<(act: string) => string | undefined>(() => undefined);

// --- The trigger ------------------------------------------------------------------------------

/**
 * **A menu opened by a press on its trigger**: the trigger first, then its `Menu`. A button of
 * the family presses itself; any other element (a list's row, a strip's mark) is made
 * pressable. A press, Enter, Space or Down opens it on its first row, Up on its last.
 * `isOpen` and `onOpenChange` hold it where its owner decides when it is up.
 */
export function MenuTrigger({
  children,
  isOpen: held,
  defaultOpen = false,
  onOpenChange,
  onClosed,
}: {
  children: [ReactNode, ReactNode];
  isOpen?: boolean;
  defaultOpen?: boolean;
  onOpenChange?: (isOpen: boolean) => void;
  /** Called once it has closed: where the keyboard goes instead of back to the trigger. */
  onClosed?: () => void;
}) {
  const [own, setOwn] = useState(defaultOpen);
  const isOpen = held ?? own;
  const gone = useGone(isOpen, onClosed);
  const [trigger, menu] = children;
  return (
    <AriaMenuTrigger
      isOpen={isOpen}
      onOpenChange={(next) => {
        if (held === undefined) setOwn(next);
        onOpenChange?.(next);
      }}
    >
      {pressableTrigger(trigger)}
      <SurfaceGone.Provider value={gone}>
        <RootMenu.Provider value="trigger">{menu}</RootMenu.Provider>
      </SurfaceGone.Provider>
    </AriaMenuTrigger>
  );
}

/**
 * **A menu held open by its owner**, under an element that keeps its own presses and keys (a
 * list's row that is a roving item, a strip's mark that a click toggles): its owner says when it
 * is open and where the keyboard lands (`autoFocus`: the `first` row, the `last`, or the menu
 * itself, `true`), and says `aria-haspopup` and `aria-expanded` on the element. Escape, a click
 * outside and a row chosen ask `onOpenChange` to close it.
 */
export function AnchoredMenu({
  triggerRef,
  isOpen,
  onOpenChange,
  autoFocus = true,
  onClosed,
  children,
}: {
  triggerRef: RefObject<Element | null>;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  autoFocus?: "first" | "last" | boolean;
  onClosed?: () => void;
  /** The `Menu`. */
  children: ReactNode;
}) {
  const state = useMenuTriggerState({ isOpen, onOpenChange });
  return (
    <OpenMenu
      state={state}
      triggerRef={triggerRef}
      autoFocus={autoFocus}
      onClosed={onClosed}
      placedByItsOpener={false}
    >
      {children}
    </OpenMenu>
  );
}

/**
 * **Down and Up on an owner's element open its menu** on the first row and on the last, as they
 * open a menu React Aria's trigger opens: for an `AnchoredMenu`'s owner whose arrows are not its
 * own (a strip's mark; a roving row keeps its arrows for the rows).
 */
export function opensOnTheArrows(
  isOpen: boolean,
  open: (to: "first" | "last") => void,
): (event: ReactKeyboardEvent) => void {
  return (event) => {
    if (isOpen || (event.key !== "ArrowDown" && event.key !== "ArrowUp")) return;
    event.preventDefault();
    open(event.key === "ArrowDown" ? "first" : "last");
  };
}

/**
 * **What a menu React Aria's `MenuTrigger` did not open is handed**: its state, which a
 * submenu reads too, where the keyboard lands, how it closes, and what it stands beside. For
 * `AnchoredMenu` and `ContextMenu`.
 */
export function OpenMenu({
  state,
  triggerRef,
  autoFocus,
  onClosed,
  placedByItsOpener,
  children,
}: {
  state: RootMenuTriggerState;
  triggerRef: RefObject<Element | null>;
  autoFocus: "first" | "last" | boolean;
  onClosed?: () => void;
  /** A context menu stands at the pointer, which its state holds. */
  placedByItsOpener: boolean;
  children: ReactNode;
}) {
  const gone = useGone(state.isOpen, onClosed);
  if (!state.isOpen) return null;
  return (
    <Provider
      values={[
        [OverlayTriggerStateContext, state],
        [RootMenuTriggerStateContext, state],
        [MenuContext, { autoFocus, onClose: state.close }],
        [
          PopoverContext,
          {
            trigger: placedByItsOpener ? "ContextMenu" : "MenuTrigger",
            triggerRef,
            placement: "bottom start",
            offset: placedByItsOpener ? 0 : OFFSET,
          },
        ],
      ]}
    >
      <SurfaceGone.Provider value={gone}>
        <RootMenu.Provider value={placedByItsOpener ? "none" : "owner"}>
          <PlacedByItsOpener.Provider value={placedByItsOpener}>
            {children}
          </PlacedByItsOpener.Provider>
        </RootMenu.Provider>
      </SurfaceGone.Provider>
    </Provider>
  );
}

// --- The menu ---------------------------------------------------------------------------------

/**
 * **The arrows stop at a menu's ends** and do not wrap round to its other end, as every menu in
 * the window did before React Aria and every list in a popover does (R109). React Aria's menu
 * takes this option and its types leave it out, so it is spread in.
 */
const STOPS_AT_THE_ENDS: object = { shouldFocusWrap: false };

/** Whether the menu being drawn is placed by what opened it: a submenu, beside its row, or a
 *  context menu, at the pointer. */
const PlacedByItsOpener = createContext(false);

/** Takes away the trigger's name React Aria hands a menu, where its owner names it. */
const NOT_BY_ITS_TRIGGER = { "aria-labelledby": null };

export type MenuProps<T extends object = object> = Omit<
  AriaMenuProps<T>,
  "className" | "style" | "render" | "children"
> &
  Pick<SurfaceProps, "side" | "align" | "offset" | "isNonModal" | "className" | "style"> & {
    children: ReactNode;
    /** A key on the menu, before its own. */
    onKeyDown?: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
    /** A preview the pointer's rest opened: no row takes the keyboard, and Escape or a press
     *  outside closes it. */
    takesNoKeyboard?: boolean;
    /** The list's element, for an owner that hands it the keyboard itself. */
    listRef?: Ref<HTMLDivElement>;
    /** The pointer coming onto the surface and leaving it, for a menu that closes as it leaves. */
    onPointerEnter?: () => void;
    onPointerLeave?: () => void;
    /** What stands under the list and is no row: what binds, what was used. */
    footer?: ReactNode;
    /** `data-*` attributes reach the surface as they are. */
    [data: `data-${string}`]: string | number | boolean | undefined;
  };

/**
 * **A menu**: the surface, under its trigger (or beside its row, for a submenu), and the list of
 * its rows, named by `aria-label` or `aria-labelledby`: the thing it is about, said once (the
 * menus' rule 1). `className` is its place (a width); its look is the family's.
 */
export function Menu<T extends object = object>({
  side,
  align,
  offset,
  isNonModal,
  takesNoKeyboard = false,
  footer,
  listRef,
  onPointerEnter,
  onPointerLeave,
  className,
  style,
  children,
  ...rest
}: MenuProps<T>) {
  const placedForIt = useContext(PlacedByItsOpener);
  const root = useContext(RootMenu);
  // **A menu named by `aria-label` is named by it**: React Aria labels a menu by its trigger,
  // and a name its owner gives (the thing it is about, the menus' rule 1) wins over that.
  const labelledBy =
    rest["aria-labelledby"] ?? (rest["aria-label"] === undefined ? undefined : null);
  const opener = (useSlottedContext(PopoverContext) as { triggerRef?: RefObject<Element | null> })
    ?.triggerRef;
  const data: Record<string, unknown> = {};
  const menu: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(rest))
    (key.startsWith("data-") ? data : menu)[key] = value;
  return (
    <>
      {root !== null && <ClosedFromOutside opener={root} escape={takesNoKeyboard} />}
      <Surface
        kind="menu"
        // A submenu's place is its row's, which React Aria hands it, and a context menu's is the
        // pointer's.
        placed={!placedForIt}
        side={side}
        align={align}
        offset={offset}
        // **Not modal** (`docs/ui-primitives.md`, "Menus"): a menu is not a question, so the rest
        // of the window is not hidden from a screen reader nor kept from the pointer while it
        // is up. A press outside closes it and still reaches what it was on.
        isNonModal={isNonModal ?? true}
        className={className}
        style={style}
        aria-label={rest["aria-label"]}
        aria-labelledby={labelledBy ?? undefined}
        onPointerEnter={onPointerEnter}
        onPointerLeave={onPointerLeave}
        // The keyboard going to an owner's element is that element's press, which says itself
        // what it does to the menu: it does not close it on its way.
        shouldCloseOnInteractOutside={
          root === "owner"
            ? (element: Element) => !(opener?.current?.contains(element) ?? false)
            : undefined
        }
        {...data}
      >
        <PlacedByItsOpener.Provider value={false}>
          <RootMenu.Provider value={null}>
            <TakesNoKeyboard.Provider value={takesNoKeyboard}>
              <AriaMenu
                {...STOPS_AT_THE_ENDS}
                {...(menu as AriaMenuProps<T>)}
                {...(takesNoKeyboard ? { autoFocus: false } : {})}
                {...(labelledBy === null ? (NOT_BY_ITS_TRIGGER as object) : {})}
                ref={listRef}
                data-scope="popover"
                data-part="list"
              >
                {children}
              </AriaMenu>
            </TakesNoKeyboard.Provider>
          </RootMenu.Provider>
        </PlacedByItsOpener.Provider>
        {footer !== undefined && footer !== null && footer !== false && (
          <div data-scope="popover" data-part="footer" data-of="rows">
            {footer}
          </div>
        )}
      </Surface>
    </>
  );
}

/**
 * **A menu that takes no keyboard** (a preview the pointer's rest opened): its rows show the
 * pointer's place by their hover and never take the keyboard from whoever has it.
 */
const TakesNoKeyboard = createContext(false);

/**
 * **What opened the menu being drawn**, for a menu at the root (a submenu closes with its
 * menu): React Aria's trigger, whose press while it is open closes it; an owner's element,
 * whose own press says what a press on it does; or nothing to stand back for (a context menu).
 */
const RootMenu = createContext<Opener | null>(null);

/**
 * **A press outside closes a menu**, as on every platform, and still reaches what it was on:
 * React Aria closes a menu that is not modal only when the keyboard leaves it for something
 * else. A press on React Aria's own trigger while it is open closes it and opens nothing; an
 * owner's element is left to its own press. For a menu that took no keyboard, Escape closes it
 * too, from wherever the keyboard is, and goes no further: the menu was what it was for.
 */
function ClosedFromOutside({ opener, escape }: { opener: Opener; escape: boolean }) {
  const state = useContext(OverlayTriggerStateContext);
  const popover = useSlottedContext(PopoverContext) as { triggerRef?: RefObject<Element | null> };
  useClosedFromOutside({
    isOpen: state?.isOpen ?? false,
    close: () => state?.close(),
    triggerRef: popover?.triggerRef,
    opener,
    escape,
  });
  return null;
}

/**
 * **A menu drawn in place**, not over the window: a short list of rows to pick from inside a
 * dialog or under a list, with the menu's keyboard. Named by `aria-label`.
 */
export function InlineMenu<T extends object = object>({
  children,
  className,
  ...rest
}: Omit<AriaMenuProps<T>, "className" | "style" | "render" | "children"> & {
  children: ReactNode;
  /** Its place in its view. Never a look. */
  className?: string;
  ref?: Ref<HTMLDivElement>;
}) {
  // **Inside a popover's list (R165), the list is that list**: its rows are laid out in it, so
  // a theme's padding and gap reach them once, and this is no second `list` part.
  const inList = useContext(InPopoverList);
  return (
    <AriaMenu
      {...STOPS_AT_THE_ENDS}
      {...rest}
      className={className}
      data-scope="popover"
      data-part={inList ? undefined : "list"}
      data-in-list={inList ? "" : undefined}
      data-inline=""
    >
      {children}
    </AriaMenu>
  );
}

// --- Rows -------------------------------------------------------------------------------------

export type MenuItemProps<T extends object = object> = Omit<
  AriaMenuItemProps<T>,
  "className" | "style" | "children" | "render"
> & {
  variant?: MenuVariant;
  /** The key that does the same, said at the row's end, where the caller knows it. */
  keys?: string;
  /** The act's id in the key registry, whose key the row says when the registry has one. */
  shortcut?: string;
  /** A row's house meaning (`ends-it`), or its place. Never a look. */
  className?: string;
  /** The row's hint, drawn by the tooltip family: what its words do not say (a name cut short,
   *  why it cannot run). */
  tooltip?: string;
  /** A row that cannot run and still stands in the arrows' way, saying why. */
  "aria-disabled"?: boolean;
  /** A row that opens something in place under it (a report), and whether it is open. */
  "aria-expanded"?: boolean;
  "aria-keyshortcuts"?: string;
  /** A key on the row, before the row's and the menu's own: preventing its default keeps it
   *  from them. */
  onKeyDown?: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
  children: ReactNode;
  /** `data-*` attributes reach the row as they are. */
  [data: `data-${string}`]: string | number | boolean | undefined;
};

/**
 * **A row**: its words (`MenuWords`, or what the caller draws), the key that does the same at
 * its end, and its indicator: a tick on a toggle that is on, an arrow on a row that opens a submenu.
 */
export function MenuItem<T extends object = object>({
  variant = "default",
  keys,
  shortcut,
  tooltip,
  "aria-disabled": unavailable,
  "aria-expanded": expanded,
  "aria-keyshortcuts": keyShortcuts,
  onHoverStart,
  onPressStart,
  onKeyDown,
  children,
  ...rest
}: MenuItemProps<T>) {
  const registered = useContext(MenuShortcuts);
  const key = (shortcut === undefined ? undefined : registered(shortcut)) ?? keys;
  const noKeyboard = useContext(TakesNoKeyboard);
  const menu = useContext(MenuStateContext);
  const native = {
    "aria-disabled": unavailable || undefined,
    "aria-expanded": expanded,
    "aria-keyshortcuts": keyShortcuts,
  };
  const given = Object.fromEntries(Object.entries(native).filter(([, one]) => one !== undefined));
  const drawnHere =
    onKeyDown !== undefined || tooltip !== undefined || Object.keys(given).length > 0;
  return (
    <AriaMenuItem
      {...(rest as AriaMenuItemProps<T>)}
      data-scope="menu"
      data-part="root"
      data-variant={variant}
      data-unavailable={unavailable ? "" : undefined}
      onHoverStart={(event) => {
        // In a menu that takes no keyboard, the pointer coming onto a row is where it is and
        // nothing more: React Aria would hand the row the keyboard, and it is taken back here,
        // before the row is focused.
        if (noKeyboard) menu?.selectionManager.setFocused(false);
        onHoverStart?.(event);
      }}
      onPressStart={(event) => {
        // **A row pressed is where the keyboard is** (#3074). A press that is not the pointer's
        // (a screen reader's, `element.click()`) moves no focus, and React Aria hands a menu
        // opened by one the keyboard only once the page's transitions end, which can be after
        // this press. Its submenu would then close as its menu took the keyboard back. Taken
        // here, the menu's late hand-on finds the keyboard already moved and leaves it.
        const row = event.target;
        if (
          event.pointerType === "virtual" &&
          !noKeyboard &&
          row instanceof HTMLElement &&
          !row.contains(row.ownerDocument.activeElement)
        )
          row.focus({ preventScroll: true });
        onPressStart?.(event);
      }}
      render={
        drawnHere
          ? (dom) => {
              const own = dom as ComponentPropsWithRef<"div">;
              const row = (
                <div
                  {...own}
                  {...given}
                  onKeyDown={(event) => {
                    onKeyDown?.(event);
                    if (!event.defaultPrevented) own.onKeyDown?.(event);
                  }}
                />
              );
              return tooltip === undefined ? row : <Tooltip tip={tooltip}>{row}</Tooltip>;
            }
          : undefined
      }
    >
      {({ isSelected, selectionMode, hasSubmenu }) => (
        <>
          {children}
          {/* Said where it is seen, and not run onto the row's description: a row is named and
              described by its own words, and the key registry says a key to a screen reader
              by its name (#2921). */}
          {key !== undefined && key !== "" && (
            <kbd data-scope="menu" data-part="shortcut">
              {key}
            </kbd>
          )}
          {/* A toggle is one row with a tick at its end while it is on (the menus' rule 6). */}
          {selectionMode !== "none" && (
            <span data-scope="menu" data-part="indicator" data-indicator="check" aria-hidden="true">
              {isSelected && <Check />}
            </span>
          )}
          {hasSubmenu && (
            <span
              data-scope="menu"
              data-part="indicator"
              data-indicator="submenu"
              aria-hidden="true"
            >
              <ChevronRight />
            </span>
          )}
        </>
      )}
    </AriaMenuItem>
  );
}

/**
 * **A row's words, and what may stand under them** (the menus' rule 2): what a row that cannot be
 * undone costs, or why a row cannot run. The words are one line and give way to an ellipsis;
 * **what is under them is never cut**, and wraps.
 */
export function MenuWords({ said, under }: { said: ReactNode; under?: string }) {
  return (
    <span data-scope="menu" data-part="words">
      <span data-scope="menu" data-part="label">
        {said}
      </span>
      {under !== undefined && under !== "" && (
        <span data-scope="menu" data-part="description">
          {under}
        </span>
      )}
    </span>
  );
}

/** **A group of rows**, named by its `title` when it has one: purlis's own words in the label
 *  role, or a name from data (`named`), written as it was given. */
export function MenuSection<T extends object = object>({
  title,
  named,
  children,
  ...rest
}: Omit<AriaMenuSectionProps<T>, "className" | "style" | "children"> & {
  title?: string;
  named?: boolean;
  children: ReactNode;
}) {
  return (
    <AriaMenuSection {...(rest as AriaMenuSectionProps<T>)} data-scope="popover" data-part="group">
      {title !== undefined && (
        <AriaHeader data-scope="popover" data-part="header" data-named={named ? "" : undefined}>
          {title}
        </AriaHeader>
      )}
      {children}
    </AriaMenuSection>
  );
}

/** **The rule between groups**, and above everything that loses something. */
export function MenuSeparator() {
  return <AriaSeparator data-scope="popover" data-part="rule" />;
}

/**
 * **A row that opens a submenu** (the menus' rule 3): `row` is its `MenuItem`, and `children`
 * the submenu's rows. Right opens it, Left and Escape close it, and a pointer resting on the
 * row opens it.
 */
export function Submenu({
  row,
  children,
  ...menu
}: Omit<MenuProps, "children" | "side" | "align"> & {
  row: ReactElement;
  children: ReactNode;
}) {
  return (
    <AriaSubmenuTrigger>
      {row}
      <SurfaceGone.Provider value={undefined}>
        <PlacedByItsOpener.Provider value>
          <Menu {...menu}>{children}</Menu>
        </PlacedByItsOpener.Provider>
      </SurfaceGone.Provider>
    </AriaSubmenuTrigger>
  );
}
