// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **The control primitives** (`primitives/controls.css`, ADR 0094 §4, #1834): one class per kind of
 * control, drawn from the tokens alone, over Radix's behaviour.
 *
 * Each control is held to what a person sees and what a contributor can break, never to class
 * plumbing:
 * - **its height and its focus ring come from the tokens**, read through the window's cascade
 *   (`cascade.testkit.ts`, which knows `primitives/controls.css` is the layer above `App.css`), so a view
 *   rule that tries to redraw a classed control does not count;
 * - **the keyboard works it** as the primitive under it promises: Tab reaches it, and Enter,
 *   Space or an arrow does what a click would;
 * - **it is announced by name**, with its role and its state.
 *
 * The stylesheet itself is read as text too (jsdom applies none): it sits in the `components`
 * layer, reads only tokens, and writes no length the tokens do not give it.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { useState } from "react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import * as Checkbox from "@radix-ui/react-checkbox";
import * as Menu from "@radix-ui/react-dropdown-menu";
import * as RadioGroup from "@radix-ui/react-radio-group";
import * as Select from "@radix-ui/react-select";
import * as Switch from "@radix-ui/react-switch";
import { drawnWith } from "../cascade.testkit";
import { MenuKeys, MenuWords } from "./MenuRow";
import { isGeometry } from "../theme/geometry";
import { MOTION_TOKENS, motionProperty } from "../theme/motion";
import { property, TOKENS } from "../theme/theme";

afterEach(cleanup);

beforeAll(() => {
  // jsdom lays nothing out: Radix Select scrolls its chosen row into view and asks for pointer
  // capture, neither of which jsdom has. A real WebView has both.
  Element.prototype.scrollIntoView ??= () => {};
  Element.prototype.hasPointerCapture ??= () => false;
  Element.prototype.releasePointerCapture ??= () => {};
});

