// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The choice families' wrappers** (#2907): what a person presses and hears, and the attributes
 * a theme's parts are keyed on (ADR 0103). The look itself is held by the stories, in a real
 * browser (`Choice.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { Checkbox, CheckboxGroup, RovingCheckbox } from "./Checkbox";
import { OptionCard, OptionCards, Radio, RadioGroup, Segment, SegmentedControl } from "./Radio";
import { Slider } from "./Slider";
import { Switch } from "./Switch";
import { ColorWell } from "./ColorWell";
import { drawnWith } from "../../cascade.testkit";

afterEach(cleanup);

/** A colour as the well holds it, `#rrggbb`, from its six digits. */
const hex = (digits: string) => `#${digits}`;

/** The root a control's parts are drawn inside: its label. */
const rootOf = (control: HTMLElement) => control.closest("label") as HTMLElement;

describe("a checkbox", () => {
  it("is a native box in its own label, ticked by its words and by Space", async () => {
    const changed = vi.fn();
    render(<Checkbox onChange={changed}>Sign commits</Checkbox>);
    const box = screen.getByRole("checkbox", { name: "Sign commits" });
    expect(box.tagName).toBe("INPUT");
    await userEvent.click(screen.getByText("Sign commits"));
    expect(changed).toHaveBeenLastCalledWith(true);
    box.focus();
    await userEvent.keyboard(" ");
    expect(changed).toHaveBeenLastCalledWith(false);
  });

  it("carries its parts and its variant, and its state on its root", () => {
    render(
      <Checkbox isSelected variant="chip">
        working
      </Checkbox>,
    );
    const root = rootOf(screen.getByRole("checkbox", { name: "working" }));
    expect(root).toHaveAttribute("data-scope", "checkbox");
    expect(root).toHaveAttribute("data-part", "root");
    expect(root).toHaveAttribute("data-variant", "chip");
    expect(root).toHaveAttribute("data-selected", "true");
    expect(root.querySelector('[data-part="box"]')).not.toBeNull();
    expect(root.querySelector('[data-part="label"]')).toHaveTextContent("working");
  });

  it("takes the id on its box, so a row's label names it", () => {
    render(
      <>
        <label htmlFor="footer">Draw purlis's footer</label>
        <Checkbox id="footer" />
      </>,
    );
    const box = screen.getByRole("checkbox", { name: "Draw purlis's footer" });
    expect(box).toHaveAttribute("id", "footer");
    expect(box).toHaveAttribute("tabindex", "0");
  });

  it("draws a chip's count after its words, said once and never part of its name (#3484)", () => {
    render(
      <Checkbox variant="chip" count={2}>
        needs you
      </Checkbox>,
    );
    const box = screen.getByRole("checkbox", { name: "needs you" });
    expect(rootOf(box).querySelector('[data-part="count"]')).toHaveTextContent("2");
    expect(box).toHaveAccessibleDescription("2");
  });

  it("is described by its line, which is not part of its name", () => {
    render(<Checkbox description="Save on its own after a quiet spell.">Auto-save</Checkbox>);
    const box = screen.getByRole("checkbox", { name: "Auto-save" });
    expect(box).toHaveAccessibleDescription("Save on its own after a quiet spell.");
  });
});

describe("a roving group of chips", () => {
  function Chips() {
    const [on, setOn] = useState<string[]>([]);
    return (
      <CheckboxGroup roving aria-label="Show">
        {["needs you", "working", "done"].map((value, at) => (
          <RovingCheckbox
            key={value}
            at={at}
            variant="chip"
            isSelected={on.includes(value)}
            onChange={(to) =>
              setOn((was) => (to ? [...was, value] : was.filter((one) => one !== value)))
            }
          >
            {value}
          </RovingCheckbox>
        ))}
      </CheckboxGroup>
    );
  }

  it("is one Tab stop, with the arrows between its chips and Space ticking", async () => {
    render(
      <>
        <Chips />
        <button type="button">after</button>
      </>,
    );
    const [needs, working, done] = ["needs you", "working", "done"].map((name) =>
      screen.getByRole("checkbox", { name }),
    );
    expect(screen.getByRole("group", { name: "Show" })).toBeInTheDocument();
    await userEvent.tab();
    expect(needs).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(working).toHaveFocus();
    await userEvent.keyboard(" ");
    expect(working).toBeChecked();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    expect(needs).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(done).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "after" })).toHaveFocus();
    // Back in, on the chip the keyboard was last on.
    await userEvent.tab({ shift: true });
    expect(done).toHaveFocus();
  });
});

