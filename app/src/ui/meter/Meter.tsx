// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The meter family** (#3674, spec #2897 as the operator amended it on 2026-10-28, ADR 0104): a
 * value drawn as a bar, on React Aria's `Meter` and `ProgressBar`, wrapped here once. A screen
 * imports these and never writes `role="meter"` or `role="progressbar"`, `<meter>` or
 * `<progress>` (`purlis/meter-family`).
 *
 * - **`Meter`**: a labelled value on a known range, a grade's score from 0 to 100. Its label at
 *   the start, its track, and its value in figures of one width at the end. Its `variant` says
 *   what it is in a comparison: `subject` (the skill with it, the accent), `baseline` (without it,
 *   the warning colour) and `previous` (its previous version, the muted tier).
 *   Each is told apart by its label and its value beside it as well as its colour (WCAG 1.4.1), and
 *   each fill is held at 3:1 on its track by the theme engine, in every theme.
 * - **`MeterGroup`**: meters that compare, sharing one scale and one label column, so their
 *   lengths compare: the Grades tab's latest grade, with it, without it and its previous version.
 * - **`ProgressBar`**: how far something is (a running grade's chats done of N), or that it is
 *   going with no end known (`isIndeterminate`). Its value is said politely as it changes, at
 *   most once a second (`ui/said.ts`); an indeterminate bar's stripes move, and stand still
 *   under reduced motion, where they still say it is going.
 *
 * **Drawn by its parts** (ADR 0103): the `root` (its `data-variant`), its `label`, its `track`,
 * the `fill` on it and its `value`. `meter.css` draws the family default and a theme's parts
 * reach the same attributes; forced colours draw the fill in the system's highlight on a track
 * edged in the words' colour (`forced-colors.css`).
 */
import { createContext, useContext, type ReactNode } from "react";
import {
  Label,
  Meter as AriaMeter,
  ProgressBar as AriaProgressBar,
  type MeterProps as AriaMeterProps,
  type ProgressBarProps as AriaProgressBarProps,
} from "react-aria-components";
import { VisuallyHidden } from "react-aria";
import { useSaidAtMost } from "../said";

/** The family's variants, in the order the manifest names them. */
export const METER_VARIANTS = ["subject", "baseline", "previous"] as const;
export type MeterVariant = (typeof METER_VARIANTS)[number];

/** **The scale a group's meters share**: so their lengths compare. */
interface Scale {
  minValue: number;
  maxValue: number;
}

const Shared = createContext<Scale | null>(null);

/** What every bar of the family takes beside its value. */
interface Bar {
  /** Its name: drawn at its start, and what a reader hears it by. */
  label: ReactNode;
  /** Its value as words, drawn at its end ("86%", "3 of 12"); the percentage where not said. */
  valueLabel?: string;
  /** A place for it in its view: a margin, a width. Never a look. */
  className?: string;
}

/** The track and the fill on it, `percentage` of it long. */
function Track({ percentage }: { percentage: number | undefined }) {
  return (
    <span data-scope="meter" data-part="track" aria-hidden="true">
      <span
        data-scope="meter"
        data-part="fill"
        style={percentage === undefined ? undefined : { inlineSize: `${percentage}%` }}
      />
    </span>
  );
}

export interface MeterProps
  extends Bar, Omit<AriaMeterProps, "children" | "className" | "style" | "render" | "valueLabel"> {
  /** What it is in a comparison: the subject, the baseline, the previous version. */
  variant?: MeterVariant;
}

/** **A labelled value on a known range**: its label, its track and its value in figures. */
export function Meter({
  label,
  valueLabel,
  variant = "subject",
  className,
  minValue,
  maxValue,
  ...rest
}: MeterProps) {
  const scale = useContext(Shared);
  return (
    <AriaMeter
      {...rest}
      minValue={minValue ?? scale?.minValue}
      maxValue={maxValue ?? scale?.maxValue}
      valueLabel={valueLabel}
      className={className}
      data-scope="meter"
      data-part="root"
      data-variant={variant}
      data-grouped={scale === null ? undefined : ""}
    >
      {({ percentage, valueText }) => (
        <>
          <Label data-scope="meter" data-part="label">
            {label}
          </Label>
          <Track percentage={percentage} />
          <span data-scope="meter" data-part="value" aria-hidden="true">
            {valueText}
          </span>
        </>
      )}
    </AriaMeter>
  );
}

/**
 * **Meters that compare**: one scale for all of them (0 to 100 unless said), and one label
 * column, so a longer bar is a larger value. Named for a reader ("Latest grade").
 */
export function MeterGroup({
  children,
  minValue = 0,
  maxValue = 100,
  className,
  "aria-label": label,
  "aria-labelledby": labelledBy,
}: {
  children: ReactNode;
  minValue?: number;
  maxValue?: number;
  className?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
}) {
  return (
    <Shared.Provider value={{ minValue, maxValue }}>
      <div
        role="group"
        aria-label={label}
        aria-labelledby={labelledBy}
        className={className}
        data-scope="meter"
        data-part="group"
      >
        {children}
      </div>
    </Shared.Provider>
  );
}

export interface ProgressBarProps
  extends
    Bar,
    Omit<AriaProgressBarProps, "children" | "className" | "style" | "render" | "valueLabel"> {}

/**
 * **How far something is**: done of N, or going with no end known (`isIndeterminate`). Its value
 * is said politely as it changes, at most once a second.
 */
export function ProgressBar({
  label,
  valueLabel,
  className,
  isIndeterminate = false,
  value = 0,
  minValue = 0,
  maxValue = 100,
  ...rest
}: ProgressBarProps) {
  // What it says as it changes, beside the bar: a progress bar's own children are presentational
  // to a reader, so a live region inside it would never be heard.
  const share = maxValue > minValue ? (value - minValue) / (maxValue - minValue) : 0;
  const words = valueLabel ?? `${Math.round(Math.min(1, Math.max(0, share)) * 100)}%`;
  return (
    <>
      <AriaProgressBar
        {...rest}
        value={value}
        minValue={minValue}
        maxValue={maxValue}
        isIndeterminate={isIndeterminate}
        valueLabel={valueLabel}
        className={className}
        data-scope="meter"
        data-part="root"
        data-variant="subject"
        data-progress=""
        data-indeterminate={isIndeterminate || undefined}
      >
        {({ percentage, valueText }) => (
          <>
            <Label data-scope="meter" data-part="label">
              {label}
            </Label>
            <Track percentage={isIndeterminate ? undefined : percentage} />
            <span data-scope="meter" data-part="value" aria-hidden="true">
              {valueText}
            </span>
          </>
        )}
      </AriaProgressBar>
      <Said
        words={isIndeterminate ? "" : typeof label === "string" ? `${label} ${words}` : words}
      />
    </>
  );
}

/** A progress bar's value, said politely as it changes, at most once a second. */
function Said({ words }: { words: string }) {
  const spoken = useSaidAtMost(words);
  return (
    <VisuallyHidden>
      <span role="status">{spoken}</span>
    </VisuallyHidden>
  );
}