const SRC = join(process.cwd(), "src");
const sheet = (file: string) =>
  readFileSync(join(SRC, file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const CSS = sheet("primitives/controls.css");

/** Every rule of the primitives, as `[selector, body]`. */
const RULES: [string, string][] = [...CSS.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(
  ([, selector, body]) => [selector.trim().replace(/\s+/g, " "), body],
);

/** The ring a control draws outside itself, and the one a segment draws inside. */
const RING = "var(--focus-ring-width) solid var(--focus-ring)";

/** A control's height and its focus ring, as the window's cascade draws them. */
function heldToTheTokens(
  control: Element,
  { height = "var(--size-control)", offset = "var(--focus-ring-offset)" } = {},
) {
  expect(drawnWith(control, "block-size"), "its height").toBe(height);
  expect(drawnWith(control, "outline", { focusVisible: true }), "its focus ring").toBe(RING);
  expect(drawnWith(control, "outline-offset", { focusVisible: true }), "its ring's place").toBe(
    offset,
  );
}

describe("the primitives stylesheet", () => {
  it("is in the components layer, above App.css", () => {
    const styles = sheet("styles.css");
    // In two halves: the stylesheet guard reads a quoted path ending in the extension as an
    // import (`literals.test.ts`).
    expect(styles).toContain(
      ['@import "./primitives/controls', 'css" layer(components);'].join("."),
    );
    const order = (/@layer ([\w\s,-]+);/.exec(styles)?.[1] ?? "").split(/\s*,\s*/);
    expect(order.indexOf("components")).toBeGreaterThan(order.indexOf("charter"));
  });

  it("reads only tokens: a theme's colours and motion, the core geometry, and its own", () => {
    const colours = new Set(TOKENS.map(property));
    const motion = new Set(MOTION_TOKENS.map(motionProperty));
    // Its own: a length one primitive works out once (`--ui-switch-track`), and the ones a view
    // may set for it (a menu row's `--ui-menu-indent` and a menu's least and most width,
    // `--ui-menu-min` and `--ui-menu-max`; a leading icon's `--ui-input-lead-*`, which `App.css`
    // declares for the Chats filter).
    const own = new Set([...CSS.matchAll(/^\s+(--[\w-]+):/gm)].map((hit) => hit[1]));
    const fromViews = new Set([
      "--ui-menu-indent",
      "--ui-menu-min",
      "--ui-menu-max",
      "--ui-input-lead-at",
      "--ui-input-lead-size",
      "--ui-input-lead-text",
    ]);
    for (const name of fromViews) expect(sheet("App.css")).toContain(`${name}:`);
    const fromRadix = new Set([
      "--radix-select-trigger-width",
      "--radix-select-content-available-height",
      "--radix-context-menu-content-available-height",
      "--radix-dropdown-menu-content-available-height",
    ]);
    const read = [...CSS.matchAll(/var\((--[\w-]+)/g)].map((hit) => hit[1]);
    expect(read.length).toBeGreaterThan(0);
    expect(
      read.filter(
        (name) =>
          !colours.has(name) &&
          !motion.has(name) &&
          !isGeometry(name) &&
          !own.has(name) &&
          !fromViews.has(name) &&
          // What Radix measures for the primitive it draws: a select's trigger width and room,
          // named one at a time as `theme/literals.test.ts` lists them, so a typo still fails.
          !fromRadix.has(name),
      ),
    ).toEqual([]);
  });

  it("writes no length of its own: every size is a token, a hairline or nothing", () => {
    // `1px` is a border and a rule's hairline; `0px` is nothing, as a fallback. Everything else
    // is a token or a token's share.
    const lengths = RULES.flatMap(([selector, body]) =>
      [...body.matchAll(/(?<![\w-])(\d*\.?\d+)(px|rem|em|vh|vw|ch)\b/g)]
        .filter(([whole]) => whole !== "1px" && whole !== "0px")
        .map(([whole]) => `${selector} | ${whole}`),
    );
    expect(lengths).toEqual([]);
  });

  it("gives every colour as one token, so the contrast check reads every pair", () => {
    const loose = RULES.flatMap(([selector, body]) =>
      [...body.matchAll(/(?<![\w-])(color|background-color|background)\s*:\s*([^;]+)/g)]
        .map(([, , value]) => value.trim())
        .filter((value) => !/^var\(--[\w-]+\)$/.test(value) && value !== "transparent")
        .map((value) => `${selector} | ${value}`),
    );
    expect(loose).toEqual([]);
  });
});

describe("the button", () => {
  const VARIANTS = ["", "ui-button-primary", "ui-button-ghost", "ui-button-danger"];

  function Buttons({ onPress }: { onPress: (name: string) => void }) {
    return (
      <>
        {VARIANTS.map((variant) => (
          <button
            key={variant}
            type="button"
            tabIndex={0}
            className={`ui-button ${variant}`.trim()}
            onClick={() => onPress(variant || "secondary")}
          >
            Save {variant || "secondary"}
          </button>
        ))}
        <button
          type="button"
          tabIndex={0}
          className="ui-button ui-button-ghost ui-button-sm"
          onClick={() => onPress("small")}
        >
          Close
        </button>
      </>
    );
  }

  it("is one control height in every variant, the chrome height small, with the outside ring", () => {
    render(<Buttons onPress={() => {}} />);
    for (const variant of VARIANTS) {
      heldToTheTokens(screen.getByRole("button", { name: `Save ${variant || "secondary"}` }));
    }
    heldToTheTokens(screen.getByRole("button", { name: "Close" }), {
      height: "var(--size-chrome)",
    });
  });

  it("draws each variant on its own ground, from the tokens", () => {
    render(<Buttons onPress={() => {}} />);
    const ground = (name: string) => drawnWith(screen.getByRole("button", { name }), "background");
    expect(ground("Save secondary")).toBe("var(--control-base)");
    expect(ground("Save ui-button-primary")).toBe("var(--accent-surface)");
    expect(ground("Save ui-button-ghost")).toBe("transparent");
    expect(ground("Save ui-button-danger")).toBe("var(--control-base)");
    expect(drawnWith(screen.getByRole("button", { name: "Save ui-button-danger" }), "color")).toBe(
      "var(--danger-base)",
    );
  });

  it("draws a grant as plain as the secondary, edged in the accent (ADR 0097 §6)", () => {
    render(
      <button type="button" tabIndex={0} className="ui-button ui-button-grants">
        Allow
      </button>,
    );
    const grant = screen.getByRole("button", { name: "Allow" });
    heldToTheTokens(grant);
    expect(drawnWith(grant, "background")).toBe("var(--control-base)");
    expect(drawnWith(grant, "color")).toBe("var(--text-primary)");
    expect(drawnWith(grant, "border-color")).toBe("var(--accent-base)");
    expect(drawnWith(grant, "background", { hover: true })).toBe("var(--control-hover)");
  });

  it("is reached by Tab and pressed by Enter and Space", async () => {
    const user = userEvent.setup();
    const pressed: string[] = [];
    render(<Buttons onPress={(name) => pressed.push(name)} />);
    await user.tab();
    expect(screen.getByRole("button", { name: "Save secondary" })).toHaveFocus();
    await user.keyboard("{Enter}");
    await user.tab();
    await user.keyboard(" ");
    expect(pressed).toEqual(["secondary", "ui-button-primary"]);
  });

  it("as a toggle, is said pressed and stands on the accent while it is, under the pointer too", async () => {
    function Toggle() {
      const [on, setOn] = useState(false);
      return (
        <button
          type="button"
          tabIndex={0}
          className="ui-button ui-button-sm"
          aria-pressed={on}
          onClick={() => setOn((was) => !was)}
        >
          Changed only
        </button>
      );
    }
    const user = userEvent.setup();
    render(<Toggle />);
    const toggle = screen.getByRole("button", { name: "Changed only", pressed: false });
    expect(drawnWith(toggle, "background")).toBe("var(--control-base)");
    await user.tab();
    await user.keyboard(" ");
    expect(screen.getByRole("button", { name: "Changed only", pressed: true })).toBe(toggle);
    expect(drawnWith(toggle, "background")).toBe("var(--accent-surface)");
    expect(drawnWith(toggle, "border-color")).toBe("var(--accent-base)");
    expect(drawnWith(toggle, "background", { hover: true })).toBe("var(--tab-active)");
    heldToTheTokens(toggle, { height: "var(--size-chrome)" });
  });
});

describe("the field", () => {
  it("is one control height with the outside ring, named by its label, and typed into", async () => {
    const user = userEvent.setup();
    render(
      <>
        <label htmlFor="name">Name</label>
        <input id="name" className="ui-input" tabIndex={0} />
        <label htmlFor="notes">Notes</label>
        <textarea id="notes" className="ui-input" tabIndex={0} />
      </>,
    );
    const field = screen.getByRole("textbox", { name: "Name" });
    heldToTheTokens(field);
    await user.tab();
    expect(field).toHaveFocus();
    await user.keyboard("steward");
    expect(field).toHaveValue("steward");
    // Rows of text are at least three controls tall, and grow as a person drags them.
    const notes = screen.getByRole("textbox", { name: "Notes" });
    expect(drawnWith(notes, "block-size")).toBe("auto");
    expect(drawnWith(notes, "min-block-size")).toBe("calc(var(--size-control) * 3)");
  });
});

describe("a refused value", () => {
  it("draws the field's edge in the danger colour, and only while it is refused", () => {
    render(
      <>
        <input className="ui-input" aria-label="Colour" aria-invalid="true" readOnly />
        <input className="ui-input" aria-label="Name" aria-invalid="false" readOnly />
      </>,
    );
    const refused = screen.getByRole("textbox", { name: "Colour" });
    expect(drawnWith(refused, "border-color")).toBe("var(--danger-base)");
    expect(
      drawnWith(screen.getByRole("textbox", { name: "Name" }), "border-color"),
    ).toBeUndefined();
  });
});

describe("the select", () => {
  function Picker({ onPick }: { onPick: (value: string) => void }) {
    return (
      <>
        <label id="harness-label">Harness</label>
        <Select.Root defaultValue="claude" onValueChange={onPick}>
          <Select.Trigger className="ui-select" aria-labelledby="harness-label" tabIndex={0}>
            <Select.Value />
          </Select.Trigger>
          <Select.Portal>
            <Select.Content className="ui-menu" position="popper">
              <Select.Viewport>
                {["claude", "codex", "opencode"].map((one) => (
                  <Select.Item key={one} value={one} className="ui-menu-item">
                    <Select.ItemText>{one}</Select.ItemText>
                  </Select.Item>
                ))}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
      </>
    );
  }

  it("is one control height with the outside ring, and a native select is the same", () => {
    render(
      <>
        <Picker onPick={() => {}} />
        <select className="ui-select" aria-label="Level" tabIndex={0}>
          <option>Project</option>
        </select>
      </>,
    );
    heldToTheTokens(screen.getByRole("combobox", { name: "Harness" }));
    heldToTheTokens(screen.getByRole("combobox", { name: "Level" }));
  });

  it("is named, opened by the keyboard, and picks with the arrows and Enter", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Picker onPick={picked} />);
    const trigger = screen.getByRole("combobox", { name: "Harness" });
    expect(trigger).toHaveTextContent("claude");
    await user.tab();
    expect(trigger).toHaveFocus();
    await user.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    // The list is drawn in the one menu style.
    expect(list.closest(".ui-menu")).not.toBeNull();
    expect(within(list).getByRole("option", { name: "claude" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.keyboard("{ArrowDown}{Enter}");
    expect(picked).toHaveBeenCalledWith("codex");
    expect(trigger).toHaveTextContent("codex");
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});

describe("the checkbox", () => {
  it("draws its box from the control height, with the outside ring, and Space ticks it", async () => {
    const user = userEvent.setup();
    render(
      <>
        <Checkbox.Root id="live" className="ui-checkbox" tabIndex={0}>
          <Checkbox.Indicator>✓</Checkbox.Indicator>
        </Checkbox.Root>
        <label htmlFor="live">Start it live</label>
      </>,
    );
    const box = screen.getByRole("checkbox", { name: "Start it live" });
    expect(drawnWith(box, "block-size")).toBe("calc(var(--size-control) * 3 / 7)");
    expect(drawnWith(box, "outline", { focusVisible: true })).toBe(RING);
    expect(drawnWith(box, "outline-offset", { focusVisible: true })).toBe(
      "var(--focus-ring-offset)",
    );
    await user.tab();
    expect(box).toHaveFocus();
    await user.keyboard(" ");
    expect(box).toHaveAttribute("aria-checked", "true");
    expect(drawnWith(box, "border-color")).toBe("var(--accent-base)");
  });
});

describe("the radio", () => {
  it("draws its mark from the control height; the arrows move along it and Space picks", async () => {
    const user = userEvent.setup();
    const chose = vi.fn();
    render(
      <RadioGroup.Root aria-label="Where it is kept" defaultValue="team" onValueChange={chose}>
        {["team", "machine"].map((one) => (
          <span key={one}>
            <RadioGroup.Item id={one} value={one} className="ui-radio">
              <RadioGroup.Indicator className="ui-radio-mark" />
            </RadioGroup.Item>
            <label htmlFor={one}>{one}</label>
          </span>
        ))}
      </RadioGroup.Root>,
    );
    const group = screen.getByRole("radiogroup", { name: "Where it is kept" });
    const team = within(group).getByRole("radio", { name: "team" });
    expect(drawnWith(team, "block-size")).toBe("calc(var(--size-control) * 3 / 7)");
    expect(drawnWith(team, "outline", { focusVisible: true })).toBe(RING);
    await user.tab();
    expect(team).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    const machine = within(group).getByRole("radio", { name: "machine" });
    expect(machine).toHaveFocus();
    // Radix checks a radio the arrows land on in a browser, through a click it sends on focus;
    // jsdom does not run that path (the settings set picks on focus itself, `useArrowPick`).
    // Space picks everywhere.
    await user.keyboard(" ");
    expect(machine).toHaveAttribute("aria-checked", "true");
    expect(chose).toHaveBeenLastCalledWith("machine");
  });
});

describe("the switch", () => {
  it("is half a control tall, with the outside ring, and Space and Enter turn it", async () => {
    const user = userEvent.setup();
    render(
      <>
        <Switch.Root id="sound" className="ui-switch" tabIndex={0}>
          <Switch.Thumb className="ui-switch-thumb" />
        </Switch.Root>
        <label htmlFor="sound">Play a sound</label>
      </>,
    );
    const toggle = screen.getByRole("switch", { name: "Play a sound" });
    expect(drawnWith(toggle, "--ui-switch-track")).toBe("calc(var(--size-control) / 2)");
    expect(drawnWith(toggle, "block-size")).toBe("var(--ui-switch-track)");
    expect(drawnWith(toggle, "outline", { focusVisible: true })).toBe(RING);
    await user.tab();
    expect(toggle).toHaveFocus();
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await user.keyboard(" ");
    expect(toggle).toHaveAttribute("aria-checked", "true");
    // On is said by the thumb's place and the accent, not by colour alone.
    expect(drawnWith(toggle, "border-color")).toBe("var(--accent-base)");
    await user.keyboard("{Enter}");
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });
});

describe("the segmented control", () => {
  function Levels({ small = false }: { small?: boolean }) {
    const [level, setLevel] = useState("project");
    return (
      <RadioGroup.Root
        aria-label="Level"
        orientation="horizontal"
        className={small ? "ui-segmented ui-segmented-sm" : "ui-segmented"}
        value={level}
        onValueChange={setLevel}
      >
        {["project", "workspace", "you"].map((one) => (
          <RadioGroup.Item key={one} value={one} className="ui-segmented-item">
            {one}
          </RadioGroup.Item>
        ))}
      </RadioGroup.Root>
    );
  }

  it("is one control height, the chrome height small, its ring inside each segment", () => {
    const { unmount } = render(<Levels />);
    const group = screen.getByRole("radiogroup", { name: "Level" });
    expect(drawnWith(group, "block-size")).toBe("var(--size-control)");
    const segment = within(group).getByRole("radio", { name: "project" });
    expect(drawnWith(segment, "outline", { focusVisible: true })).toBe(RING);
    expect(drawnWith(segment, "outline-offset", { focusVisible: true })).toBe(
      "var(--focus-ring-inset)",
    );
    expect(drawnWith(segment, "background")).toBe("var(--accent-surface)");
    unmount();
    render(<Levels small />);
    expect(drawnWith(screen.getByRole("radiogroup", { name: "Level" }), "block-size")).toBe(
      "var(--size-chrome)",
    );
  });

  it("moves along with the arrows, picks with Space, and says which segment is chosen", async () => {
    const user = userEvent.setup();
    render(<Levels />);
    await user.tab();
    const group = screen.getByRole("radiogroup", { name: "Level" });
    expect(within(group).getByRole("radio", { name: "project" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    const workspace = within(group).getByRole("radio", { name: "workspace" });
    expect(workspace).toHaveFocus();
    await user.keyboard(" ");
    expect(workspace).toHaveAttribute("aria-checked", "true");
    expect(within(group).getByRole("radio", { name: "project" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });
});

describe("the menu", () => {
  function Opener({ onGo }: { onGo: (to: string) => void }) {
    return (
      <Menu.Root modal={false}>
        <Menu.Trigger className="ui-button ui-button-ghost" tabIndex={0}>
          More
        </Menu.Trigger>
        <Menu.Portal>
          <Menu.Content className="ui-menu more-menu" aria-label="More">
            <Menu.Item className="ui-menu-item" onSelect={() => onGo("rename")}>
              Rename
            </Menu.Item>
            <Menu.Item className="ui-menu-item" disabled>
              Copy
            </Menu.Item>
            <Menu.Separator className="ui-menu-line" />
            <Menu.Item className="ui-menu-item ui-menu-item-danger" onSelect={() => onGo("delete")}>
              Delete
            </Menu.Item>
          </Menu.Content>
        </Menu.Portal>
      </Menu.Root>
    );
  }

  it("opens from the keyboard, skips a row that cannot run, and picks with Enter", async () => {
    const user = userEvent.setup();
    const went = vi.fn();
    render(<Opener onGo={went} />);
    await user.tab();
    await user.keyboard("{Enter}");
    const menu = await screen.findByRole("menu");
    const rename = within(menu).getByRole("menuitem", { name: "Rename" });
    expect(rename).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(within(menu).getByRole("menuitem", { name: "Delete" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(went).toHaveBeenCalledWith("delete");
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("draws a row at least a control tall, highlighted rather than ringed", async () => {
    const user = userEvent.setup();
    render(<Opener onGo={() => {}} />);
    await user.tab();
    await user.keyboard("{Enter}");
    const menu = await screen.findByRole("menu");
    expect(drawnWith(menu, "box-shadow")).toBe("var(--elevation-overlay)");
    expect(drawnWith(menu, "border-radius")).toBe("var(--radius-lg)");
    const rename = within(menu).getByRole("menuitem", { name: "Rename" });
    expect(drawnWith(rename, "min-block-size")).toBe("var(--size-control)");
    expect(drawnWith(rename, "outline")).toBe("none");
    expect(rename).toHaveAttribute("data-highlighted");
    expect(drawnWith(rename, "background")).toBe("var(--tab-active)");
    const del = within(menu).getByRole("menuitem", { name: "Delete" });
    expect(drawnWith(del, "color")).toBe("var(--danger-base)");
    // The width is the one style's too, from the spacing scale: two short verbs do not make a
    // sliver, and a long name does not widen the menu across the window.
    expect(drawnWith(menu, "min-inline-size")).toBe("var(--ui-menu-min, calc(var(--space-8) * 6))");
    expect(drawnWith(menu, "max-inline-size")).toBe(
      "var(--ui-menu-max, calc(var(--space-8) * 10))",
    );
    expect(drawnWith(menu, "min-width")).toBeUndefined();
    expect(drawnWith(menu, "background")).toBe("var(--surface-overlay)");
  });

  /** A menu drawn from the row's parts (`MenuRow.tsx`), as a tab's context menu is: a row
   *  with a key at its end, and a row that cannot be undone with the one line under it. */
  function Noted() {
    return (
      <Menu.Root modal={false}>
        <Menu.Trigger className="ui-button ui-button-ghost" tabIndex={0}>
          More
        </Menu.Trigger>
        <Menu.Portal>
          <Menu.Content className="ui-menu item-menu" aria-label="More">
            <Menu.Label className="ui-menu-label ui-text-label">This chat</Menu.Label>
            <Menu.Label className="ui-menu-label ui-menu-label-named ui-text-secondary">
              devops
            </Menu.Label>
            <Menu.Item className="ui-menu-item" aria-label="Rename…">
              <MenuWords said="Rename…" />
              <MenuKeys>F2</MenuKeys>
            </Menu.Item>
            <Menu.Separator className="ui-menu-line" />
            <Menu.Item className="ui-menu-item ui-menu-item-danger" aria-label="End">
              <MenuWords said="End" under="Ends the program it runs. There is no undo." danger />
            </Menu.Item>
          </Menu.Content>
        </Menu.Portal>
      </Menu.Root>
    );
  }

  async function openNoted(): Promise<HTMLElement> {
    const user = userEvent.setup();
    render(<Noted />);
    await user.tab();
    await user.keyboard("{Enter}");
    return screen.findByRole("menu");
  }

  it("sizes a row to what it says: no fixed height, and never squeezed by the menu", async () => {
    // The menu is a column with a cap on its height. A row whose least height is written down
    // has no automatic one, so a column that is too tall squeezed every row to a control's
    // height and drew each over the next row's note. A row keeps its size and the menu scrolls.
    const menu = await openNoted();
    for (const row of within(menu).getAllByRole("menuitem")) {
      expect(drawnWith(row, "block-size")).toBeUndefined();
      expect(drawnWith(row, "height")).toBeUndefined();
      expect(drawnWith(row, "max-block-size")).toBeUndefined();
      expect(drawnWith(row, "min-block-size")).toBe("var(--size-control)");
      expect(drawnWith(row, "flex-shrink")).toBe("0");
      // One line: its words give way, and nothing wraps under them.
      expect(drawnWith(row, "flex-wrap")).toBeUndefined();
    }
    // Neither is the line between groups nor a group's name squeezed away: nothing in a menu is.
    expect(drawnWith(within(menu).getByRole("separator"), "flex-shrink")).toBe("0");
    expect(drawnWith(within(menu).getByText("This chat"), "flex-shrink")).toBe("0");
  });

  it("writes a row's words, the line under them, its key and a group's name in their text roles", async () => {
    const menu = await openNoted();
    const row = within(menu).getByRole("menuitem", { name: "Rename…" });
    // A row with nothing inside it but text is the body role's size by the row's own rule.
    expect(drawnWith(row, "font-size")).toBe("var(--font-size-md)");
    const words = within(row).getByText("Rename…");
    expect(words).toHaveClass("ui-menu-title", "ui-text-body");
    expect(drawnWith(words, "font-size")).toBe("var(--font-size-md)");
    expect(drawnWith(words, "color")).toBe("var(--text-primary)");
    const keys = within(row).getByText("F2");
    expect(keys).toHaveClass("ui-menu-keys", "ui-text-meta");
    expect(drawnWith(keys, "font-size")).toBe("var(--font-size-xs)");
    expect(drawnWith(keys, "color")).toBe("var(--text-muted)");
    // At the row's end, and never squeezed by the words.
    expect(drawnWith(keys, "margin-inline-start")).toBe("auto");
    expect(drawnWith(keys, "flex")).toBe("none");
    const label = within(menu).getByText("This chat");
    expect(drawnWith(label, "font-size")).toBe("var(--font-size-xs)");
    expect(drawnWith(label, "text-transform")).toBe("uppercase");
    expect(drawnWith(label, "color")).toBe("var(--text-secondary)");
    // **A group named by data is written as the name is** (ruling T4: labels are uppercase,
    // data never): a persona's group wears the secondary role, and no capitals.
    const named = within(menu).getByText("devops");
    expect(named).not.toHaveClass("ui-text-label");
    expect(drawnWith(named, "text-transform")).toBeUndefined();
    expect(drawnWith(named, "letter-spacing")).toBeUndefined();
    expect(drawnWith(named, "font-size")).toBe("var(--font-size-sm)");
    expect(drawnWith(named, "color")).toBe("var(--text-secondary)");
  });

  it("keeps a row's words to one line, and never cuts what is under them", async () => {
    const menu = await openNoted();
    const row = within(menu).getByRole("menuitem", { name: "End" });
    const words = within(row).getByText("End");
    const under = within(row).getByText("Ends the program it runs. There is no undo.");
    expect(drawnWith(words, "white-space")).toBe("nowrap");
    expect(drawnWith(words, "overflow")).toBe("hidden");
    expect(drawnWith(words, "text-overflow")).toBe("ellipsis");
    // **A warning that lost "There is no undo." to an ellipsis, or a reason no keyboard can
    // open, says less than nothing.** What is under a row's words wraps: no rule of any
    // stylesheet cuts it, clips it or keeps it to one line.
    expect(drawnWith(under, "white-space")).toBeUndefined();
    expect(drawnWith(under, "overflow")).toBeUndefined();
    expect(drawnWith(under, "text-overflow")).toBeUndefined();
    expect(drawnWith(under, "-webkit-line-clamp")).toBeUndefined();
    expect(drawnWith(under, "max-block-size")).toBeUndefined();
    expect(drawnWith(under, "overflow-wrap")).toBe("anywhere");
    for (const [selector, body] of RULES)
      if (selector.includes(".ui-menu-note"))
        expect(body, selector).not.toMatch(/nowrap|ellipsis|line-clamp|overflow:\s*hidden/);
    // The two are a column that gives way before the key or the mark at the row's end does.
    const column = words.parentElement as HTMLElement;
    expect(column).toHaveClass("ui-menu-words");
    expect(drawnWith(column, "flex-direction")).toBe("column");
    expect(drawnWith(column, "min-inline-size")).toBe("0");
    // The line under is the secondary role, and stays so on a row in the danger colour; the
    // row's words take the row's colour and wear no role that would set another.
    expect(under).toHaveClass("ui-menu-note", "ui-text-secondary");
    expect(drawnWith(under, "color")).toBe("var(--text-secondary)");
    expect(drawnWith(under, "font-size")).toBe("var(--font-size-sm)");
    expect(words).not.toHaveClass("ui-text-body");
    expect(drawnWith(row, "color")).toBe("var(--danger-base)");
  });

  it("is never taller than the room the window has for it, and scrolls inside", async () => {
    const menu = await openNoted();
    const room = drawnWith(menu, "max-block-size") ?? "";
    expect(room).toContain("--radix-context-menu-content-available-height");
    expect(room).toContain("--radix-dropdown-menu-content-available-height");
    expect(drawnWith(menu, "overflow-y")).toBe("auto");
    // The room is the one cap: a surface's own class no longer writes a share of the window
    // that the menu's place may not have.
    for (const shell of [".more-menu", ".tasks-menu", ".item-menu"]) {
      const body = new RegExp(`(?:^|\\n)\\${shell} \\{([^}]*)\\}`).exec(sheet("App.css"))?.[1];
      expect(body ?? "", shell).not.toMatch(/max-(height|block-size)/);
    }
  });
});

/**
 * **One menu style** (#1834): the three copied shells (`.more-menu`, `.tasks-menu`,
 * `.item-menu`) were the same surface written three times. Every Radix menu in the window now
 * wears `.ui-menu`, and every row `.ui-menu-item`; a surface's own class only sizes it.
 */
describe("every menu in the window", () => {
  const files = Object.entries(
    import.meta.glob("../**/*.tsx", { query: "?raw", import: "default", eager: true }) as Record<
      string,
      string
    >,
  ).filter(([path]) => !path.includes(".test."));

  /** Each `<Kind.Part …>` opening tag's className, by file. */
  function partsOf(pattern: RegExp): { where: string; className: string }[] {
    return files.flatMap(([path, text]) =>
      [...text.matchAll(pattern)].map((hit) => {
        const rest = text.slice(hit.index + hit[0].length);
        const tag = rest.slice(0, rest.search(/\n\s*>|\/>|>\n/) + 1);
        const className = /className=(?:"([^"]*)"|\{([^}]*)\})/.exec(tag);
        return {
          where: `${path}:${text.slice(0, hit.index).split("\n").length}`,
          className: className?.[1] ?? className?.[2] ?? "",
        };
      }),
    );
  }

  it("is found: the menus are where the shells were", () => {
    const found = partsOf(/<(?:Menu|ContextMenu)\.(?:Content|SubContent)\b/g).map(
      ({ where }) => where.split(":")[0],
    );
    expect(new Set(found)).toEqual(
      new Set([
        "../AwaySummary.tsx",
        "../Menus.tsx",
        "../settings/SettingsTab.tsx",
        "../StripMarks.tsx",
        "../TabChip.tsx",
        "../TerminalMenu.tsx",
        "../VaultTab.tsx",
      ]),
    );
  });

  it("wears the one menu style, and every row the one row style", () => {
    const contents = partsOf(/<(?:Menu|ContextMenu)\.(?:Content|SubContent)\b/g);
    const rows = partsOf(/<(?:Menu|ContextMenu)\.(?:Item|SubTrigger)\b/g);
    expect(rows.length).toBeGreaterThan(5);
    const without = (part: { className: string }, name: string) =>
      !part.className.split(/[\s"'`?:]+/).includes(name);
    expect(contents.filter((one) => without(one, "ui-menu")).map(({ where }) => where)).toEqual([]);
    expect(rows.filter((one) => without(one, "ui-menu-item")).map(({ where }) => where)).toEqual(
      [],
    );
  });

  it("is drawn by no shell of its own in App.css", () => {
    const app = sheet("App.css");
    const looks = ["background", "border", "border-radius", "box-shadow", "padding"];
    for (const shell of [".more-menu", ".tasks-menu", ".item-menu"]) {
      const body = new RegExp(`\\n\\${shell} \\{([^}]*)\\}`).exec(app)?.[1] ?? "";
      // Not its width either: the one style has a least and a most width, and a surface that
      // needs another hands the style a custom property, never a width of its own.
      expect(body, `${shell} writes a width`).not.toMatch(
        /(?<![\w-])(?:min-|max-)?(?:width|inline-size):/,
      );
      for (const look of looks)
        expect(body, `${shell} draws ${look}`).not.toMatch(new RegExp(`(?<![\\w-])${look}:`));
    }
    // **The show-more and away menus have no most width**: their rows are tabs and sentences
    // with no part that gives way to an ellipsis, so the one style's most width would wrap or
    // clip them. The one thing their class hands the style.
    expect(new RegExp("\\n\\.more-menu \\{([^}]*)\\}").exec(app)?.[1].trim()).toBe(
      "--ui-menu-max: none;",
    );
    for (const file of ["../AwaySummary.tsx", "../StripMarks.tsx"]) {
      const text = files.find(([path]) => path === file)?.[1] ?? "";
      expect(text, file).toMatch(/<Menu\.Content[^>]*className="ui-menu more-menu/);
    }
    for (const gone of [".more-tab", ".menu-row", ".tasks-menu-line", ".menu-sub-mark"])
      expect(app, gone).not.toContain(`${gone} {`);
  });
});
