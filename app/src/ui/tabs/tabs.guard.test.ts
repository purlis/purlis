// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **Choosing a tab never moves one** (#2914, #2800·2; R134, R149): the family default changes a
 * tab's colour and fill with its state and nothing else, so no tab changes its width when the
 * choice moves or the pointer passes.
 *
 * Held over `tabs.css`'s text, the way a browser reads it: every rule whose subject is a tab or
 * something inside one, in a state (chosen, under the pointer, the keyboard on it), sets no
 * property that sizes a box: no padding, margin, border width, weight, size, gap, width or
 * height. A state's pill and flares are drawn by pseudo-elements, absolutely, outside the
 * tab's box, so they are exempt. The story measures the same in a real browser
 * (`Tabs.stories.tsx`, Docked).
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const CSS = readFileSync(join(process.cwd(), "src/ui/tabs/tabs.css"), "utf8").replace(
  /\/\*[\s\S]*?\*\//g,
  "",
);

/** A state's attribute or pseudo-class, as the family's selectors spell them. */
const STATE = /\[data-(?:selected|hovered|focus-visible|pressed)\]|:(?:hover|focus-visible|active)/;

/** A property that sizes a box, or moves what is beside it. */
const SIZES =
  /^(?:padding|margin|border(?:-(?:block|inline|top|right|bottom|left))?(?:-(?:start|end))?-width|border(?:-(?:block|inline|top|right|bottom|left))?(?:-(?:start|end))?|font(?:-(?:size|weight|family|stretch))?|letter-spacing|gap|column-gap|row-gap|(?:min-|max-)?(?:inline|block)-size|(?:min-|max-)?(?:width|height)|flex(?:-(?:basis|grow|shrink))?|line-height)(?:-[\w-]+)?$/;

/** Each rule: its selectors, each split at its top-level commas, and its declarations. */
function rules(css: string): { selectors: string[]; declared: string[] }[] {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(([, selectors, body]) => ({
    selectors: selectors
      .split(/,(?![^(]*\))/)
      .map((one) => one.replace(/\s+/g, " ").trim())
      .filter((one) => one !== ""),
    declared: body
      .split(";")
      .map((one) => one.split(":")[0]?.trim() ?? "")
      .filter((one) => one !== "" && !one.startsWith("--")),
  }));
}

describe("a tab's state", () => {
  const stated = rules(CSS).flatMap(({ selectors, declared }) =>
    selectors
      // The tab or what is inside it, in a state, and not a pseudo-element drawn outside it.
      .filter((one) => /\[data-part="tab"\]/.test(one) && STATE.test(one) && !/::/.test(one))
      .map((selector) => ({ selector, declared })),
  );

  it("is found: the chosen tab, the tab under the pointer and the × under it", () => {
    const found = stated.map(({ selector }) => selector).join("\n");
    expect(found).toContain('[data-scope="tabs"][data-part="tab"][data-selected]');
    expect(found).toContain('[data-scope="tabs"][data-part="tab"][data-hovered]');
    expect(found).toContain('[data-part="closer"][data-scope="tabs"]:hover');
  });

  it("changes a tab's colour and fill and nothing that sizes it", () => {
    const sized = stated.flatMap(({ selector, declared }) =>
      declared
        // A corner is drawn inside the box and sizes nothing.
        .filter((property) => SIZES.test(property) && !/radius/.test(property))
        .map((one) => `${selector} | ${one}`),
    );
    expect(sized).toEqual([]);
  });

  it("never changes the weight of a tab's words, chosen or not", () => {
    // R149: the tab-strip prototype's selected weight is refused (#2800·2).
    expect(CSS).not.toMatch(/font-weight/);
  });

  it("draws its pill and its flares out of the tab's box, so they take no room", () => {
    const outside = rules(CSS).filter(({ selectors }) =>
      selectors.some((one) => /\[data-part="tab"\]\[data-(?:selected|hovered)\][^ ]*::/.test(one)),
    );
    expect(outside.length).toBeGreaterThan(0);
    for (const { selectors } of outside) {
      const body = CSS.slice(CSS.indexOf(selectors[0]));
      const declared = body.slice(body.indexOf("{"), body.indexOf("}"));
      if (/content:/.test(declared)) expect(declared).toMatch(/position: absolute/);
    }
  });
});

/**
 * **Nothing the window's own stylesheet says of a chat tab is lost to the layers** (#2914).
 * `App.css` is in the `charter` layer and `tabs.css` in `components`, above it, so a property
 * both set on a tab's cell or its button is `tabs.css`'s whatever the selectors weigh. A rule in
 * `App.css` that still sets one would be dead, silently: each moves to `tabs.css`.
 */
describe("the window's own stylesheet and the family's", () => {
  const app = readFileSync(join(process.cwd(), "src/App.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );
  /** The properties each rule of `css` sets on what `subject` matches, by selector. */
  const setOn = (css: string, subject: RegExp) =>
    rules(css).flatMap(({ selectors, declared }) =>
      selectors
        .filter((one) => subject.test(one) && !/::/.test(one))
        .flatMap((selector) => declared.map((property) => ({ selector, property }))),
    );
  /** A selector's last compound, after its last combinator. */
  const LAST = String.raw`[^\s>+~]*$`;
  const cases = [
    {
      what: "a tab's cell",
      app: new RegExp(String.raw`(?:^|[\s>+~])\.tab(?![\w-])` + LAST),
      family: new RegExp(String.raw`\[data-part="tab"\]` + LAST),
    },
    {
      what: "a tab's button",
      app: new RegExp(String.raw`\.tab(?![\w-])[^\s>+~]*\s*>\s*\[role="tab"\]` + LAST),
      family: new RegExp(String.raw`\[data-part="tab"\][^\s>+~]*\s*>\s*\[role="tab"\]` + LAST),
    },
  ];

  it.each(cases)("leaves no property of $what to App.css that tabs.css sets", (one) => {
    const family = new Set(setOn(CSS, one.family).map(({ property }) => property));
    const lost = setOn(app, one.app)
      .filter(({ property }) => family.has(property))
      .map(({ selector, property }) => `${selector} | ${property}`);
    expect(lost).toEqual([]);
  });

  it("finds App.css's rules on a chat tab, so the case above reads something", () => {
    expect(setOn(app, cases[0].app).map(({ selector }) => selector)).toContain(
      ".tabs .tab[data-dragging]",
    );
  });
});
