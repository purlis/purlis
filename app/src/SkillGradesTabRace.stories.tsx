// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROBE for #4098, not for merging: SkillGrades › Grades exactly (its args, its play), with every
 * frame drawn late by 0, 50, 150 or 400 ms. Logs each scroll of the pane the Scorecard is in, with
 * what had the keyboard then, and fails when it ends somewhere other than with frames on time.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import gradesMeta, { Grades } from "./SkillGrades.stories";

const meta = {
  ...gradesMeta,
  parameters: { screenshot: false },
} satisfies Meta<typeof gradesMeta.component>;
export default meta;
type Story = StoryObj<typeof meta>;

function lateFrames(ms: number): () => void {
  if (ms === 0) return () => undefined;
  const [ask, cancel] = [window.requestAnimationFrame, window.cancelAnimationFrame];
  const waiting = new Map<number, ReturnType<typeof setTimeout>>();
  let next = 1;
  window.requestAnimationFrame = (callback) => {
    const id = next++;
    waiting.set(
      id,
      setTimeout(() => {
        waiting.delete(id);
        ask(callback);
      }, ms),
    );
    return id;
  };
  window.cancelAnimationFrame = (id) => {
    clearTimeout(waiting.get(id));
    waiting.delete(id);
  };
  return () => {
    window.requestAnimationFrame = ask;
    window.cancelAnimationFrame = cancel;
  };
}

const said = (at: Element | null) =>
  at === null
    ? "none"
    : `${at.getAttribute("role") ?? at.tagName.toLowerCase()}:${(at.getAttribute("aria-label") ?? at.textContent ?? "").trim().slice(0, 30)}`;

/** Where the on-time run ends, per cell, to compare (a run in the same cell, same file). */
const onTime = new Map<string, number>();

function probe(late: number): Story {
  return {
    args: Grades.args,
    play: async (context) => {
      const log: string[] = [];
      const start = performance.now();
      const scrolled = (event: Event) => {
        const pane = event.target as Element;
        if (!(pane instanceof HTMLElement)) return;
        log.push(
          `${Math.round(performance.now() - start)}ms ${pane.getAttribute("data-scope") ?? pane.className.toString().slice(0, 30)} top=${pane.scrollTop} left=${pane.scrollLeft} on=${said(document.activeElement)}`,
        );
      };
      const focused = (event: FocusEvent) =>
        log.push(
          `${Math.round(performance.now() - start)}ms focus ${said(event.target as Element)}`,
        );
      document.addEventListener("scroll", scrolled, true);
      document.addEventListener("focusin", focused);
      const back = lateFrames(late);
      try {
        await Grades.play?.(context as never);
        await new Promise((resolve) => setTimeout(resolve, 600));
      } finally {
        back();
        document.removeEventListener("scroll", scrolled, true);
        document.removeEventListener("focusin", focused);
      }
      const open = document.activeElement as HTMLElement;
      let pane = open.parentElement;
      while (pane !== null && !["auto", "scroll"].includes(getComputedStyle(pane).overflowY))
        pane = pane.parentElement;
      const end = pane?.scrollTop ?? -1;
      const cell = document.documentElement.className + "|" + navigator.userAgent.slice(-20);
      if (late === 0) onTime.set(cell, end);
      const was = onTime.get(cell);
      console.log(`probe-4098 late=${late} end=${end} onTime=${was} :: ${log.join(" ; ")}`);
      if (was !== undefined && end !== was)
        throw new Error(
          `late=${late}: pane ends at ${end}, on time at ${was} :: ${log.join(" ; ")}`,
        );
    },
  };
}

export const OnTime = probe(0);
export const Late50 = probe(50);
export const Late150 = probe(150);
export const Late400 = probe(400);
