// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **A field with a leading icon** (#1856, the field family since #2905): the family's box with
 * the glass at its start, for every box that filters or searches.
 *
 * Held here to what a person sees and what a contributor can break:
 * - **it is still the family's box**: one control tall, the outside ring, read through the
 *   window's cascade;
 * - **the text starts past the icon**: the box's start inset, the icon's size and the gap after
 *   it are core tokens, which a view lining the text up with something else may hand it;
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
import { TextField } from "../ui/field/Field";

afterEach(cleanup);

const SRC = join(process.cwd(), "src");

function FilterBox() {
  return (
    <TextField
      type="search"
      icon={<Search aria-hidden="true" />}
      aria-label="Filter files"
      value=""
      onChange={() => {}}
    />
  );
}

/** The box a search field is drawn in: the text input's root. */
const boxOf = (input: Element) => input.parentElement as Element;

describe("a field with a leading icon", () => {
  it("is the family's box: one control tall, with the outside ring", () => {
    render(<FilterBox />);
    const box = boxOf(screen.getByRole("searchbox", { name: "Filter files" }));
    expect(drawnWith(box, "block-size")).toBe("var(--size-control)");
    box.setAttribute("data-focus-within", "true");
    expect(drawnWith(box, "outline")).toBe("var(--focus-ring-width) solid var(--focus-ring)");
    expect(drawnWith(box, "outline-offset")).toBe("var(--focus-ring-offset)");
  });

  it("starts its text past the icon, by the box's inset, the icon's size and a gap", () => {
    render(<FilterBox />);
    const box = boxOf(screen.getByRole("searchbox", { name: "Filter files" }));
    const icon = box.querySelector('[data-scope="text-input"][data-part="icon"]') as Element;
    // Each size a token, which a view lining the text up with something else may hand it.
    expect(drawnWith(box, "padding-inline-start")).toBe("var(--text-input-start, var(--space-3))");
    expect(drawnWith(box, "gap")).toBe("var(--text-input-gap, var(--space-2))");
    const glass = icon.querySelector("svg") as Element;
    expect(drawnWith(glass, "inline-size")).toBe("var(--text-input-icon, 1em)");
    expect(drawnWith(glass, "block-size")).toBe("var(--text-input-icon, 1em)");
    // The icon is an item of the box's own row, before the words.
    expect(icon.nextElementSibling?.tagName).toBe("INPUT");
  });

  it("is decoration: hidden from a screen reader and the pointer, the field keeps its name", async () => {
    const user = userEvent.setup();
    render(<FilterBox />);
    const icon = document.querySelector('[data-part="icon"]') as Element;
    expect(icon).toHaveAttribute("aria-hidden", "true");
    expect(drawnWith(icon, "pointer-events")).toBe("none");
    const input = screen.getByRole("searchbox", { name: "Filter files" });
    await user.tab();
    expect(input).toHaveFocus();
    await user.keyboard("main");
    expect(input).toHaveAttribute("type", "search");
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

/**
 * Each search box in a file's text that is not the field family's with a leading icon: a raw
 * `<input type="search">`, or a `<TextField type="search">` with no `icon`.
 */
function bareSearchBoxes(name: string, text: string): string[] {
  const source = ts.createSourceFile(name, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const out: string[] = [];
  const visit = (node: ts.Node) => {
    const element = ts.isJsxSelfClosingElement(node)
      ? node
      : ts.isJsxElement(node)
        ? node.openingElement
        : undefined;
    if (element && attr(element, "type") === "search") {
      const tag = element.tagName.getText();
      const line = source.getLineAndCharacterOfPosition(element.getStart()).line + 1;
      if (tag === "input" || (tag === "TextField" && attr(element, "icon") === undefined))
        out.push(`${name}:${line}`);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return out;
}

describe("the window's search boxes", () => {
  it("are each the family's box with the glass at its start", () => {
    const files = sources();
    expect(files.length).toBeGreaterThan(50);
    expect(
      files.flatMap((file) => bareSearchBoxes(relative(SRC, file), readFileSync(file, "utf8"))),
    ).toEqual([]);
  });

  it("finds a raw box, and a family box with no glass", () => {
    const planted = (box: string) => bareSearchBoxes("planted.tsx", `const a = ${box};`);
    expect(
      planted(`<TextField type="search" icon={<Search aria-hidden="true" />} value="" />`),
    ).toEqual([]);
    expect(planted(`<div><input type="search" /></div>`)).toEqual(["planted.tsx:1"]);
    expect(planted(`<TextField type="search" value="" />`)).toEqual(["planted.tsx:1"]);
  });
});
