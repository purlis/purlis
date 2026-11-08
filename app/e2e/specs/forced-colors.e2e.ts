// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $$, browser, expect } from "@wdio/globals";
import { SYSTEM_COLOURS, type ForcedValues } from "../../src/theme/forcedProbe.testkit.ts";
import { forcedProbes, type NamedProbe } from "../forcedProbes.js";

/**
 * **Forced colours, against the real app** (#2328, #2583, `src/theme/forced-colors.css`).
 *
 * `forced-colors.test.ts` holds what the sheet says, and `tailwind.test.ts` that Tailwind's
 * compiler keeps it in the last layer. What neither can hold is that it **survives the Vite build
 * into a WebView**, that the WebView reads it, and that, with forced colours active, it wins the
 * cascade over every view, control and utility: jsdom evaluates no media query and computes no
 * cascade.
 *
 * The first test reads the window's own style sheets as the engine parsed them. The second turns
 * forced colours on and reads what is drawn. A runner cannot turn a contrast theme on, and the
 * WebViews the scenario run drives (WKWebView, WebKitGTK) offer no media emulation to a WebDriver
 * client, so the window's own forced-colours rules are placed beside their media query, in the
 * same layer and in the same order, which is what the query matching does to the cascade. Where
 * the WebView already matches `(forced-colors: active)`, nothing is placed and what it draws is
 * read as it is. Then controls carrying the window's own classes are drawn, and each one's
 * computed style must be the system colour the engine resolves, with the shape the sheet gives it.
 * What is drawn, and what each must read, is each probe's: the window's in
 * `src/theme/forced-colors.probe.ts`, and a family's in `src/ui/<family>/forced-colors.probe.ts`
 * (#3481, `e2e/forcedProbes.ts`).
 */

type Found = {
  /** Each forced-colours query in the window's sheets, as the engine parsed it: its text, whether
   *  the engine knows it, and whether it matches now. */
  queries: { text: string; known: boolean; active: boolean }[];
  rules: { selector: string; style: Record<string, string> }[];
};

/** Each control's computed style, by its probe's name and its `data-p` (`badge/needs`), before and
 *  after forced colours are active. */
type Drawn = {
  already: boolean;
  placed: number;
  system: Record<string, string>;
  before: Record<string, Record<string, string>>;
  after: Record<string, Record<string, string>>;
};

async function untilTheStripIsRead(): Promise<void> {
  await browser.waitUntil(
    async () =>
      (await $$('[data-testid="title-bar"] [data-titlebar-all]').getElements()).length > 0,
    { timeout: 30_000, interval: 250, timeoutMsg: "the strip never listed the fixture plane" },
  );
}