describe("a pointer's press on what a choice draws", () => {
  // A person's click carries a pointer, which React Aria takes on the visible label; the
  // WebView driver's pointerless click is a virtual press, which it leaves to the input, and
  // the scenario specs click the input for that reason (#2907). A real click on the words, the
  // box or the card still chooses.
  it("ticks a box from its label, its box and its words", async () => {
    render(<Checkbox>Sign commits</Checkbox>);
    const box = screen.getByRole("checkbox", { name: "Sign commits" });
    const root = rootOf(box);
    await userEvent.click(root);
    expect(box).toBeChecked();
    await userEvent.click(root.querySelector('[data-part="box"]') as HTMLElement);
    expect(box).not.toBeChecked();
    await userEvent.click(screen.getByText("Sign commits"));
    expect(box).toBeChecked();
  });

  it("turns a switch from its track and its words", async () => {
    render(<Switch>Auto-save</Switch>);
    const toggle = screen.getByRole("switch", { name: "Auto-save" });
    await userEvent.click(rootOf(toggle).querySelector('[data-part="track"]') as HTMLElement);
    expect(toggle).toBeChecked();
    await userEvent.click(screen.getByText("Auto-save"));
    expect(toggle).not.toBeChecked();
  });

  it("picks a radio and an option card from what they draw", async () => {
    function Both() {
      const [radio, setRadio] = useState<string | null>(null);
      const [card, setCard] = useState<string | null>(null);
      return (
        <>
          <RadioGroup aria-label="Density" value={radio} onChange={setRadio}>
            <Radio value="compact">Compact</Radio>
          </RadioGroup>
          <OptionCards aria-label="Harness" value={card} onChange={setCard}>
            <OptionCard value="codex" title="codex" detail="Billed to Codex." />
          </OptionCards>
        </>
      );
    }
    render(<Both />);
    await userEvent.click(screen.getByText("Compact"));
    expect(screen.getByRole("radio", { name: "Compact" })).toBeChecked();
    await userEvent.click(screen.getByText("Billed to Codex."));
    expect(screen.getByRole("radio", { name: "codex" })).toBeChecked();
  });
});

describe("a roving group whose stop cannot be used", () => {
  function Row({ off }: { off: readonly string[] }) {
    return (
      <CheckboxGroup roving aria-label="Show">
        {["needs you", "working", "done"].map((value, at) => (
          <RovingCheckbox key={value} at={at} variant="chip" isDisabled={off.includes(value)}>
            {value}
          </RovingCheckbox>
        ))}
      </CheckboxGroup>
    );
  }

  it("gives the Tab stop to the first box that can be used", async () => {
    render(<Row off={["needs you"]} />);
    await userEvent.tab();
    expect(screen.getByRole("checkbox", { name: "working" })).toHaveFocus();
  });

  it("moves the stop when the box that held it can no longer be used", async () => {
    const { rerender } = render(<Row off={[]} />);
    await userEvent.tab();
    await userEvent.keyboard("{End}");
    expect(screen.getByRole("checkbox", { name: "done" })).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(screen.getByRole("checkbox", { name: "needs you" })).toHaveFocus();
    await userEvent.tab();
    rerender(<Row off={["needs you"]} />);
    await userEvent.tab({ shift: true });
    expect(screen.getByRole("checkbox", { name: "working" })).toHaveFocus();
  });
});

describe("a radio group", () => {
  it("is one group of native radios, each described by its line", async () => {
    const picked = vi.fn();
    render(
      <RadioGroup aria-label="Density" value="comfortable" onChange={picked}>
        <Radio value="comfortable" description="Full height.">
          Comfortable
        </Radio>
        <Radio value="compact">Compact</Radio>
      </RadioGroup>,
    );
    expect(screen.getByRole("radiogroup", { name: "Density" })).toBeInTheDocument();
    const comfortable = screen.getByRole("radio", { name: "Comfortable" });
    expect(comfortable).toBeChecked();
    expect(comfortable).toHaveAccessibleDescription("Full height.");
    expect(rootOf(comfortable)).toHaveAttribute("data-scope", "radio");
    expect(rootOf(comfortable)).toHaveAttribute("data-selected", "true");
    await userEvent.click(screen.getByText("Compact"));
    expect(picked).toHaveBeenCalledWith("compact");
  });
});

