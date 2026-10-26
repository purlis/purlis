// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A colour well** (#2907): any colour, picked in the system's own colour picker. It is the
 * native `<input type="color">`, drawn here once, so a screen never writes a raw one
 * (`purlis/choice-family`, an error). The platform draws the well and its picker, so it has no
 * parts in the manifest; the family places it and gives it the window's focus ring.
 *
 * **What it hands back is the colour the picker settled on** (`onCommit`), once: the picker's
 * `change`, or the well left with a colour moved to and not yet handed back. While the picker
 * moves, `onDraft` hears each colour it passes.
 */
import { useRef, type Ref } from "react";

export interface ColorWellProps {
  id?: string;
  /** The colour shown: `#rrggbb`. */
  value: string;
  /** Each colour the picker passes while it is open. */
  onDraft?: (to: string) => void;
  /** The colour the picker settled on, or the one shown when the well is left. */
  onCommit: (to: string) => void;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  isDisabled?: boolean;
  /** A place for it in its view. Never a look. */
  className?: string;
  ref?: Ref<HTMLInputElement>;
}

export function ColorWell({ value, onDraft, onCommit, isDisabled, ref, ...rest }: ColorWellProps) {
  // A colour the picker moved to that has not been handed back yet.
  const moved = useRef(false);
  // The colour last handed back: the picker's `change` that settled on it is heard by React as
  // a change too, and moves nothing.
  const settled = useRef<string | undefined>(undefined);
  const settle = (to: string) => {
    moved.current = false;
    settled.current = to;
    onCommit(to);
  };
  return (
    <input
      {...rest}
      type="color"
      // #190: WebKit leaves an input out of the Tab order without `tabIndex`.
      tabIndex={0}
      disabled={isDisabled}
      value={value}
      data-scope="color-well"
      onChange={(event) => {
        if (event.currentTarget.value === settled.current) return;
        moved.current = true;
        onDraft?.(event.currentTarget.value);
      }}
      ref={(element) => {
        // React's `onChange` is the `input` event; the picker settling is the native `change`.
        if (element) element.onchange = () => settle(element.value);
        if (typeof ref === "function") ref(element);
        else if (ref) ref.current = element;
      }}
      onBlur={(event) => {
        if (moved.current) settle(event.currentTarget.value);
      }}
    />
  );
}
