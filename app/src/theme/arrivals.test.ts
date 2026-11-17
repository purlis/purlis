// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vite/client" />
/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { ARRIVALS_ATTRIBUTE, arriveAtOnceInScenarioTests } from "./motion";

/**
 * **An overlay arrives at once in the scenario-test build** (#4351). WebKit suspends a CSS
 * animation while its window is hidden, and React Aria takes an overlay away only once its
 * animations have finished: an overlay that fades in as it enters (`[data-entering]`) and is
 * not drawn at once under `:root[data-arrivals="at-once"]` stays at opacity 0, and stays up
 * once closed, for as long as a scenario run's window is not in front.
 */

/** Every stylesheet of the window, by its path from `app`, read off disk (`literals.test.ts`
 *  says why the glob names them and Node reads them). */
const SHEETS: [string, string][] = Object.keys(import.meta.glob("../**/*.css")).map((path) => {
  // Vite hands this directory's own sheets back as `./x.css`.
  const from = path.startsWith("../") ? `src/${path.slice(3)}` : `src/theme/${path.slice(2)}`;
  return [from, readFileSync(join(process.cwd(), from), "utf8")];
});

/** A sheet's rules as selector and body, comments out. Flat rules are all this reads, which is
 *  what the families write an overlay's entering in. */
function rules(text: string): { selector: string; body: string }[] {
  const plain = text.replace(/\/\*[\s\S]*?\*\//g, " ");
  return [...plain.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((hit) => ({
    selector: hit[1].trim().replace(/\s+/g, " "),
    body: hit[2],
  }));
}

const AT_ONCE = `:root[${ARRIVALS_ATTRIBUTE}="at-once"]`;

describe("an overlay in the scenario-test build (#4351)", () => {
  it("reads the window's stylesheets", () => {
    expect(SHEETS.length).toBeGreaterThan(0);
    expect(SHEETS.every(([, text]) => text.length > 0)).toBe(true);
  });

  it("is drawn at once wherever it would fade in as it enters", () => {
    const missing: string[] = [];
    let entering = 0;
    for (const [path, text] of SHEETS) {
      const all = rules(text);
      for (const { selector, body } of all) {
        if (!selector.includes("[data-entering]") || selector.startsWith(AT_ONCE)) continue;
        if (!/\banimation(-name)?\s*:/.test(body)) continue;
        entering += 1;
        const twin = all.find(
          (rule) =>
            rule.selector === `${AT_ONCE} ${selector}` &&
            /\banimation\s*:\s*none\b/.test(rule.body),
        );
        if (twin === undefined) missing.push(`${path}: ${selector}`);
      }
    }
    // The dialog's scrim and the popover, at least: a guard that found none checked nothing.
    expect(entering).toBeGreaterThanOrEqual(2);
    expect(missing).toEqual([]);
  });

  it("slides no workspace in and swaps no side view's list in, which hid every region", () => {
    const all = SHEETS.flatMap(([, text]) => rules(text));
    const arrivals = [
      '.regions[data-slide="on"]',
      '.regions[data-slide="back"]',
      ".region-view[data-swapped]",
    ];
    const missing = arrivals.filter((selector) => {
      // The arrival is still drawn with an animation, or this guards a selector gone.
      expect(
        all.some((rule) => rule.selector === selector && /\banimation\s*:/.test(rule.body)),
      ).toBe(true);
      return !all.some(
        (rule) =>
          rule.selector.split(",").some((one) => one.trim() === `${AT_ONCE} ${selector}`) &&
          /\banimation\s*:\s*none\b/.test(rule.body),
      );
    });
    expect(missing).toEqual([]);
  });

  it("is marked on the root in the scenario-test build, and in no other", () => {
    const scenario = document.createElement("html");
    arriveAtOnceInScenarioTests(scenario, true);
    expect(scenario.getAttribute(ARRIVALS_ATTRIBUTE)).toBe("at-once");

    const shipped = document.createElement("html");
    arriveAtOnceInScenarioTests(shipped, false);
    expect(shipped.hasAttribute(ARRIVALS_ATTRIBUTE)).toBe(false);
  });
});
