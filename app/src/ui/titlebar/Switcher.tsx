// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What the title bar's switchers are made of** (#2916, R94–R100, R107–R109): the workspace
 * popover the ⌄ opens and the two-column switcher the project's crumb opens while the rail is
 * hidden. Both stand on the popover family's surface and anatomy (its filter row, list and
 * footer, `ui/popover/Popover.tsx`), and the filter is the field family's box. What a switcher
 * adds is this family's own: its rows, its pin toggles, its marks and its key cap.
 *
 * - **The filter keeps the keyboard** (R95): a combobox whose ↑ and ↓ move the row it would go
 *   to (`aria-activedescendant`), so typing goes on narrowing while a row is chosen, and Enter
 *   goes there. Its caret is put at the end as it opens.
 * - **A row is an option of a listbox**, named by its words, with what waits in it as its
 *   description (WCAG 2.5.3: the name begins with what is drawn). The pin toggle beside it (R107)
 *   stands in a column of its own beside the list, row for row, since an option holds no control.
 */
import {
  useLayoutEffect,
  useRef,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { Pin } from "lucide-react";
import { mergeProps, useFocusRing, useHover, useId } from "react-aria";
import { TextField } from "../field/Field";
import { PopoverFilter, PopoverList } from "../popover/Popover";
import { Tooltip } from "../tooltip/Tooltip";

/**
 * **The filter row** (R95, R107): the field family's box that narrows the list under it and keeps
 * the keyboard while its arrows move the chosen row, then the key that opens it in a cap.
 */
export function SwitcherFilter({
  label,
  value,
  onChange,
  listbox,
  active,
  keys,
  onKeyDown,
}: {
  /** What it finds, which is its placeholder and its name. */
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** The id of the list it narrows and moves through. */
  listbox: string;
  /** The id of the row Enter would go to, or none. */
  active?: string;
  /** The key that opens it, said at its end: `⌥⌘P`. */
  keys: string;
  onKeyDown: (event: ReactKeyboardEvent<HTMLInputElement>) => void;
}) {
  const box = useRef<HTMLInputElement>(null);
  // The keyboard is put in it as it opens, with the caret at the end of what it holds (R95).
  useLayoutEffect(() => {
    const input = box.current;
    if (input === null) return;
    input.focus({ preventScroll: true });
    input.setSelectionRange(input.value.length, input.value.length);
  }, []);
  return (
    <PopoverFilter>
      <div data-scope="titlebar" data-part="filter">
        <TextField
          inputRef={box}
          className="titlebar-filter"
          aria-label={label}
          placeholder={label}
          role="combobox"
          aria-expanded="true"
          aria-controls={listbox}
          aria-activedescendant={active}
          aria-autocomplete="list"
          autoComplete="off"
          spellCheck={false}
          value={value}
          onChange={onChange}
          onKeyDown={onKeyDown}
        />
        <kbd data-scope="titlebar" data-part="key" aria-hidden="true">
          {keys}
        </kbd>
      </div>
    </PopoverFilter>
  );
}

/**
 * **A list of rows to go to** (R96, R107): a listbox named `label`, in the popover family's
 * scrolling list, under a header that says it when `header` is given, with the pin toggles in a
 * column beside it where `pins` is given.
 */
export function SwitcherList({
  id,
  label,
  header,
  empty,
  pins,
  nothing,
  children,
}: {
  id: string;
  label: string;
  /** The column's heading: `Projects`, `Workspaces in purlis`. */
  header?: string;
  /** What it says when nothing matches (R99): `No project matches.` */
  empty?: string;
  /** The pin toggles, one per row, in the rows' order. */
  pins?: ReactNode;
  /** Nothing matched, though the list still holds a row that is no match (New workspace…). */
  nothing?: boolean;
  children: ReactNode;
}) {
  const none = nothing ?? (Array.isArray(children) ? children.length === 0 : children === null);
  return (
    <PopoverList>
      {header !== undefined && (
        <div data-scope="titlebar" data-part="header" aria-hidden="true">
          {header}
        </div>
      )}
      <div data-scope="titlebar" data-part="rows">
        <div id={id} role="listbox" aria-label={label} data-scope="titlebar" data-part="listbox">
          {children}
        </div>
        {pins !== undefined && (
          <div role="group" aria-label={`Pins of ${label}`} data-scope="titlebar" data-part="pins">
            {pins}
          </div>
        )}
      </div>
      {none && empty !== undefined && (
        <div data-scope="titlebar" data-part="empty">
          {empty}
        </div>
      )}
    </PopoverList>
  );
}

/**
 * **A row** (R96, R107): its mark, its name and its waiting count, 32px, the card's fill under the
 * pointer and the accent's wash on the row Enter would go to (`active`, marked selected as well, as
 * the family's state is keyed); a press goes to it. Named by its words; what waits in it is its
 * description.
 */
export function SwitcherRow({
  id,
  name,
  waiting = 0,
  active,
  mark,
  unavailable,
  onPress,
}: {
  id: string;
  name: string;
  waiting?: number;
  active: boolean;
  /** A project's mark, or the plane root's icon, before its name. */
  mark?: ReactNode;
  /** Why it cannot run now, where it cannot (a project this purlis opens read-only): the row
   *  stands in the arrows' way, faint, and says why as its description. */
  unavailable?: string;
  onPress: () => void;
}) {
  const { hoverProps, isHovered } = useHover({});
  const said = useId();
  const row = useRef<HTMLDivElement>(null);
  // The row Enter would go to stays in sight as the arrows move it.
  useLayoutEffect(() => {
    if (active) row.current?.scrollIntoView?.({ block: "nearest" });
  }, [active]);
  const description = [waiting > 0 ? `${waiting} waiting` : undefined, unavailable].filter(
    (one) => one !== undefined,
  );
  return (
    <div
      {...mergeProps(hoverProps, {
        onClick: unavailable === undefined ? onPress : undefined,
        // A press keeps the keyboard in the filter, where it was.
        onMouseDown: (event: ReactMouseEvent) => event.preventDefault(),
      })}
      ref={row}
      id={id}
      role="option"
      aria-selected={active}
      aria-disabled={unavailable === undefined ? undefined : true}
      aria-describedby={description.length > 0 ? said : undefined}
      data-scope="titlebar"
      data-part="row"
      data-active={active ? "" : undefined}
      // The family's selected state, as `aria-selected` says it: a theme's
      // `titlebar.row.selected` keys reach the row Enter would go to by it (#3656).
      data-selected={active ? "" : undefined}
      data-hovered={isHovered ? "" : undefined}
      data-unavailable={unavailable === undefined ? undefined : ""}
    >
      {mark}
      <span data-scope="titlebar" data-part="row-name" data-titlebar-name="">
        {name}
      </span>
      {waiting > 0 && (
        <span data-scope="titlebar" data-part="count" aria-hidden="true">
          {waiting > 99 ? "99+" : String(waiting)}
        </span>
      )}
      {description.length > 0 && (
        <span id={said} hidden>
          {description.join(". ")}
        </span>
      )}
    </div>
  );
}

/**
 * **A row's pin** (R107): 28px, the icon faint at rest and in the accent while pinned, no box and
 * no edge of its own; the card's fill under the pointer. Its tooltip is `Unpin` or `Pin to the
 * title bar`. A press keeps the popover open (R109).
 */
export function PinToggle({
  name,
  pinned,
  onChange,
}: {
  name: string;
  pinned: boolean;
  onChange: (pinned: boolean) => void;
}) {
  const { hoverProps, isHovered } = useHover({});
  const { focusProps, isFocusVisible } = useFocusRing();
  return (
    <Tooltip tip={pinned ? "Unpin" : "Pin to the title bar"}>
      <button
        type="button"
        {...mergeProps(hoverProps, focusProps, {
          onClick: () => onChange(!pinned),
          // A press keeps the keyboard where it was: the filter goes on narrowing.
          onMouseDown: (event: ReactMouseEvent) => event.preventDefault(),
        })}
        tabIndex={0}
        aria-label={pinned ? `Unpin ${name}` : `Pin ${name} to the title bar`}
        aria-pressed={pinned}
        data-scope="titlebar"
        data-part="pin"
        data-selected={pinned ? "" : undefined}
        data-hovered={isHovered ? "" : undefined}
        data-focus-visible={isFocusVisible ? "" : undefined}
        data-titlebar-pin=""
      >
        <Pin aria-hidden="true" />
      </button>
    </Tooltip>
  );
}

/** **A project's mark** (R96): its monogram in its own colour, as the rail draws it. */
export function SwitcherMark({
  letters,
  colour,
  small = false,
}: {
  letters: string;
  /** 20px, as the crumb draws it (R93), where a switcher's row draws 22. */
  small?: boolean;
  /** Its colour, a name of the theme's palette, and the tokens it draws in it
   *  (`ui/rail/projectColour.ts`). */
  colour?: { colour: string | undefined; tint: Record<string, string> };
}) {
  return (
    <span
      data-scope="titlebar"
      data-part="mark"
      data-letters={letters}
      data-colour={colour?.colour}
      data-small={small ? "" : undefined}
      style={colour?.tint as CSSProperties}
      aria-hidden="true"
    />
  );
}

/** **A row's icon mark**: the plane root's house, or New workspace…'s plus, where a project has
 *  its monogram. */
export function SwitcherIcon({ children }: { children: ReactNode }) {
  return (
    <span data-scope="titlebar" data-part="mark" data-small="" aria-hidden="true">
      {children}
    </span>
  );
}

/** **The two-column switcher's columns** (R94), the line between them a part of its own. */
export function SwitcherColumns({ children }: { children: [ReactNode, ReactNode] }) {
  const [projects, workspaces] = children;
  return (
    <div data-scope="titlebar" data-part="columns">
      {projects}
      <div data-scope="titlebar" data-part="divider" aria-hidden="true" />
      {workspaces}
    </div>
  );
}
