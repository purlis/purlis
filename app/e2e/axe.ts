// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { AxeBuilder } from "@axe-core/webdriverio";
import { browser } from "@wdio/globals";

/**
 * **axe over the real window** (#2929, spec #2897's third seam): `@axe-core/webdriverio`, run in
 * the built app's own WebView, WKWebView on macOS and WebKitGTK on Linux.
 *
 * The stories hold each component to axe in Chromium and WebKit (`.storybook/preview.tsx`); what
 * only the window can show is the components together: the landmarks they make between them, a
 * name two regions share, a colour drawn over a ground no story draws, a dialog over the shell.
 *
 * **Legacy mode, on purpose.** The adapter's default finishes its run in a new browser window it
 * opens for the purpose (`createWindow`), and the app's embedded WebDriver has no second window to
 * open: legacy mode runs `axe.run` in the page itself, which is what the stories' runner does too.
 */

/** The axe tags of WCAG 2.2 level AA: every A and AA rule of 2.0, 2.1 and 2.2, as the stories
 *  hold them (`.storybook/preview.tsx`). */
export const WCAG_22_AA = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

/**
 * **The landmark and heading rules** (#2929): axe's own, tagged best practice rather than WCAG, so
 * the WCAG run above leaves them out. They are how a screen reader's rotor finds its way round
 * the window: one main, a banner and the rest at the top level, every landmark of a kind named
 * apart, and headings that never skip a level.
 */
export const STRUCTURE_RULES = [
  "landmark-one-main",
  "landmark-no-duplicate-main",
  "landmark-main-is-top-level",
  "landmark-no-duplicate-banner",
  "landmark-banner-is-top-level",
  "landmark-no-duplicate-contentinfo",
  "landmark-contentinfo-is-top-level",
  "landmark-complementary-is-top-level",
  "landmark-unique",
  "heading-order",
  "empty-heading",
];

/** One violation as a failure says it: the rule, how bad, what it asks, and where. */
export type Found = { rule: string; impact: string; help: string; where: string[] };

type Violation = {
  id: string;
  impact?: string | null;
  help: string;
  nodes: { target: unknown[]; html?: string; failureSummary?: string }[];
};

/** What axe found, each with the first few places it found it, for a failure to read. */
export function foundOf(violations: readonly Violation[]): Found[] {
  return violations.map((one) => ({
    rule: one.id,
    impact: one.impact ?? "unknown",
    help: one.help,
    where: one.nodes.slice(0, 5).map((node) => {
      const target = node.target.map((part) => JSON.stringify(part)).join(" >>> ");
      const why = (node.failureSummary ?? "").replace(/\s+/g, " ").trim();
      // The element itself, opening tag and words, so a failure says what was drawn there.
      const html = (node.html ?? "").replace(/\s+/g, " ").trim().slice(0, 200);
      return [target, html, why].filter(Boolean).join(" | ");
    }),
  }));
}

/** Runs axe over the whole window as it is drawn now, held to WCAG 2.2 AA with `target-size`
 *  on, as the stories are (#2897). */
export async function wcagFound(): Promise<Found[]> {
  // One options object: the builder's `options()` replaces whatever `withTags()` set, so the two
  // together ran every rule, best practice too (the first run of #6037).
  const results = await new AxeBuilder({ client: browser })
    .setLegacyMode(true)
    .options({
      runOnly: { type: "tag", values: WCAG_22_AA },
      rules: { "target-size": { enabled: true } },
    })
    .analyze();
  return foundOf(results.violations as Violation[]);
}

/** Runs axe's landmark and heading rules alone over the whole window ({@link STRUCTURE_RULES}). */
export async function structureFound(): Promise<Found[]> {
  const results = await new AxeBuilder({ client: browser })
    .setLegacyMode(true)
    .withRules(STRUCTURE_RULES)
    .analyze();
  return foundOf(results.violations as Violation[]);
}
