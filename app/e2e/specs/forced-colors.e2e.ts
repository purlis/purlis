// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $$, browser, expect } from "@wdio/globals";

/**
 * **Forced colours, against the real app** (#2328, `src/theme/forced-colors.css`).
 *
 * `forced-colors.test.ts` holds what the sheet says, and `tailwind.test.ts` that Tailwind's
 * compiler keeps it in the last layer. What neither can hold is that it **survives the Vite build
 * into a WebView** and that the WebView reads it: jsdom evaluates no media query. A runner cannot
 * turn a contrast theme on, so this reads the window's own style sheets as the engine parsed
 * them: the forced-colours block is there, its media query is one the engine knows, and its
 * rules draw focus, the selected tab and the state marks in the system's colours.
 */

type Found = {
  known: boolean;
  active: boolean;
  rules: { selector: string; style: Record<string, string> }[];
};

async function untilTheStripIsRead(): Promise<void> {
  await browser.waitUntil(
    async () => (await $$('[data-strip="Workspaces"] [role="tab"]').getElements()).length > 0,
    { timeout: 30_000, interval: 250, timeoutMsg: "the strip never listed the fixture plane" },
  );
}

describe("forced colours", () => {
  it("are in the built window, in a media query the WebView reads, in the system's colours", async () => {
    await untilTheStripIsRead();
    const found: Found = await browser.execute(() => {
      const rules: { selector: string; style: Record<string, string> }[] = [];
      const read = (list: CSSRuleList, forced: boolean) => {
        for (const rule of [...list]) {
          if (rule instanceof CSSMediaRule) {
            read(rule.cssRules, forced || /forced-colors:\s*active/.test(rule.media.mediaText));
          } else if ("cssRules" in rule && !(rule instanceof CSSStyleRule)) {
            read((rule as CSSGroupingRule).cssRules, forced);
          } else if (forced && rule instanceof CSSStyleRule) {
            const style: Record<string, string> = {};
            for (const name of [...rule.style]) style[name] = rule.style.getPropertyValue(name);
            rules.push({ selector: rule.selectorText, style });
          }
        }
      };
      for (const sheet of [...document.styleSheets]) {
        try {
          read(sheet.cssRules, false);
        } catch {
          // A sheet of another origin cannot be read, and none of the window's own is one.
        }
      }
      const query = matchMedia("(forced-colors: active)");
      return { known: query.media !== "not all", active: query.matches, rules };
    });

    expect(found.known).toBe(true);
    const lower = (value: string | undefined) => (value ?? "").toLowerCase();
    const focus = found.rules.find((rule) => rule.selector.includes(":focus-visible"));
    expect(lower(focus?.style["outline-color"])).toBe("highlight");
    const tab = found.rules.find((rule) =>
      rule.selector.includes('[role="tab"][aria-selected="true"]'),
    );
    expect(lower(tab?.style["background-color"])).toBe("selecteditem");
    const mark = found.rules.find((rule) => rule.selector.includes(".state-running"));
    expect(lower(mark?.style["background-color"])).toBe("canvastext");
    // The runner draws no contrast theme, so none of it is drawn now.
    expect(found.active).toBe(false);
  });
});
