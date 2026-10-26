// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **One overlay surface** (`primitives/overlays.css`, #1845): a dialog, a popover, a menu and a
 * tooltip are one ground, one border, one corner and one elevation, a dialog and a popover one
 * padding, and what is pressed inside one is the shared control.
 *
 * Held to what a person sees and what a contributor can break:
 * - **the surface is the same on every overlay**, read through the window's cascade
 *   (`cascade.testkit.ts`), so a view rule that tries to redraw one does not count;
 * - **a dialog still works**: it is announced by its title, the keyboard stays in it, and Escape
 *   leaves it; a popover opens from the keyboard and says what it is;
 * - **every overlay in the window wears the surface**, and every button in one the shared
 *   button, read from the source the way `controls.test.tsx` reads the menus;
 * - **`App.css` draws no overlay surface of its own**: the classes it keeps (`.row-card`) only
 *   place and size theirs.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import ts from "typescript";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "../cascade.testkit";
import { isGeometry } from "../theme/geometry";
import { MOTION_TOKENS, motionProperty } from "../theme/motion";
import { property, TOKENS } from "../theme/theme";
import { Button } from "../ui/button/Button";
import { Menu, MenuItem, MenuTrigger } from "../ui/menu/Menu";
import { Dialog } from "../ui/dialog/Dialog";
import { PopoverDialog, PopoverTrigger } from "../ui/popover/Popover";

afterEach(cleanup);