describe("a segmented control", () => {
  it("is a radio group of segments at its size, a count in a segment's name", () => {
    render(
      <SegmentedControl size="sm" aria-label="Level" value="you" onChange={() => {}}>
        <Segment value="you">You</Segment>
        <Segment value="workspace" count={3}>
          Workspace
        </Segment>
      </SegmentedControl>,
    );
    const group = screen.getByRole("radiogroup", { name: "Level" });
    expect(group).toHaveAttribute("data-scope", "segmented-control");
    expect(group).toHaveAttribute("data-size", "sm");
    const workspace = screen.getByRole("radio", { name: "Workspace 3" });
    expect(rootOf(workspace)).toHaveAttribute("data-scope", "segment");
    expect(rootOf(workspace).querySelector('[data-part="count"]')).toHaveTextContent("3");
  });
});

describe("option cards", () => {
  it("are radios named by their title and described by their detail, with no circle", () => {
    render(
      <OptionCards aria-label="Harness" value="claude" onChange={() => {}}>
        <OptionCard value="claude" title="claude" detail="Billed to Claude Code." />
        <OptionCard value="codex" title="codex" detail="Billed to Codex." />
      </OptionCards>,
    );
    const claude = screen.getByRole("radio", { name: "claude" });
    expect(claude).toHaveAccessibleDescription("Billed to Claude Code.");
    const root = rootOf(claude);
    expect(root).toHaveAttribute("data-scope", "option-card");
    expect(root).toHaveAttribute("data-variant", "card");
    expect(root.querySelector('[data-part="check"]')).not.toBeNull();
    expect(root.querySelector('[data-scope="radio"]')).toBeNull();
  });

  it("draw a tile as its icon alone, named by its label", () => {
    render(
      <OptionCards variant="tile" aria-label="Icon" value="" onChange={() => {}}>
        <OptionCard variant="tile" value="" aria-label="Initials" icon="ST" />
      </OptionCards>,
    );
    const tile = rootOf(screen.getByRole("radio", { name: "Initials" }));
    expect(tile).toHaveAttribute("data-variant", "tile");
    expect(tile.querySelector('[data-part="check"]')).toBeNull();
  });
});

describe("a switch", () => {
  it("is a native switch in its own label, its track and thumb inside", async () => {
    const changed = vi.fn();
    render(<Switch onChange={changed}>Auto-save</Switch>);
    const toggle = screen.getByRole("switch", { name: "Auto-save" });
    const root = rootOf(toggle);
    expect(root).toHaveAttribute("data-scope", "switch");
    expect(root.querySelector('[data-part="track"] [data-part="thumb"]')).not.toBeNull();
    await userEvent.click(toggle);
    expect(changed).toHaveBeenCalledWith(true);
  });
});

describe("a slider", () => {
  it("is named by what names it, and says its value in words", () => {
    const changed = vi.fn();
    render(
      <>
        <span>Window text size</span>
        <Slider
          id="size"
          label="Window text size"
          minValue={10}
          maxValue={24}
          value={14}
          onChange={changed}
          shown={(px) => `${px}px`}
          spoken={(px) => `${px} pixels`}
        />
      </>,
    );
    const slider = screen.getByRole("slider", { name: "Window text size" });
    expect(slider).toHaveValue("14");
    expect(slider).toHaveAttribute("aria-valuetext", "14 pixels");
    fireEvent.change(slider, { target: { value: "18" } });
    expect(changed).toHaveBeenCalledWith(18);
    // The id is the root's, and a focus given to it is the thumb's.
    document.getElementById("size")?.focus();
    expect(slider).toHaveFocus();
  });
});

describe("a colour well", () => {
  it("hands back the colour the picker settled on once", () => {
    const committed = vi.fn();
    const drafted = vi.fn();
    render(
      <ColorWell
        aria-label="Colour"
        value={hex("112233")}
        onDraft={drafted}
        onCommit={committed}
      />,
    );
    const well = screen.getByLabelText("Colour") as HTMLInputElement;
    fireEvent.input(well, { target: { value: hex("445566") } });
    expect(drafted).toHaveBeenCalledWith(hex("445566"));
    fireEvent.change(well, { target: { value: hex("445566") } });
    fireEvent.blur(well);
    expect(committed).toHaveBeenCalledTimes(1);
  });
});

