// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The select family** (#2906, spec #2897, ADR 0104): every one-of-many pick in the window, on
 * React Aria's `Select`, `Button`, `SelectValue`, `ListBox`, `ListBoxItem` and `Autocomplete`,
 * wrapped here once. A screen draws no `<select>` of its own (`purlis/select-family`, an error)
 * and never imports the primitives library.
 *
 * - **Drawn by its parts** (ADR 0103). The button is the family's root, `data-scope="select"
 *   data-part="root"`, drawn as the text input's box; in it what is picked (`value`), what it
 *   shows while nothing is (`placeholder`, quiet) and the drawn `chevron` at its end (quiet,
 *   R64). Its list stands on the popover family's `menu` surface and each option is the menu
 *   family's row, its tick the row's `indicator`. A screen gives a select a place (`className`
 *   for margin, width, flex, order), never a look.
 * - **Sizes**: `sm` 24, `md` 28 (the default), `lg` 32, as the text input's. One size per row.
 * - **Keyboard first**: Space, Enter, ↑ or ↓ open it on the option it holds; ↑ and ↓ move and
 *   stop at the ends; Enter picks; Escape closes it and leaves the value as it was; a letter
 *   typed goes to the first option it starts. **A long list is filtered** ({@link FILTERED_FROM}
 *   options or more): the list opens with a filter row at its top that takes the keyboard, and
 *   what is typed narrows the list while ↑ and ↓ still move in it.
 * - **An empty value is an option like any other** ("Every workspace", "not set"): React Aria
 *   keeps no key for nothing, so an option's `""` travels as a word of its own and comes back as
 *   `""`. A value none of the options has shows `placeholder`, as nothing picked.
 * - **The button carries what it holds as `data-value`**, for the window's e2e specs to read the
 *   way they read a field's value.
 * - **Drawn only while its project is in front**: its list is the popover family's surface.
 */
import {
  useId,
  useState,
  type ComponentPropsWithRef,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import {
  Autocomplete,
  Button as AriaButton,
  ListBox,
  ListBoxItem,
  Select as AriaSelect,
  SelectValue,
  useFilter,
  type Key,
} from "react-aria-components";
import { Check, ChevronDown, Search } from "lucide-react";
import { TextField, type FieldSize } from "../field/Field";
import { PopoverFilter, Surface } from "../popover/Popover";
import { Tooltip } from "../tooltip/Tooltip";

/** The select's sizes: the text input's, 24, 28 and 32 px at the default text size and density. */
export const SELECT_SIZES = ["sm", "md", "lg"] as const satisfies readonly FieldSize[];
export type SelectSize = (typeof SELECT_SIZES)[number];

/** **From how many options a select's list is filtered** (R165: a list of more than a few is
 *  narrowed by typing). Below it, the letters typed go to the option they start. */
export const FILTERED_FROM = 10;

/** An option: what a pick writes, the words it shows, and whether it can be picked. An option
 *  out of reach says why in its `tooltip`. */
export type SelectOption = {
  value: string;
  label: string;
  isDisabled?: boolean;
  tooltip?: string;
};

export type SelectProps = {
  /** The button's id, which a row's `<label for>` points at. */
  id?: string;
  /** Its place in its view: margin, width, flex, order. Never a look. */
  className?: string;
  value: string;
  options: readonly SelectOption[];
  onValueChange: (to: string) => void;
  /** What it shows while it holds a value none of the options has. */
  placeholder?: string;
  isDisabled?: boolean;
  /** Its hint, the shared tooltip (#2908). */
  tooltip?: string;
  size?: SelectSize;
  /** Filter its list whatever its length (`true`), or never (`false`); by its length when left
   *  out ({@link FILTERED_FROM}). */
  filtered?: boolean;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  /** `data-*` attributes reach the button as they are: a test's id. */
  [data: `data-${string}`]: string | number | boolean | undefined;
};

/** The key `""` travels as inside React Aria: no value a file or a view writes is this. */
const EMPTY = "\u0000empty";

const into = (value: string): string => (value === "" ? EMPTY : value);
const outOf = (key: Key): string => (key === EMPTY ? "" : String(key));

/** **A select**: the button, and the list it opens under it. */
export function Select({
  id,
  className,
  value,
  options,
  onValueChange,
  placeholder,
  isDisabled,
  tooltip,
  size = "md",
  filtered,
  ...rest
}: SelectProps) {
  const held = options.some((one) => one.value === value);
  const long = filtered ?? options.length >= FILTERED_FROM;
  const aria: Record<string, unknown> = {};
  const data: Record<string, unknown> = {};
  for (const [key, one] of Object.entries(rest)) (key.startsWith("data-") ? data : aria)[key] = one;
  return (
    <AriaSelect
      {...aria}
      value={held ? into(value) : null}
      onChange={(key) => {
        if (key === null) return;
        const to = outOf(key as Key);
        // An option out of reach that says why is in the arrows' way, and is never picked.
        if (!options.some((one) => one.value === to && saysWhy(one))) onValueChange(to);
      }}
      isDisabled={isDisabled}
      placeholder={placeholder ?? ""}
      data-scope="select"
      data-part="field"
    >
      {({ isOpen }) => (
        <>
          <AriaButton
            {...data}
            id={id}
            className={className}
            data-scope="select"
            data-part="root"
            data-size={size}
            data-open={isOpen ? "" : undefined}
            data-value={held ? value : ""}
            // #190: WebKit leaves a button out of the Tab order without `tabIndex`. Its hint is
            // the shared tooltip's, which keeps an unavailable select's reason heard (#2908).
            render={(dom) => {
              const own = dom as ComponentPropsWithRef<"button">;
              const button = (
                <button
                  {...own}
                  tabIndex={own.tabIndex ?? 0}
                  // Escape on a select whose list is closed is for what it stands in (a form
                  // that closes, a dialog): React Aria would keep it.
                  onKeyDown={(event) => {
                    if (event.key !== "Escape") own.onKeyDown?.(event);
                  }}
                />
              );
              return tooltip ? <Tooltip tip={tooltip}>{button}</Tooltip> : button;
            }}
          >
            <SelectValue data-scope="select" data-part="value" data-shows-words="">
              {({ isPlaceholder, selectedText }) =>
                isPlaceholder ? (
                  <span data-scope="select" data-part="placeholder">
                    {placeholder}
                  </span>
                ) : (
                  selectedText
                )
              }
            </SelectValue>
            <span data-scope="select" data-part="chevron" aria-hidden="true">
              <ChevronDown />
            </span>
          </AriaButton>
          <Surface kind="menu" data-of="select" data-layout={long ? "list" : undefined}>
            {long ? <FilteredList options={options} /> : <OptionList options={options} />}
          </Surface>
        </>
      )}
    </AriaSelect>
  );
}

/** **The list of a long select**: a filter row at its top, which takes the keyboard as it opens,
 *  and the options that match under it. */
function FilteredList({ options }: { options: readonly SelectOption[] }) {
  const { contains } = useFilter({ sensitivity: "base" });
  const [typed, setTyped] = useState("");
  return (
    <Autocomplete filter={contains} inputValue={typed} onInputChange={setTyped}>
      <PopoverFilter>
        <TextField
          size="sm"
          aria-label="Filter"
          placeholder="Filter"
          icon={<Search />}
          autoFocus
          value={typed}
          onChange={setTyped}
        />
      </PopoverFilter>
      <OptionList options={options} none="Nothing matches what you typed." />
    </Autocomplete>
  );
}

/** **The options**: each one the menu family's row, its tick at its end while it is picked. */
function OptionList({ options, none }: { options: readonly SelectOption[]; none?: string }) {
  return (
    <ListBox
      data-scope="popover"
      data-part="list"
      renderEmptyState={() => (
        <span data-scope="select" data-part="none">
          {none}
        </span>
      )}
    >
      {options.map((one) => (
        <Option key={into(one.value)} one={one} />
      ))}
    </ListBox>
  );
}

/** The handlers a press reaches an option by, which an option out of reach never hears. */
const PRESSES = [
  "onPointerDown",
  "onPointerUp",
  "onMouseDown",
  "onMouseUp",
  "onClick",
  "onTouchStart",
  "onTouchEnd",
] as const;

/** Whether an option is out of reach and says why: it stays in the arrows' way, saying so. */
const saysWhy = (one: SelectOption): boolean =>
  one.isDisabled === true && one.tooltip !== undefined;

/**
 * **An option**: the menu family's row, its tick at its end while it is picked. One out of reach
 * with a reason (`isDisabled` and `tooltip`) is the menu's row that cannot run (D-2906-11): it
 * stays in the arrows' way, in the faint tier, with why on the line under its words, which is
 * its description, and a press or Enter on it picks nothing. One out of reach with no reason is
 * passed over. An option in reach with a hint shows it as its tooltip.
 */
function Option({ one }: { one: SelectOption }): ReactNode {
  const why = useId();
  const unavailable = saysWhy(one);
  return (
    <ListBoxItem
      id={into(one.value)}
      textValue={one.label}
      isDisabled={one.isDisabled === true && !unavailable}
      data-scope="menu"
      data-part="root"
      data-variant="default"
      data-value={one.value}
      data-unavailable={unavailable ? "" : undefined}
      render={
        unavailable
          ? (dom) => {
              const own = Object.fromEntries(
                Object.entries(dom as ComponentPropsWithRef<"div">).filter(
                  ([key]) => !(PRESSES as readonly string[]).includes(key),
                ),
              ) as Record<string, unknown>;
              const keyDown = own.onKeyDown as ((event: KeyboardEvent) => void) | undefined;
              const keyUp = own.onKeyUp as ((event: KeyboardEvent) => void) | undefined;
              return (
                <div
                  {...own}
                  aria-disabled="true"
                  aria-describedby={why}
                  onKeyDown={(event) => {
                    if (event.key !== "Enter" && event.key !== " ") keyDown?.(event);
                  }}
                  onKeyUp={(event) => {
                    if (event.key !== "Enter" && event.key !== " ") keyUp?.(event);
                  }}
                />
              );
            }
          : one.tooltip === undefined
            ? undefined
            : (dom) => (
                <Tooltip tip={one.tooltip}>
                  <div {...(dom as object)} />
                </Tooltip>
              )
      }
    >
      {({ isSelected }) => (
        <>
          <span data-scope="menu" data-part="words">
            <span data-scope="menu" data-part="label">
              {one.label}
            </span>
            {unavailable && (
              // Its description and not its name: heard after the name, as why.
              <span data-scope="menu" data-part="description" id={why} aria-hidden="true">
                {one.tooltip}
              </span>
            )}
          </span>
          <span data-scope="menu" data-part="indicator" data-indicator="check" aria-hidden="true">
            {isSelected && <Check />}
          </span>
        </>
      )}
    </ListBoxItem>
  );
}