const SRC = join(process.cwd(), "src");
const sheet = (file: string) =>
  readFileSync(join(SRC, file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const CSS = sheet("primitives/overlays.css");

/** Every rule of the overlays sheet, as `[selector, body]`. */
const RULES: [string, string][] = [...CSS.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(
  ([, selector, body]) => [selector.trim().replace(/\s+/g, " "), body],
);

describe("the overlays stylesheet", () => {
  it("is in the components layer, after the controls it shares a surface with", () => {
    const styles = sheet("styles.css");
    // In parts: the stylesheet guard reads a quoted path ending in the extension as an import.
    const overlays = ['@import "./primitives/overlays', 'css" layer(components);'].join(".");
    const controls = ['@import "./primitives/controls', 'css" layer(components);'].join(".");
    expect(styles).toContain(overlays);
    expect(styles.indexOf(overlays)).toBeGreaterThan(styles.indexOf(controls));
  });

  it("reads only tokens", () => {
    const colours = new Set(TOKENS.map(property));
    const motion = new Set(MOTION_TOKENS.map(motionProperty));
    const read = [...CSS.matchAll(/var\((--[\w-]+)/g)].map((hit) => hit[1]);
    expect(read.length).toBeGreaterThan(0);
    expect(
      read.filter((name) => !colours.has(name) && !motion.has(name) && !isGeometry(name)),
    ).toEqual([]);
  });

  it("writes no length of its own: every size is a token, a hairline or nothing", () => {
    const lengths = RULES.flatMap(([selector, body]) =>
      [...body.matchAll(/(?<![\w-])(\d*\.?\d+)(px|rem|em|vh|vw|ch)\b/g)]
        .filter(([whole]) => whole !== "1px" && whole !== "0px")
        .map(([whole]) => `${selector} | ${whole}`),
    );
    expect(lengths).toEqual([]);
  });

  it("gives every colour as one token, so the contrast check reads every pair", () => {
    const loose = RULES.flatMap(([selector, body]) =>
      [...body.matchAll(/(?<![\w-])(color|background-color|background|fill)\s*:\s*([^;]+)/g)]
        .map(([, , value]) => value.trim())
        .filter((value) => !/^var\(--[\w-]+\)$/.test(value))
        .map((value) => `${selector} | ${value}`),
    );
    expect(loose).toEqual([]);
  });
});

/** A dialog the way the window draws one: a scrim, the surface, a title and its answers. */
function Asking() {
  const [open, setOpen] = useState(true);
  return (
    <Dialog
      isOpen={open}
      onClose={() => setOpen(false)}
      title="Rename workspace"
      main={{ children: "Rename" }}
    />
  );
}

function Card() {
  return (
    <PopoverTrigger>
      <Button>About this row</Button>
      <PopoverDialog className="row-card" aria-label="What this row is" arrow>
        <p>Its facts.</p>
      </PopoverDialog>
    </PopoverTrigger>
  );
}

function Opener() {
  return (
    <MenuTrigger>
      <Button>More</Button>
      <Menu className="more-menu" aria-label="More">
        <MenuItem textValue="Rename">Rename</MenuItem>
      </Menu>
    </MenuTrigger>
  );
}

/** The surface an overlay of the popover family stands on: its menu's or its dialog's. */
const surfaceOf = (inside: HTMLElement): HTMLElement =>
  inside.closest<HTMLElement>('[data-scope="popover"][data-part="root"]') ?? inside;

describe("one surface", () => {
  /** What every overlay is drawn on, with, and lifted by. */
  const SURFACE: [string, string][] = [
    ["background", "var(--surface-overlay)"],
    ["color", "var(--text-primary)"],
    ["border", "1px solid var(--border-subtle)"],
    ["border-radius", "var(--radius-lg)"],
    ["box-shadow", "var(--elevation-overlay)"],
  ];
  /** The same, as the dialog family writes it (`ui/dialog/dialog.css`). */
  const DIALOG_SURFACE: [string, string][] = [
    ["background-color", "var(--surface-overlay)"],
    ["color", "var(--text-primary)"],
    ["border", "1px solid var(--border-subtle)"],
    ["border-radius", "var(--radius-lg)"],
    ["box-shadow", "var(--elevation-overlay)"],
  ];

  it("draws a dialog, a popover, a menu and a tooltip on one ground, border, corner and lift", async () => {
    const user = userEvent.setup();
    render(<Card />);
    await user.click(screen.getByRole("button", { name: "About this row" }));
    const popover = surfaceOf(await screen.findByRole("dialog", { name: "What this row is" }));
    for (const [name, value] of SURFACE)
      expect(drawnWith(popover, name), `popover ${name}`).toBe(value);
    cleanup();

    render(<Opener />);
    await user.click(screen.getByRole("button", { name: "More" }));
    const menu = surfaceOf(await screen.findByRole("menu"));
    for (const [name, value] of SURFACE) expect(drawnWith(menu, name), `menu ${name}`).toBe(value);
    cleanup();

    render(
      <div data-scope="tooltip" data-part="root" role="tooltip">
        Kept in purlis.toml
      </div>,
    );
    const tooltip = screen.getByRole("tooltip");
    for (const [name, value] of SURFACE)
      expect(drawnWith(tooltip, name), `tooltip ${name}`).toBe(value);
    cleanup();

    render(<Asking />);
    const dialog = screen.getByRole("dialog", { name: "Rename workspace" });
    for (const [name, value] of DIALOG_SURFACE)
      expect(drawnWith(dialog, name), `dialog ${name}`).toBe(value);
  });

  it("lays a scrim over the window in the theme's scrim colour", () => {
    render(<Asking />);
    const scrim = document.querySelector('[data-scope="dialog"][data-part="scrim"]');
    if (scrim === null) throw new Error("the dialog drew no scrim");
    expect(drawnWith(scrim, "position")).toBe("fixed");
    expect(drawnWith(scrim, "inset")).toBe("0");
    expect(drawnWith(scrim, "background-color")).toBe("var(--overlay-scrim)");
  });

  it("draws a popover's arrow in the surface's ground", async () => {
    const user = userEvent.setup();
    render(<Card />);
    await user.click(screen.getByRole("button", { name: "About this row" }));
    const popover = surfaceOf(await screen.findByRole("dialog", { name: "What this row is" }));
    const arrow = popover.querySelector('[data-part="arrow"] svg');
    if (arrow === null) throw new Error("the popover drew no arrow");
    expect(drawnWith(arrow, "fill")).toBe("var(--surface-overlay)");
  });

  it("draws the dialog's answers as the shared buttons, in a row that keeps their size", () => {
    render(<Asking />);
    const dialog = screen.getByRole("dialog", { name: "Rename workspace" });
    const rename = within(dialog).getByRole("button", { name: "Rename" });
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    for (const button of [rename, cancel]) {
      // The footer's row wraps a long answer's words inside the dialog's width, and keeps every
      // answer at least the control's height.
      expect(drawnWith(button, "min-block-size")).toBe("var(--size-control)");
      expect(drawnWith(button, "border-radius")).toBe("var(--radius-control)");
    }
    expect(drawnWith(rename, "background")).toBe("var(--accent-fill)");
    expect(drawnWith(cancel, "background")).toBe("var(--control-base)");
  });

  it("is a dialog still: named by its title, the keyboard kept in it, Escape leaves", async () => {
    const user = userEvent.setup();
    render(<Asking />);
    const dialog = screen.getByRole("dialog", { name: "Rename workspace" });
    const rename = within(dialog).getByRole("button", { name: "Rename" });
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    expect(cancel).toHaveFocus();
    await user.tab();
    expect(rename).toHaveFocus();
    await user.tab();
    expect(cancel).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens a popover from the keyboard and says what it is", async () => {
    const user = userEvent.setup();
    render(<Card />);
    await user.tab();
    await user.keyboard("{Enter}");
    const card = await screen.findByRole("dialog", { name: "What this row is" });
    expect(within(card).getByText("Its facts.")).toBeTruthy();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

/** Every non-test component file under `src/`, as source text. */
const FILES = Object.entries(
  import.meta.glob("../**/*.tsx", { query: "?raw", import: "default", eager: true }) as Record<
    string,
    string
  >,
).filter(([path]) => !path.includes(".test.") && !path.includes(".stories."));

/** The Radix overlay parts and the class each must wear. */
const PARTS: [RegExp, string][] = [
  [/^(Popover|HoverCard)\.Content$/, "ui-popover"],
  [/^Tooltip\.Content$/, "ui-tooltip"],
  [/^(Popover|HoverCard|Tooltip)\.Arrow$/, "ui-overlay-arrow"],
];

/** The popover and menu families' overlays (#2909), which draw their own surface. */
const FAMILY = /^(AnchoredPopover|HeldPopover|PopoverDialog|Menu|InlineMenu)$/;

/** A native control inside an overlay, and the class it must wear. A button of the button family
 *  is `Button` (#2904), so a native `<button>` left inside one is a row, a segment or a menu's
 *  row. */
const NATIVE: Record<string, string[]> = {
  button: ["ui-row", "ui-tree-row", "ui-segmented-item", "ui-menu-item"],
  // A box is the field family's (#2905), in a dialog's settings row too, so a native box left
  // inside one is never right. A select is the select family's (#2906), in a settings row too
  // (#3289), so a native one left inside an overlay wears only the old select class.
  input: [],
  textarea: [],
  select: ["ui-select"],
};

/** The button family's members (`ui/button/Button.tsx`), which draw their own look. */
const BUTTONS = new Set(["Button", "IconButton", "ToggleButton"]);

interface Found {
  where: string;
  tag: string;
  classes: string[][];
}

/** The class names an element says it wears: one list per string a `className` can be. */
function classesOf(attributes: ts.JsxAttributes, source: ts.SourceFile): string[][] {
  const named = attributes.properties.find(
    (one) => ts.isJsxAttribute(one) && one.name.getText(source) === "className",
  ) as ts.JsxAttribute | undefined;
  if (named?.initializer === undefined) return [[]];
  const said: string[][] = [];
  const walk = (node: ts.Node) => {
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node))
      said.push(node.text.split(/\s+/));
    else if (ts.isTemplateExpression(node)) said.push(node.getText(source).split(/[\s`${}]+/));
    else if (node.kind === ts.SyntaxKind.UndefinedKeyword || node.getText(source) === "undefined")
      said.push([]);
    else ts.forEachChild(node, walk);
  };
  walk(named.initializer);
  return said.length === 0 ? [[]] : said;
}

/** Each Radix overlay part and each native control inside an overlay's content, by file. */
function overlaysIn(): { parts: Found[]; controls: Found[]; families: Found[] } {
  const parts: Found[] = [];
  const controls: Found[] = [];
  const families: Found[] = [];
  for (const [path, text] of FILES) {
    const source = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    const visit = (node: ts.Node, inside: boolean) => {
      let open: ts.JsxOpeningElement | ts.JsxSelfClosingElement | undefined;
      if (ts.isJsxElement(node)) open = node.openingElement;
      else if (ts.isJsxSelfClosingElement(node)) open = node;
      let within = inside;
      if (open !== undefined) {
        const tag = open.tagName.getText(source);
        const where = `${path.replace(/^\.\.\//, "")}:${source.getLineAndCharacterOfPosition(open.getStart(source)).line + 1}`;
        const classes = classesOf(open.attributes, source);
        if (PARTS.some(([part]) => part.test(tag))) parts.push({ where, tag, classes });
        if (/\.Content$/.test(tag) && PARTS.some(([part]) => part.test(tag))) within = true;
        if (FAMILY.test(tag)) {
          families.push({ where, tag, classes });
          within = true;
        }
        if (inside && tag in NATIVE) controls.push({ where, tag, classes });
        if (inside && BUTTONS.has(tag)) controls.push({ where, tag: "Button", classes });
      }
      ts.forEachChild(node, (child) => visit(child, within));
    };
    visit(source, false);
  }
  return { parts, controls, families };
}

describe("every overlay in the window", () => {
  const { parts, controls, families } = overlaysIn();

  it("is found: the cards, the lists and the controls inside them", () => {
    // The dialogs are the dialog family's, whose answers it draws itself (`ui/dialog/Dialog.tsx`),
    // and are held by its own tests: no Radix dialog is left.
    expect(parts.filter(({ tag }) => /^(Dialog|AlertDialog)\./.test(tag))).toEqual([]);
    // The cards are the popover family's: a chat row's, a list row's and a refused name's; and
    // the title bar's two switchers (#2916).
    expect(families.filter(({ tag }) => /Popover/.test(tag)).length).toBe(5);
    expect(parts.filter(({ tag }) => tag === "Popover.Content").length).toBe(0);
  });

  it("wears the surface: a scrim, a dialog, a popover, a tooltip and an arrow each its class", () => {
    const bare = parts.filter(({ tag, classes }) => {
      const wanted = PARTS.find(([part]) => part.test(tag))?.[1] ?? "";
      return classes.some((one) => !one.includes(wanted));
    });
    expect(bare.map(({ where, tag }) => `${where} ${tag}`)).toEqual([]);
  });

  it("draws every button and field inside one with the shared control", () => {
    const bare = controls.filter(
      ({ tag, classes }) =>
        tag in NATIVE && classes.some((one) => !NATIVE[tag].some((wanted) => one.includes(wanted))),
    );
    expect(bare.map(({ where, tag }) => `${where} ${tag}`)).toEqual([]);
  });

  it("would be caught if a bare one came back", () => {
    const source = (text: string) =>
      ts.createSourceFile("X.tsx", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    const attrs = (text: string) => {
      const file = source(text);
      let found: ts.JsxAttributes | undefined;
      const visit = (node: ts.Node) => {
        if (
          found === undefined &&
          (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node))
        )
          found = node.attributes;
        ts.forEachChild(node, visit);
      };
      visit(file);
      if (found === undefined) throw new Error("no element");
      return classesOf(found, file);
    };
    expect(attrs(`<button className={busy ? "ui-button ends-it" : undefined} />`)).toEqual([
      ["ui-button", "ends-it"],
      [],
    ]);
    expect(attrs(`<button type="button" />`)).toEqual([[]]);
  });
});

describe("App.css", () => {
  const app = sheet("App.css");
  /** What a surface is drawn with, as opposed to where it is and how big. */
  const LOOKS = [
    "background",
    "background-color",
    "border",
    "border-color",
    "border-radius",
    "box-shadow",
    "padding",
  ];

  it("draws no overlay surface of its own: the classes it keeps only place and size theirs", () => {
    const rules = [...app.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(([, selector, body]) => [
      selector.trim().replace(/\s+/g, " "),
      body,
    ]);
    const surfaces = /^(\.row-card|\.ui-popover|\.ui-tooltip|\.palette|\.chat-card)(\.[\w-]+)*$/;
    const drawn = rules.flatMap(([selector, body]) =>
      selector
        .split(/\s*,\s*/)
        .filter((one) => surfaces.test(one))
        .flatMap((one) =>
          LOOKS.filter((look) => new RegExp(`(?<![\\w-])${look}:`).test(body)).map(
            (look) => `${one} | ${look}`,
          ),
        ),
    );
    expect(drawn).toEqual([]);
    expect(app).not.toMatch(/\n\.asking \{/);
    // A dialog's surface is the family's (`ui/dialog/dialog.css`): the window keeps no other.
    expect(app).not.toMatch(/\.(ui-dialog|ui-scrim|warning)(?![\w-])/);
  });

  it("lifts nothing by a shadow of its own: every lift is an elevation token", () => {
    const shadows = [...app.matchAll(/(?<![\w-])box-shadow:\s*([^;]+)/g)]
      .map(([, value]) => value.trim())
      .filter((value) => value.includes("--overlay-shadow"));
    expect(shadows).toEqual([]);
    expect(app).not.toMatch(/var\(--overlay-scrim\)/);
  });
});
