// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { classKeyed, keyedOnClasses, parted, partsWithoutTheirClass } from "./partClasses";

describe("a stylesheet keyed on its parts' classes (#4351, ADR 0103 amended 2026-11-04)", () => {
  it("keys a scope and a part on their classes, quoted or not", () => {
    expect(classKeyed(':where([data-scope="dialog"][data-part="modal"]) { color: red; }')).toBe(
      ":where(.ds-dialog.dp-modal) { color: red; }",
    );
    expect(classKeyed("[data-scope=status-bar] [data-part='separator']{}")).toBe(
      ".ds-status-bar .dp-separator{}",
    );
  });

  it("leaves every other attribute, a presence test and a case-blind match as they are", () => {
    const kept = [
      '[data-scope="tabs"][data-variant="file"]',
      "[data-part]",
      '[data-scope^="dia"]',
      '[data-scope="dialog" i]',
      '[data-scope="has space"]',
    ];
    for (const one of kept)
      expect(classKeyed(one)).toBe(one.replace('[data-scope="tabs"]', ".ds-tabs"));
  });
});

describe("the build's PostCSS step", () => {
  it("keys each rule's selector on its parts' classes, where the rule stands", () => {
    const rule = { selector: ':where([data-scope="tabs"][data-part="tab"]), [data-part="x"] > p' };
    keyedOnClasses().Rule(rule);
    expect(rule.selector).toBe(":where(.ds-tabs.dp-tab), .dp-x > p");
  });
});

describe("a part's props carry its classes", () => {
  it("adds the scope's and the part's class beside the attributes, which stay", () => {
    expect(parted({ "data-scope": "dialog", "data-part": "modal", id: "x" })).toEqual({
      "data-scope": "dialog",
      "data-part": "modal",
      id: "x",
      className: "ds-dialog dp-modal",
    });
  });

  it("keeps the class it was given, and leaves props with no part alone", () => {
    expect(parted({ "data-part": "body", className: "mt-2" }).className).toBe("mt-2 dp-body");
    const plain = { id: "x" };
    expect(parted(plain)).toBe(plain);
  });

  it("draws the classes of the attributes it ends with, not of a part it was handed", () => {
    // A family that names its own scope over the one its caller passed: the element is the
    // family's part, so it carries the family's classes alone.
    expect(
      parted({ "data-scope": "button", "data-part": "root", className: "ds-dialog dp-close x" })
        .className,
    ).toBe("x ds-button dp-root");
  });

  it("composes a class that is a function of the state, as React Aria's components take", () => {
    const { className } = parted({
      "data-scope": "menu",
      "data-part": "item",
      className: ({ isFocused }: { isFocused: boolean }) => (isFocused ? "aimed" : ""),
    });
    expect(typeof className).toBe("function");
    expect((className as (state: { isFocused: boolean }) => string)({ isFocused: true })).toBe(
      "aimed ds-menu dp-item",
    );
  });
});

describe("what the window drew without its parts' classes", () => {
  it("names each part whose classes are not exactly its attributes'", () => {
    const root = document.createElement("div");
    root.innerHTML = [
      '<p data-scope="card" data-part="root" class="ds-card dp-root"></p>',
      '<p data-scope="card" data-part="title"></p>',
      '<p data-scope="button" data-part="root" class="ds-dialog ds-button dp-root"></p>',
      '<p class="dp-root"></p>',
    ].join("");
    expect(partsWithoutTheirClass(root)).toEqual([
      'card/title: class "" should be "ds-card dp-title"',
      'button/root: class "ds-dialog ds-button dp-root" should be "ds-button dp-root"',
      '-/-: class "dp-root" should be ""',
    ]);
  });
});
