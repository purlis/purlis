// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $$, browser, expect } from "@wdio/globals";

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
 */

type Found = {
  /** Each forced-colours query in the window's sheets, as the engine parsed it: its text, whether
   *  the engine knows it, and whether it matches now. */
  queries: { text: string; known: boolean; active: boolean }[];
  rules: { selector: string; style: Record<string, string> }[];
};

/** Each control's computed style, by its `data-p`, before and after forced colours are active. */
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
        (rule) => rule.selector.includes('[data-part="grip"]') && rule.selector.includes(state),
      );
    expect(lower(grip('[data-separator="hover"]')?.style["background-color"])).toBe("canvastext");
    expect(lower(grip('[data-separator="active"]')?.style["background-color"])).toBe("highlight");
    const tab = found.rules.find((rule) =>
      rule.selector.includes('[role="tab"][aria-selected="true"]'),
    );
    expect(lower(tab?.style["background-color"])).toBe("selecteditem");
    const mark = found.rules.find((rule) => rule.selector.includes(".state-running"));
    expect(lower(mark?.style["background-color"])).toBe("canvastext");
  });

  it("draw focus, selection, what needs you, danger, state marks, switches and dialogs in the system's colours when active", async () => {
    await untilTheStripIsRead();
    const drawn: Drawn = await browser.execute(() => {
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
      // Each control with the window's own classes and attributes, as its component draws it.
      box.innerHTML = [
        '<input data-p="focus" type="text" aria-label="probe">',
        '<div role="tablist"><button data-p="tab" type="button" role="tab" aria-selected="true">tab</button></div>',
        '<div><div data-p="option" role="option" aria-selected="true">option</div></div>',
        '<span data-p="needs" data-scope="count" data-part="root" data-variant="hot">2</span>',
        '<button data-p="needs-number-pill" type="button" data-scope="status-item" data-part="root" data-variant="waiting"><span data-p="needs-number" data-scope="status-item" data-part="count">2</span></button>',
        '<span data-p="needs-activity" data-scope="activity" data-part="count" data-tone="needs-you">2</span>',
        '<button data-p="danger" type="button" data-scope="button" data-part="root" data-variant="danger" data-size="md">end</button>',
        '<p data-p="trouble" class="trouble">trouble</p>',
        '<span data-p="running" class="state-running"></span>',
        '<span data-p="waiting" class="state-waiting"></span>',
        '<span data-p="failed" class="state-failed"></span>',
        '<label data-scope="switch" data-part="root" data-selected="true"><span data-p="switch-on" data-scope="switch" data-part="track"><span data-p="thumb-on" data-scope="switch" data-part="thumb"></span></span></label>',
        '<label data-scope="switch" data-part="root"><span data-p="switch-off" data-scope="switch" data-part="track"><span data-p="thumb-off" data-scope="switch" data-part="thumb"></span></span></label>',
        '<div data-p="dialog" role="dialog" aria-label="probe">dialog</div>',
        '<div data-p="alertdialog" role="alertdialog" aria-label="probe">alert</div>',
      ].join("");
      document.body.appendChild(box);

      // What the engine resolves each system colour to, through an inline style no rule beats.
      const system: Record<string, string> = {};
      for (const name of [
        "Canvas",
        "CanvasText",
        "Highlight",
        "HighlightText",
        "SelectedItem",
        "SelectedItemText",
        "Mark",
        "MarkText",
      ]) {
        const probe = document.createElement("span");
        probe.style.setProperty("color", name);
        box.appendChild(probe);
        system[name] = CSS.supports("color", name) ? getComputedStyle(probe).color : "";
        probe.remove();
      }

      const read = () => {
        const out: Record<string, Record<string, string>> = {};
        for (const el of [...box.querySelectorAll<HTMLElement>("[data-p]")]) {
          const style = getComputedStyle(el);
          out[el.dataset.p ?? ""] = {
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
        box.querySelector<HTMLElement>('[data-p="focus"]')?.focus();
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
    });

    expect(drawn.already || drawn.placed > 0).toBe(true);
    const unknown = Object.entries(drawn.system)
      .filter(([, value]) => value === "")
      .map(([name]) => name);
    expect(unknown).toEqual([]);
    const sys = drawn.system;
    const at = (name: string, ...keys: string[]) =>
      Object.fromEntries(keys.map((key) => [key, drawn.after[name]?.[key]]));
    // Without forced colours none of the shapes below is drawn, so it is the query that drew them.
    if (!drawn.already) {
      expect(drawn.before.dialog?.outlineStyle).not.toBe("solid");
      expect(drawn.before.danger?.outlineStyle).not.toBe("dashed");
      expect(drawn.before.focus?.outlineColor).not.toBe(sys.Highlight);
    }

    // Focus: a solid ring in Highlight wherever Tab lands.
    expect(at("focus", "focusVisible", "outlineStyle", "outlineWidth", "outlineColor")).toEqual({
      focusVisible: "true",
      outlineStyle: "solid",
      outlineWidth: "2px",
      outlineColor: sys.Highlight,
    });
    // What is selected or current: the system's pair, and a tab is underlined.
    expect(at("tab", "color", "background", "decoration")).toEqual({
      color: sys.SelectedItemText,
      background: sys.SelectedItem,
      decoration: "underline",
    });
    expect(at("option", "color", "background")).toEqual({
      color: sys.SelectedItemText,
      background: sys.SelectedItem,
    });
    // What needs you: Mark and MarkText.
    for (const needs of ["needs", "needs-number", "needs-activity"])
      expect({ [needs]: at(needs, "color", "background") }).toEqual({
        [needs]: { color: sys.MarkText, background: sys.Mark },
      });
    // What ends or destroys, and a ground that says something is wrong: a dashed outline.
    for (const danger of ["danger", "trouble"])
      expect({
        [danger]: at(danger, "outlineStyle", "outlineWidth", "outlineColor", "outlineOffset"),
      }).toEqual({
        [danger]: {
          outlineStyle: "dashed",
          outlineWidth: "2px",
          outlineColor: sys.CanvasText,
          outlineOffset: "-2px",
        },
      });
    // A state mark keeps its shape in CanvasText; waiting adds its ring.
    for (const mark of ["running", "waiting", "failed"])
      expect({ [mark]: drawn.after[mark]?.background }).toEqual({ [mark]: sys.CanvasText });
    expect(drawn.after.waiting?.shadow).toContain(sys.Canvas);
    expect(drawn.after.waiting?.shadow).toContain(sys.CanvasText);
    // A switch: on is filled in Highlight with its thumb in HighlightText; off has a CanvasText
    // thumb.
    expect(at("switch-on", "background")).toEqual({ background: sys.Highlight });
    expect(drawn.after["thumb-on"]?.background).toBe(sys.HighlightText);
    expect(drawn.after["thumb-off"]?.background).toBe(sys.CanvasText);
    // Its track is outlined in the text's colour, on or off, so the pill keeps its shape.
    expect(at("switch-off", "outlineStyle", "outlineWidth", "outlineColor")).toEqual({
      outlineStyle: "solid",
      outlineWidth: "1px",
      outlineColor: sys.CanvasText,
    });
    // What is opened over the window: outlined, in place of its shadow.
    for (const dialog of ["dialog", "alertdialog"])
      expect({ [dialog]: at(dialog, "outlineStyle", "outlineWidth", "outlineColor") }).toEqual({
        [dialog]: { outlineStyle: "solid", outlineWidth: "1px", outlineColor: sys.CanvasText },
      });
  });
});
