// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **A field with a leading icon** (`.ui-input-lead`, `primitives/controls.css`, #1856): the
 * shared field with the glass at its start, for every box that filters or searches.
 *
 * DS-12 (#1844) moved those boxes onto `.ui-input`, which had no icon part, so they lost their
 * glass (D-1844-2). Held here to what a person sees and what a contributor can break:
 * - **it is still the shared field**: one control tall, the outside ring, read through the
 *   window's cascade;
 * - **the text starts past the icon**: the field's start inset is the icon's inset, its size and
 *   a gap, all core tokens, so typed text never runs under the glass;
 * - **the icon is decoration**: hidden from a screen reader and from the pointer, so the box keeps
 *   its own name and a press on the glass lands in the field;
 * - **every search box in the window wears it**, read from the source with the TypeScript AST.
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Search } from "lucide-react";
import { drawnWith } from "../cascade.testkit";

afterEach(cleanup);

/** A declared value as one line: the stylesheet's formatter breaks a long one over several. */
const flat = (value: string | undefined) =>
  value?.replace(/\(\s+/g, "(").replace(/\s+\)/g, ")").replace(/\s+/g, " ");

const SRC = join(process.cwd(), "src");

function FilterBox() {
  return (
    <div className="ui-input-lead">
      <Search className="ui-input-lead-icon" aria-hidden="true" />
      <input type="search" className="ui-input" tabIndex={0} aria-label="Filter files" />
    </div>
  );
}

describe("a field with a leading icon", () => {
  it("is the shared field: one control tall, with the outside ring", () => {
    render(<FilterBox />);
    const box = screen.getByRole("searchbox", { name: "Filter files" });
    expect(drawnWith(box, "block-size")).toBe("var(--size-control)");
    expect(drawnWith(box, "outline", { focusVisible: true })).toBe(
      "var(--focus-ring-width) solid var(--focus-ring)",
    );
  });

  it("starts its text past the icon, by the icon's inset, its size and a gap", () => {
    render(<FilterBox />);
    const box = screen.getByRole("searchbox", { name: "Filter files" });
    const icon = document.querySelector(".ui-input-lead-icon") as Element;
    // Each size a token, which a view lining the text up with something else may hand it.
    expect(drawnWith(icon, "inset-inline-start")).toBe("var(--ui-input-lead-at, var(--space-3))");
    expect(drawnWith(icon, "inline-size")).toBe("var(--ui-input-lead-size, var(--font-size-base))");
    expect(drawnWith(icon, "block-size")).toBe("var(--ui-input-lead-size, var(--font-size-base))");
    expect(flat(drawnWith(box, "padding-inline-start"))).toBe(
      "var(--ui-input-lead-text, calc(var(--space-3) + var(--font-size-base) + var(--space-2)))",
    );
    // The icon is placed against the field's own box, centred on its height.
    const lead = icon.parentElement as Element;
    expect(drawnWith(lead, "position")).toBe("relative");
    expect(drawnWith(icon, "position")).toBe("absolute");
  });

  it("is decoration: hidden from a screen reader and the pointer, the field keeps its name", async () => {
    const user = userEvent.setup();
    render(<FilterBox />);
    const icon = document.querySelector(".ui-input-lead-icon") as Element;
    expect(icon).toHaveAttribute("aria-hidden", "true");
    expect(drawnWith(icon, "pointer-events")).toBe("none");
    const box = screen.getByRole("searchbox", { name: "Filter files" });
    await user.tab();
    expect(box).toHaveFocus();
    await user.keyboard("main");
    expect(box).toHaveValue("main");
  });
});

/** Every source file the window is drawn from: not tests. */
function sources(dir = SRC): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return name.endsWith(".tsx") && !/\.test\.tsx$|\.testkit\.tsx$/.test(name) ? [path] : [];
  });
}

const attr = (element: ts.JsxOpeningLikeElement, name: string): string | undefined => {
  for (const one of element.attributes.properties) {
    if (!ts.isJsxAttribute(one) || one.name.getText() !== name) continue;
    if (one.initializer === undefined) return "";
    return one.initializer.getText().replace(/^["{`]+|["}`]+$/g, "");
  }
  return undefined;
};

const opening = (node: ts.JsxChild): ts.JsxOpeningLikeElement | undefined =>
  ts.isJsxSelfClosingElement(node) ? node : ts.isJsxElement(node) ? node.openingElement : undefined;

/**
 * Each `<input type="search">` in a file's text that is not a shared field with a leading icon:
 * its parent is not a `.ui-input-lead`, the input is not a `.ui-input`, or no hidden
 * `.ui-input-lead-icon` stands before it.
 */
function bareSearchBoxes(name: string, text: string): string[] {
  const source = ts.createSourceFile(name, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const classes = (element: ts.JsxOpeningLikeElement | undefined) =>
    element === undefined ? [] : (attr(element, "className") ?? "").split(/\s+/);
  const out: string[] = [];
  const visit = (node: ts.Node) => {
    if (
      ts.isJsxSelfClosingElement(node) &&
      node.tagName.getText() === "input" &&
      attr(node, "type") === "search"
    ) {
      const line = source.getLineAndCharacterOfPosition(node.getStart()).line + 1;
      const parent = node.parent;
      const siblings = ts.isJsxElement(parent) ? [...parent.children] : [];
      const icon = siblings
        .slice(0, siblings.indexOf(node))
        .map(opening)
        .find((one) => classes(one).includes("ui-input-lead-icon"));
      const lead =
        ts.isJsxElement(parent) && classes(parent.openingElement).includes("ui-input-lead");
      const field = classes(node).includes("ui-input");
      const hidden = icon !== undefined && attr(icon, "aria-hidden") !== undefined;
      if (!lead || !field || !hidden) out.push(`${name}:${line}`);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return out;
}

describe("the window's search boxes", () => {
  it("are each the shared field with the glass at its start", () => {
    const files = sources();
    expect(files.length).toBeGreaterThan(50);
    expect(
      files.flatMap((file) => bareSearchBoxes(relative(SRC, file), readFileSync(file, "utf8"))),
    ).toEqual([]);
  });

  it("finds a box that is bare, one with no glass, and one whose glass is announced", () => {
    const planted = (box: string) => bareSearchBoxes("planted.tsx", `const a = ${box};`);
    const input = `<input type="search" className="ui-input" />`;
    const glass = `<Search className="ui-input-lead-icon" aria-hidden="true" />`;
    expect(planted(`<div className="ui-input-lead">${glass}${input}</div>`)).toEqual([]);
    expect(planted(`<div>${input}</div>`)).toEqual(["planted.tsx:1"]);
    expect(planted(`<div className="ui-input-lead">${input}</div>`)).toEqual(["planted.tsx:1"]);
    expect(
      planted(
        `<div className="ui-input-lead"><Search className="ui-input-lead-icon" />${input}</div>`,
      ),
    ).toEqual(["planted.tsx:1"]);
    expect(planted(`<div className="ui-input-lead">${glass}<input type="search" /></div>`)).toEqual(
      ["planted.tsx:1"],
    );
  });
});
