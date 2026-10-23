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
import * as RadioGroup from "@radix-ui/react-radio-group";
import * as Select from "@radix-ui/react-select";
import * as Switch from "@radix-ui/react-switch";
import { drawnWith } from "../cascade.testkit";
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
    // may set for it: none, now that a menu's row is its family's (`ui/menu/menu.css`).
    const own = new Set([...CSS.matchAll(/^\s+(--[\w-]+):/gm)].map((hit) => hit[1]));
    const fromViews = new Set<string>();
    for (const name of fromViews) expect(sheet("App.css")).toContain(`${name}:`);
    const fromRadix = new Set([
      "--radix-select-trigger-width",
      "--radix-select-content-available-height",
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
