// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The steps family** (#4480, spec #4469, ADR 0104): a guide's list of steps, each with its state,
 * on React Aria's `Button`, wrapped here once. A screen imports {@link StepList} and never writes
 * `data-scope="steps"` (`purlis/steps-family`).
 *
 * A step's state is told by its mark and its words, never by colour alone (WCAG 1.4.1). A step not
 * reached yet is shown with its number and can't be pressed: a person jumps back, never ahead.
 *
 * **Drawn by its parts** (ADR 0103): the `root`, the `count` of steps done, each `item` (its
 * `data-variant` is its state), the `press` it is pressed by, its `mark`, its `name` and its `line`.
 * `steps.css` draws the family default and a theme's parts reach the same attributes.
 */
import type { ReactNode } from "react";
import { VisuallyHidden } from "react-aria";
import { Button as AriaButton } from "react-aria-components";
import { AlertTriangle, Check, Clock, Minus } from "lucide-react";

/** A step's states, in the order the manifest names them. */
export const STEP_STATES = ["done", "current", "attention", "skipped", "waiting", "later"] as const;
export type StepState = (typeof STEP_STATES)[number];

/** What a reader hears of a state beside the step's name. */
const SAID: Record<StepState, string> = {
  done: "Done",
  current: "You are here",
  attention: "Needs attention",
  skipped: "Skipped",
  waiting: "Waiting on someone",
  later: "Not reached yet",
};

/** The mark of a state; a number where it has none. */
const MARK: Partial<Record<StepState, ReactNode>> = {
  done: <Check aria-hidden size={14} strokeWidth={3} />,
  attention: <AlertTriangle aria-hidden size={14} />,
  skipped: <Minus aria-hidden size={14} strokeWidth={3} />,
  waiting: <Clock aria-hidden size={14} />,
};

export interface Step {
  id: string;
  name: string;
  state: StepState;
  /** A short line under its name: what it settled on, or what it needs. */
  line?: string | null;
}

export interface StepListProps {
  /** Its name to a reader: "Steps of Pool setup". */
  "aria-label": string;
  steps: readonly Step[];
  /** The step on screen, which may be one before where the person has reached. */
  showing: string | null;
  onGo: (step: string) => void;
  /** A place for it in its view: a width, a margin. Never a look. */
  className?: string;
  /** What is said under the list. */
  children?: ReactNode;
}

/** **A guide's steps**: how far along the person is, and any step reached to jump back to. */
export function StepList({
  "aria-label": label,
  steps,
  showing,
  onGo,
  className,
  children,
}: StepListProps) {
  const done = steps.filter((step) => step.state === "done").length;
  return (
    <nav aria-label={label} className={className} data-scope="steps" data-part="root">
      <p data-scope="steps" data-part="count">
        {done} of {steps.length} done
      </p>
      <ol data-scope="steps" data-part="list">
        {steps.map((step, at) => (
          <li
            key={step.id}
            data-scope="steps"
            data-part="item"
            data-variant={step.state}
            data-here={step.id === showing ? "" : undefined}
          >
            <AriaButton
              data-scope="steps"
              data-part="press"
              isDisabled={step.state === "later"}
              aria-current={step.id === showing ? "step" : undefined}
              onPress={() => onGo(step.id)}
            >
              <span data-scope="steps" data-part="mark">
                {MARK[step.state] ?? at + 1}
              </span>
              <span data-scope="steps" data-part="words">
                <span data-scope="steps" data-part="name">
                  {step.name}
                </span>
                {step.line && (
                  <span data-scope="steps" data-part="line">
                    {step.line}
                  </span>
                )}
                <VisuallyHidden>, {SAID[step.state]}</VisuallyHidden>
              </span>
            </AriaButton>
          </li>
        ))}
      </ol>
      {children}
    </nav>
  );
}