describe("forced colours", () => {
  it("are in the built window, in a media query the WebView reads, in the system's colours", async () => {
    await untilTheStripIsRead();
    const found: Found = await browser.execute(() => {
      const rules: { selector: string; style: Record<string, string> }[] = [];
      const queries: { text: string; known: boolean; active: boolean }[] = [];
      const read = (list: CSSRuleList, forced: boolean) => {
        for (const rule of [...list]) {
          if (rule instanceof CSSMediaRule) {
            const text = rule.media.mediaText;
            const mine = /forced-colors/.test(text);
            if (mine) {
              const query = matchMedia(text);
              queries.push({ text, known: query.media !== "not all", active: query.matches });
            }
            read(rule.cssRules, forced || mine);
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
      return { queries, rules };
    });

    // The sheet's own query, exactly as written, and one the engine knows: a query that never
    // matches would leave the test below placing rules no forced-colours platform draws. The
    // runner draws no contrast theme, so it does not match now.
    expect(found.queries.length).toBeGreaterThan(0);
    for (const query of found.queries)
      expect(query).toEqual({ text: "(forced-colors: active)", known: true, active: false });
    const lower = (value: string | undefined) => (value ?? "").toLowerCase();
    // The window's focus rule: the one that draws the ring, not a resize handle's grip (#3143).
    const focus = found.rules.find(
      (rule) => rule.selector.includes(":focus-visible") && rule.style["outline-color"],
    );
    expect(lower(focus?.style["outline-color"])).toBe("highlight");
    // A resize handle's grip: the words' colour under the pointer, the highlight while it is
    // dragged or has the keyboard.
    const grip = (state: string) =>
      found.rules.find(
        // Keyed on the part's class once built (#4351, `src/ui/partClasses.ts`).
        (rule) => rule.selector.includes(".dp-grip") && rule.selector.includes(state),
      );
    expect(lower(grip('[data-separator="hover"]')?.style["background-color"])).toBe("canvastext");
    expect(lower(grip('[data-separator="active"]')?.style["background-color"])).toBe("highlight");
    const tab = found.rules.find((rule) =>
      rule.selector.includes('[role="tab"][aria-selected="true"]'),
    );
    expect(lower(tab?.style["background-color"])).toBe("selecteditem");
    const mark = found.rules.find((rule) => rule.selector.includes(".explorer .touch-mark"));
    expect(lower(mark?.style["background-color"])).toBe("canvastext");
  });

  // Each probe in its own folder, the window's and each family's (#3481): a family adds its file
  // and touches no line here.
  let probes: NamedProbe[] = [];
  before(async () => {
    probes = await forcedProbes();
  });

  it("draw what each probe draws in the system's colours when active", async () => {
    await untilTheStripIsRead();
    expect(probes.length).toBeGreaterThan(1);
    const drawn: Drawn = await browser.execute(
      (given: { name: string; markup: string; focus?: string }[], colours: readonly string[]) => {
        // No named helpers inside `execute`: the spec's bundler wraps a function declaration in a
        // `__name` the page does not have. Arrows assigned to a name are what the test above uses.
        const already = matchMedia("(forced-colors: active)").matches;
        const placed: { list: CSSStyleSheet | CSSGroupingRule; at: number; count: number }[] = [];
        const place = (list: CSSStyleSheet | CSSGroupingRule) => {
          // From the end, so a rule placed after one query moves no query not yet visited.
          for (let at = list.cssRules.length - 1; at >= 0; at--) {
            const rule = list.cssRules[at];
            if (
              rule instanceof CSSMediaRule &&
              /forced-colors:\s*active/.test(rule.media.mediaText)
            ) {
              const inner = [...rule.cssRules].map((one) => one.cssText);
              inner.forEach((text, n) => list.insertRule(text, at + 1 + n));
              placed.push({ list, at: at + 1, count: inner.length });
            } else if ("cssRules" in rule && !(rule instanceof CSSStyleRule)) {
              place(rule as CSSGroupingRule);
            }
          }
        };

        const box = document.createElement("div");
        box.setAttribute("data-forced-probe", "");
        box.style.setProperty("position", "fixed");
        box.style.setProperty("left", "-10000px");
        box.style.setProperty("top", "0");
        // Each probe's controls, with the window's own classes and attributes, as its component
        // draws them, in a holder of its own so two probes' `data-p` never meet.
        for (const one of given) {
          const holder = document.createElement("div");
          holder.setAttribute("data-forced-family", one.name);
          holder.innerHTML = one.markup;
          box.appendChild(holder);
        }
        // Each hand-drawn part takes the classes the built stylesheet keys it on (#4351).
        for (const one of [box, ...box.querySelectorAll("[data-scope], [data-part]")]) {
          const { scope, part } = (one as HTMLElement).dataset;
          if (scope) one.classList.add(`ds-${scope}`);
          if (part) one.classList.add(`dp-${part}`);
        }
        document.body.appendChild(box);

        // What the engine resolves each system colour to, through an inline style no rule beats.
        const system: Record<string, string> = {};
        for (const name of colours) {
          const probe = document.createElement("span");
          probe.style.setProperty("color", name);
          box.appendChild(probe);
          system[name] = CSS.supports("color", name) ? getComputedStyle(probe).color : "";
          probe.remove();
        }

        // Each control's computed style, by its probe's name and its `data-p`.
        const read = () => {
          const out: Record<string, Record<string, string>> = {};
          for (const el of [
            ...box.querySelectorAll<HTMLElement>("[data-forced-family] [data-p]"),
          ]) {
            const family = el.closest<HTMLElement>("[data-forced-family]")?.dataset.forcedFamily;
            const style = getComputedStyle(el);
            out[`${family}/${el.dataset.p ?? ""}`] = {
              color: style.color,
              background: style.backgroundColor,
              border: style.borderTopColor,
              outlineStyle: style.outlineStyle,
              outlineWidth: style.outlineWidth,
              outlineColor: style.outlineColor,
              outlineOffset: style.outlineOffset,
              decoration: style.textDecorationLine,
              shadow: style.boxShadow,
              focusVisible: String(el.matches(":focus-visible")),
            };
          }
          return out;
        };

        const before = read();
        const was = document.activeElement as HTMLElement | null;
        try {
          if (!already) {
            for (const sheet of [...document.styleSheets]) {
              try {
                place(sheet);
              } catch {
                // A sheet of another origin cannot be read, and none of the window's own is one.
              }
            }
          }
          for (const one of given)
            if (one.focus)
              box
                .querySelector<HTMLElement>(
                  `[data-forced-family="${one.name}"] [data-p="${one.focus}"]`,
                )
                ?.focus();
          return {
            already,
            placed: placed.reduce((sum, one) => sum + one.count, 0),
            system,
            before,
            after: read(),
          };
        } finally {
          // The window as it was, for the specs after this one: last placed first, so each
          // place's index is the one it was placed at.
          for (const one of placed.reverse())
            for (let n = 0; n < one.count; n++) one.list.deleteRule(one.at);
          box.remove();
          was?.focus();
        }
      },
      probes.map(({ name, probe }) => ({ name, markup: probe.markup, focus: probe.focus })),
      SYSTEM_COLOURS,
    );

    expect(drawn.already || drawn.placed > 0).toBe(true);
    const unknown = Object.entries(drawn.system)
      .filter(([, value]) => value === "")
      .map(([name]) => name);
    expect(unknown).toEqual([]);
    // A value as the probe wrote it, with a system colour as the engine resolves it.
    const resolved = (values: ForcedValues): Record<string, string> =>
      Object.fromEntries(
        Object.entries(values).map(([key, value]) => [
          key,
          typeof value === "string" ? value : (drawn.system[value.system] ?? ""),
        ]),
      );
    const at = (
      from: Record<string, Record<string, string>>,
      control: string,
      keys: readonly string[],
    ) => Object.fromEntries(keys.map((key) => [key, from[control]?.[key]]));

    for (const { name, probe } of probes) {
      // Without forced colours none of these is drawn, so it is the query that draws them.
      if (!drawn.already)
        for (const [p, values] of Object.entries(probe.unforced ?? {}))
          for (const [key, value] of Object.entries(resolved(values)))
            expect({
              [`${name}/${p}`]: { [key]: drawn.before[`${name}/${p}`]?.[key] },
            }).not.toEqual({ [`${name}/${p}`]: { [key]: value } });
      for (const [p, values] of Object.entries(probe.forced)) {
        const want = resolved(values);
        expect({ [`${name}/${p}`]: at(drawn.after, `${name}/${p}`, Object.keys(want)) }).toEqual({
          [`${name}/${p}`]: want,
        });
      }
    }
  });
});
