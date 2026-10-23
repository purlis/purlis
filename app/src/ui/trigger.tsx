// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What opens a menu or a popover** (#2909): the button family's own buttons press themselves
 * (React Aria's `Button`), and any other element a family has not moved yet (a list's row, a
 * strip's mark) is made pressable by React Aria's `Pressable`, so a press, a key and a screen
 * reader open it alike. Shared by the menu and popover families.
 */
import { isValidElement, type DOMAttributes, type ReactElement, type ReactNode } from "react";
import { Pressable } from "react-aria-components";
import { Button, IconButton, ToggleButton } from "./button/Button";

const PRESSES_ITSELF = new Set<unknown>([Button, IconButton, ToggleButton]);

/** `trigger` as React Aria's trigger: as it is when it is a button of the family, else made
 *  pressable by `Pressable`. */
export function pressableTrigger(trigger: ReactNode): ReactNode {
  if (!isValidElement(trigger) || PRESSES_ITSELF.has(trigger.type)) return trigger;
  return <Pressable>{trigger as ReactElement<DOMAttributes<Element>, string>}</Pressable>;
}