// WCAG 2.5.8 (#4173): the compact density draws the chrome at 18px, so every choice's row and a
// small segmented control take the sm button's floor, 24 CSS px, as a field's does (#3250).
describe("a choice's target size (#4173)", () => {
  const FLOOR = "max(24px, var(--size-chrome), var(--space-7))";

  it("never draws a choice row under the 24px target, in any density or text size", () => {
    render(
      <>
        <Checkbox>Sign commits</Checkbox>
        <Checkbox variant="chip">working</Checkbox>
        <RadioGroup aria-label="Density" value="compact" onChange={() => {}}>
          <Radio value="compact">Compact</Radio>
        </RadioGroup>
        <Switch>Auto-save</Switch>
        <Slider
          label="Window text size"
          minValue={10}
          maxValue={24}
          value={14}
          onChange={() => {}}
          shown={(px) => `${px}px`}
          spoken={(px) => `${px} pixels`}
        />
      </>,
    );
    for (const [role, name] of [
      ["checkbox", "Sign commits"],
      ["checkbox", "working"],
      ["radio", "Compact"],
      ["switch", "Auto-save"],
    ] as const) {
      const root = rootOf(screen.getByRole(role, { name }));
      expect(drawnWith(root, "min-block-size"), name).toBe(FLOOR);
    }
    const slider = screen
      .getByRole("slider", { name: "Window text size" })
      .closest('[data-scope="slider"][data-part="root"]') as Element;
    expect(drawnWith(slider, "min-block-size")).toBe(FLOOR);
  });

  it("never draws a small segmented control under the 24px target", () => {
    render(
      <SegmentedControl size="sm" aria-label="Level" value="you" onChange={() => {}}>
        <Segment value="you">You</Segment>
        <Segment value="workspace">Workspace</Segment>
      </SegmentedControl>,
    );
    expect(drawnWith(screen.getByRole("radiogroup", { name: "Level" }), "block-size")).toBe(FLOOR);
  });

  // A segment sits inside the control's padding, so it is drawn under the control (20px in a 24px
  // control). Its press reaches through that padding by the same length the padding is, one
  // custom property both read, so a theme's padding (`segmented-control.root.padding`, which the
  // engine writes into it too) moves the reach with it. The story hit-tests it in the browser.
  it("pads the control and reaches a segment's press by one length, the control's padding", () => {
    render(
      <SegmentedControl size="sm" aria-label="Level" value="you" onChange={() => {}}>
        <Segment value="you">You</Segment>
      </SegmentedControl>,
    );
    const group = screen.getByRole("radiogroup", { name: "Level" });
    expect(drawnWith(group, "--segment-reach")).toBe("var(--space-1)");
    expect(drawnWith(group, "padding")).toBe("var(--segment-reach)");
  });

  // The slider's press lands on its area, not its row (#4173): the area is the 24px target, its
  // track and thumb centred in it, so nothing it draws moves.
  it("never draws the slider's press area under the 24px target", () => {
    render(
      <Slider
        label="Window text size"
        minValue={10}
        maxValue={24}
        value={14}
        onChange={() => {}}
        shown={(px) => `${px}px`}
        spoken={(px) => `${px} pixels`}
      />,
    );
    const area = document.querySelector('[data-scope="slider"][data-part="area"]') as Element;
    expect(drawnWith(area, "block-size")).toBe(FLOOR);
  });
});

// WCAG 2.5.8 (#4197): a choice with no words is drawn its mark's size, 14px across; its press
// reaches a 24 by 24 square around it, placed on its root. The story hit-tests the square.
describe("a choice with no words (#4197)", () => {
  it("places the press it reaches past its mark on its root", () => {
    render(
      <>
        <Checkbox aria-label="Choose the row" />
        <RadioGroup aria-label="Density" value="compact" onChange={() => {}}>
          <Radio value="compact" aria-label="Compact" />
        </RadioGroup>
        <Switch aria-label="Auto-save" />
      </>,
    );
    for (const [role, name] of [
      ["checkbox", "Choose the row"],
      ["radio", "Compact"],
      ["switch", "Auto-save"],
    ] as const)
      expect(drawnWith(rootOf(screen.getByRole(role, { name })), "position"), name).toBe(
        "relative",
      );
  });
});
