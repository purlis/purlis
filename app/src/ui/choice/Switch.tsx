// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The switch family** (#2907, spec #2897, ADR 0104): a setting on or off, in force the moment it
 * is turned, on React Aria's `Switch`, wrapped here once. A screen imports it and never
 * `react-aria-components`, and draws no `role="switch"` of its own (`purlis/choice-family`).
 *
 * - **A native switch in its own label**: a press of its track or its words turns it, Space turns
 *   it from the keyboard, and a screen reader hears a switch named by its words, or by
 *   `aria-label` / `aria-labelledby` where a row draws its name. The id goes on the switch itself,
 *   so a row's `<label for>` names it.
 * - **Drawn by its parts** (R63): the root (the label) carries `data-scope="switch"` and
 *   `data-part="root"`; inside it the `track`, a 34 × 20 pill, its 14 px `thumb`, and the `label`.
 *   On, the track takes the accent's fill and the thumb the window's colour, and the thumb slides
 *   across on the knob's motion token (`quick`, 150 ms in Purlis); it never slides under reduced
 *   motion. Its size is R63's at the person's text size, never a theme's control height. On is told by the thumb's place as
 *   well as the fill, never by colour alone.
 * - **Turning it never moves anything** (#2800, R153).
 * - **Its hint is the shared tooltip** (`tooltip`, #2908).
 */
import { type CSSProperties, type ReactNode, type Ref } from "react";
import { Switch as AriaSwitch, type SwitchProps as AriaSwitchProps } from "react-aria-components";
import { Tooltip, type Tip } from "../tooltip/Tooltip";

export interface SwitchProps extends Omit<
  AriaSwitchProps,
  "className" | "style" | "children" | "render"
> {
  /** Its words, the `label` part. Left out, `aria-label` or `aria-labelledby` names it. */
  children?: ReactNode;
  /** A place for it in its view: margin, width, order. Never a look. */
  className?: string;
  style?: CSSProperties;
  /** Its hint, the shared tooltip (#2908). */
  tooltip?: Tip;
  ref?: Ref<HTMLLabelElement>;
}

/** **A switch**: on or off, in force as it is turned. */
export function Switch({ children, tooltip, ...rest }: SwitchProps) {
  const drawn = (
    <AriaSwitch {...rest} data-scope="switch" data-part="root">
      <span data-scope="switch" data-part="track" aria-hidden="true">
        <span data-scope="switch" data-part="thumb" />
      </span>
      {children !== undefined && children !== null && children !== false ? (
        <span data-scope="switch" data-part="label">
          {children}
        </span>
      ) : null}
    </AriaSwitch>
  );
  return tooltip === undefined || tooltip === null ? (
    drawn
  ) : (
    <Tooltip tip={tooltip}>{drawn}</Tooltip>
  );
}
