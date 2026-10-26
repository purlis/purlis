// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The slider family** (#2907, spec #2897, ADR 0104): a number picked along a range, on React
 * Aria's `Slider`, `SliderTrack`, `SliderThumb` and `SliderOutput`, wrapped here once. A screen
 * imports it and never `react-aria-components`, and draws no `<input type="range">` of its own
 * (`purlis/choice-family`, an error).
 *
 * - **A native range under its thumb**: the arrow keys, Page Up and Down, Home and End move it,
 *   a press on the track jumps to it, and a drag follows the pointer. A screen reader hears a
 *   slider named by `label` (React Aria's `Label`) whose value reads as `spoken` says (`18
 *   pixels`), heard once as it moves; the value is drawn beside the track as `shown` says
 *   (`18px`), and is not announced again.
 * - **Drawn by its parts.** The root carries `data-scope="slider"` and `data-part="root"`; inside
 *   it the `track` (the line), the `fill` along it up to the value, the `thumb` and the
 *   `output`. A value never moves
 *   anything but the thumb and the fill: the output keeps the room of its widest value.
 * - **The id goes on the root, and hands the focus to the thumb**, so a row that gives its
 *   control the focus back after a Reset, or whose name is pressed, finds the slider. React Aria's slider is the group
 *   inside the root, laid out as the root.
 */
import { useLayoutEffect, useRef, type CSSProperties } from "react";
import {
  Label,
  Slider as AriaSlider,
  SliderOutput,
  SliderThumb,
  SliderTrack,
} from "react-aria-components";

export interface SliderProps {
  value: number;
  onChange: (to: number) => void;
  minValue: number;
  maxValue: number;
  step?: number;
  /** What the value reads as beside the slider: `18px`. */
  shown: (value: number) => string;
  /** What a screen reader says it is, when that is not `shown`: `18 pixels`. */
  spoken?: (value: number) => string;
  /** Its name: the words of a settings row's label, or its own where no row names it. React
   *  Aria's label for the slider, which a screen reader hears it by; the row draws the words. */
  label: string;
  "aria-describedby"?: string;
  /** The root's id: a focus given to it goes to the thumb. */
  id?: string;
  isDisabled?: boolean;
  /** A place for it in its view: margin, width, order. Never a look. */
  className?: string;
  style?: CSSProperties;
}

/** **A slider**: its track, its fill, its thumb and its value beside them. */
export function Slider({
  value,
  onChange,
  minValue,
  maxValue,
  step = 1,
  shown,
  spoken,
  id,
  isDisabled,
  className,
  style,
  label,
  ...named
}: SliderProps) {
  const input = useRef<HTMLInputElement>(null);
  const said = (spoken ?? shown)(value);
  // React Aria reads a thumb's value as a formatted number; what a screen reader hears is said
  // here instead, once React Aria has written its own.
  useLayoutEffect(() => {
    input.current?.setAttribute("aria-valuetext", said);
  });
  const widest = widestOf(minValue, maxValue, step, shown);
  return (
    // The root holds the id and hands a focus given to it to the thumb; React Aria's slider, the
    // group inside it, lays out as the root does.
    <span
      id={id}
      tabIndex={-1}
      onFocus={(event) => {
        if (event.target === event.currentTarget) input.current?.focus();
      }}
      className={className}
      style={style}
      data-scope="slider"
      data-part="root"
      data-disabled={isDisabled ? "" : undefined}
    >
      <AriaSlider
        value={value}
        onChange={(to) => onChange(Array.isArray(to) ? to[0] : to)}
        minValue={minValue}
        maxValue={maxValue}
        step={step}
        isDisabled={isDisabled}
        data-scope="slider"
        data-part="group"
      >
        {/* Its name, heard and not drawn: the row it stands in draws the words. */}
        <Label data-scope="slider" data-part="name" style={HIDDEN}>
          {label}
        </Label>
        {/* The area a press lands in is as tall as the thumb; the track drawn is the line
          through its middle. */}
        <SliderTrack data-scope="slider" data-part="area">
          {({ state }) => (
            <>
              <span data-scope="slider" data-part="track">
                <span
                  data-scope="slider"
                  data-part="fill"
                  style={{ inlineSize: `${state.getThumbPercent(0) * 100}%` }}
                />
              </span>
              <SliderThumb
                inputRef={input}
                aria-describedby={named["aria-describedby"]}
                data-scope="slider"
                data-part="thumb"
              />
            </>
          )}
        </SliderTrack>
        <SliderOutput data-scope="slider" data-part="output">
          {/* The widest value keeps its room, so nothing moves as the value grows. */}
          <span data-scope="slider" data-part="room" aria-hidden="true">
            {widest}
          </span>
          <span data-scope="slider" data-part="value">
            {shown(value)}
          </span>
        </SliderOutput>
      </AriaSlider>
    </span>
  );
}

/** Drawn for a screen reader alone (the usual visually-hidden box). */
const HIDDEN: CSSProperties = {
  position: "absolute",
  inlineSize: 1,
  blockSize: 1,
  overflow: "hidden",
  clipPath: "inset(50%)",
  whiteSpace: "nowrap",
};

/** The most values a slider's room is measured over; beyond it, its ends and a sample. */
const MOST_MEASURED = 512;

/**
 * **The widest value a slider can show**, by the length of each value as `shown` draws it, so
 * its room is kept for the longest and nothing moves as the value changes.
 */
function widestOf(min: number, max: number, step: number, shown: (value: number) => string) {
  const count = Math.max(1, Math.floor((max - min) / step) + 1);
  const every = Math.max(1, Math.ceil(count / MOST_MEASURED));
  let widest = shown(min);
  for (let at = 0; at < count; at += every) {
    const one = shown(min + at * step);
    if (one.length > widest.length) widest = one;
  }
  const last = shown(max);
  return last.length > widest.length ? last : widest;
}
