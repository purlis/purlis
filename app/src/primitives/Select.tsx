// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The one select** (#1860, spec #1832 D2): Radix Select, its trigger drawn by `.ui-select` and
 * its list by the one menu style (`.ui-menu`, `.ui-menu-item`). Every select in the window is
 * this one: the settings set's `Choice kind="select"`, and the filters of a view.
 *
 * What it adds to Radix is one thing a person never sees: **an empty value is an option like any
 * other** ("Every workspace", "not set"). Radix keeps `""` to mean "nothing picked", so an
 * option's `""` travels as a word of its own and comes back as `""`. A value none of the options
 * has shows `placeholder`, quieter, as nothing picked.
 *
 * The trigger carries what it holds as `data-value`, for the window's e2e specs to read the way
 * they read a field's value.
 */
import * as Radix from "../overlays/select";
import { Check } from "lucide-react";
import { Tooltip } from "../ui/tooltip/Tooltip";

/** An option: what a pick writes, the words it shows, and whether it can be picked. An option
 *  out of reach says why on its `title`, drawn as its tooltip (#2908). */
export type SelectOption = {
  value: string;
  label: string;
  disabled?: boolean;
  title?: string;
};

export type SelectProps = {
  id?: string;
  /** The extra classes of a view or a row, after `ui-select`. */
  className?: string;
  value: string;
  options: readonly SelectOption[];
  onValueChange: (to: string) => void;
  /** What it shows while it holds a value none of the options has. */
  placeholder?: string;
  disabled?: boolean;
  title?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
};

/** The word `""` travels as inside Radix: no value a file or a view writes is this. */
const EMPTY = "\u0000empty";

const into = (value: string) => (value === "" ? EMPTY : value);
const outOf = (value: string) => (value === EMPTY ? "" : value);

export function Select({
  id,
  className,
  value,
  options,
  onValueChange,
  placeholder,
  disabled,
  title,
  ...aria
}: SelectProps) {
  const held = options.some((one) => one.value === value);
  return (
    <Radix.Root
      // `""` is Radix's "nothing picked": the placeholder shows.
      value={held ? into(value) : ""}
      onValueChange={(to) => onValueChange(outOf(to))}
      disabled={disabled}
    >
      <Tooltip tip={title}>
        <Radix.Trigger
          id={id}
          className={className === undefined ? "ui-select" : `ui-select ${className}`}
          // #190: WebKit leaves a control out of the Tab order without `tabIndex`.
          tabIndex={0}
          data-value={held ? value : ""}
          {...aria}
        >
          <Radix.Value placeholder={placeholder} />
        </Radix.Trigger>
      </Tooltip>
      <Radix.Portal>
        <Radix.Content
          className="ui-menu ui-select-list"
          position="popper"
          sideOffset={2}
          onCloseAutoFocus={(event) => {
            // Closed, the keyboard goes back to the select, unless the pick sent it somewhere
            // on purpose: a New… that opens its form takes it to the form's first field.
            const at = document.activeElement;
            if (at !== null && at !== document.body) event.preventDefault();
          }}
        >
          <Radix.Viewport>
            {options.map((one, at) => (
              // By place, not by value, as every option list of the set is keyed.
              <Tooltip key={at} tip={one.title}>
                <Radix.Item
                  className="ui-menu-item"
                  value={into(one.value)}
                  disabled={one.disabled}
                  data-value={one.value}
                >
                  <Radix.ItemText>{one.label}</Radix.ItemText>
                  <Radix.ItemIndicator className="ui-menu-mark">
                    <Check className="node-icon" aria-hidden />
                  </Radix.ItemIndicator>
                </Radix.Item>
              </Tooltip>
            ))}
          </Radix.Viewport>
        </Radix.Content>
      </Radix.Portal>
    </Radix.Root>
  );
}
