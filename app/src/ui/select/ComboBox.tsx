// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The combobox family** (#2906, spec #2897, ADR 0104): every box a person types in with a list
 * of what matches under it, wrapped here once. A screen writes no `role="combobox"` and no
 * `<datalist>` of its own (`purlis/select-family`, an error).
 *
 * - **`ComboBox`**, on React Aria's `ComboBox`, `Input`, `Button`, `ListBox` and `ListBoxItem`: a
 *   box with suggestions in a list it opens under it, which narrows as the person types. What is
 *   typed is the value; a suggestion picked writes its words. Its box is the family's root,
 *   `data-scope="combobox" data-part="root"`, drawn as the text input's, with the drawn
 *   `chevron` at its end (quiet, R64) that opens every suggestion. Its list stands on the popover
 *   family's `menu` surface and each suggestion is the menu family's row, as a select's options.
 * - **`ListComboBox`**: the box over a list drawn in place under it, always shown, whose rows and
 *   whose aim are its owner's (the command palette). The box is the field family's text input
 *   with the combobox's ARIA, which only this family writes: its role, that its list is open,
 *   which list it drives, and which row it is on.
 * - **Sizes**: `sm` 24, `md` 28 (the default), `lg` 32, as the text input's.
 * - **Keyboard first**: ↓ opens the suggestions, ↑ and ↓ move and stop at the ends, Enter picks,
 *   Escape closes the list and then clears nothing.
 */
import type { ReactNode } from "react";
import {
  Button as AriaButton,
  ComboBox as AriaComboBox,
  Group,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  useFilter,
} from "react-aria-components";
import { Check, ChevronDown } from "lucide-react";
import { TextField, type PlainTextFieldProps } from "../field/Field";
import { Surface } from "../popover/Popover";
import { Tooltip } from "../tooltip/Tooltip";
import type { SelectSize } from "./Select";

export type ComboBoxProps = {
  /** Its visible name, over the box. Without one, `aria-label` names it. */
  label?: ReactNode;
  "aria-label"?: string;
  /** The words in the box, as typed or as a suggestion picked wrote them. */
  value: string;
  onChange: (value: string) => void;
  /** What it suggests, in order: the ones whose words hold what is typed are shown. */
  suggestions: readonly string[];
  placeholder?: string;
  isDisabled?: boolean;
  /** Its hint, the shared tooltip (#2908), on the box. */
  tooltip?: string;
  size?: SelectSize;
  /** The box's id. */
  id?: string;
  /** Its place in its view: margin, width, flex, order. Never a look. */
  className?: string;
  /** `data-*` attributes reach the element typed in as they are: a test's id. */
  [data: `data-${string}`]: string | number | boolean | undefined;
};

/** **A combobox**: the box, its chevron, and the suggestions that match in a list under it. */
export function ComboBox({
  label,
  value,
  onChange,
  suggestions,
  placeholder,
  isDisabled,
  tooltip,
  size = "md",
  id,
  className,
  ...rest
}: ComboBoxProps) {
  const { contains } = useFilter({ sensitivity: "base" });
  const data: Record<string, unknown> = {};
  const aria: Record<string, unknown> = {};
  for (const [key, one] of Object.entries(rest)) (key.startsWith("data-") ? data : aria)[key] = one;
  const items = [...new Set(suggestions)].map((name) => ({ id: name, name }));
  return (
    <AriaComboBox
      {...aria}
      allowsCustomValue
      menuTrigger="input"
      defaultFilter={contains}
      defaultItems={items}
      inputValue={value}
      onInputChange={onChange}
      isDisabled={isDisabled}
      className={className}
      data-scope="field"
      data-part="root"
    >
      {({ isOpen }) => (
        <>
          {label !== undefined && (
            <Label data-scope="field" data-part="label">
              {label}
            </Label>
          )}
          <Group
            data-scope="combobox"
            data-part="root"
            data-size={size}
            data-open={isOpen ? "" : undefined}
          >
            <Tooltip tip={tooltip}>
              <Input {...data} id={id} placeholder={placeholder} autoComplete="off" />
            </Tooltip>
            <AriaButton data-scope="combobox" data-part="chevron">
              <ChevronDown aria-hidden="true" />
            </AriaButton>
          </Group>
          <Surface kind="menu" data-of="select">
            <ListBox<{ id: string; name: string }> data-scope="popover" data-part="list">
              {(one) => (
                <ListBoxItem
                  id={one.id}
                  textValue={one.name}
                  data-scope="menu"
                  data-part="root"
                  data-variant="default"
                >
                  {({ isSelected }) => (
                    <>
                      <span data-scope="menu" data-part="words">
                        <span data-scope="menu" data-part="label">
                          {one.name}
                        </span>
                      </span>
                      <span
                        data-scope="menu"
                        data-part="indicator"
                        data-indicator="check"
                        aria-hidden="true"
                      >
                        {isSelected && <Check />}
                      </span>
                    </>
                  )}
                </ListBoxItem>
              )}
            </ListBox>
          </Surface>
        </>
      )}
    </AriaComboBox>
  );
}

/** What `ListComboBox` takes: a text field's props, and the lists it drives. */
export type ListComboBoxProps = Omit<
  PlainTextFieldProps,
  "role" | "aria-expanded" | "aria-controls" | "aria-activedescendant" | "aria-autocomplete"
> & {
  /** The ids of the lists under it it drives, in order; none while it has no row to show. */
  controls: readonly string[];
  /** The id of the row its arrows are on, which a screen reader is told of as it moves. */
  aimed?: string;
};

/**
 * **A box over a list drawn in place under it** (the command palette): the field family's text
 * input with the combobox's ARIA. Its owner keeps its rows and its keys: the arrows move the aim,
 * Enter runs the aimed row, and the box keeps the keyboard throughout.
 */
export function ListComboBox({ controls, aimed, ...field }: ListComboBoxProps) {
  const driven = controls.filter((one) => one !== "");
  return (
    <TextField
      {...field}
      role="combobox"
      aria-autocomplete="list"
      aria-expanded={driven.length > 0 ? "true" : "false"}
      aria-controls={driven.length > 0 ? driven.join(" ") : undefined}
      aria-activedescendant={aimed}
    />
  );
}
