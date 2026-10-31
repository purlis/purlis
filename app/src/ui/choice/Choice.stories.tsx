// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The choice families** (#2907): the checkbox and its chips, the radio group, the switch, the
 * segmented control, the slider and the option cards, each drawn in every state that draws
 * differently, held to axe, walked by the keyboard and compared with its baseline in Chromium
 * and WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type ReactNode } from "react";
import { expect, waitFor } from "storybook/test";
import { Bot, Code, SquareTerminal } from "lucide-react";
import { cellInTest } from "../../../.storybook/matrix";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Checkbox, CheckboxGroup, RovingCheckbox } from "./Checkbox";
import { OptionCard, OptionCards, Radio, RadioGroup, Segment, SegmentedControl } from "./Radio";
import { Slider } from "./Slider";
import { Switch } from "./Switch";
import { Button } from "../button/Button";

/** A column the story's controls stand in, as a form's rows do. */
function Column({ children }: { children: ReactNode }) {
  return (
    <div style={{ display: "grid", gap: "var(--space-4)", justifyItems: "start", width: 360 }}>
      {children}
    </div>
  );
}

function Boxes() {
  const [sign, setSign] = useState(false);
  const [save, setSave] = useState(true);
  return (
    <Column>
      <Checkbox isSelected={sign} onChange={setSign}>
        Sign commits
      </Checkbox>
      <Checkbox
        isSelected={save}
        onChange={setSave}
        description="Save on its own after a quiet spell."
      >
        Auto-save
      </Checkbox>
      <Checkbox isIndeterminate>Some of the repos</Checkbox>
      <Checkbox isDisabled>Unavailable</Checkbox>
      <Checkbox isDisabled isSelected>
        Unavailable and ticked
      </Checkbox>
    </Column>
  );
}

/** Where each element is drawn: left, top, width and height, rounded to a tenth of a px. */
const placed = (elements: readonly Element[]) =>
  elements.map((one) => {
    const box = one.getBoundingClientRect();
    return [box.left, box.top, box.width, box.height].map((n) => Math.round(n * 10) / 10);
  });

/** How tall a length is drawn here, in px: a token's value in this cell of the matrix. */
function drawnHeight(length: string): number {
  const probe = document.createElement("div");
  probe.style.blockSize = length;
  document.body.append(probe);
  const height = probe.getBoundingClientRect().height;
  probe.remove();
  return height;
}

/** A CSS time, in ms. */
const inMs = (time: string) =>
  time.trim().endsWith("ms") ? Number.parseFloat(time) : Number.parseFloat(time) * 1000;

/** px at the default text size, as this window draws them: a rem is 14 of them. */
const px = (at14: number) =>
  (at14 / 14) * Number.parseFloat(getComputedStyle(document.documentElement).fontSize);

const meta = {
  component: Boxes,
} satisfies Meta<typeof Boxes>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab reaches each box in turn; Space ticks it and ticks it back; a tick moves nothing. */
export const Checkboxes: Story = {
  play: async ({ canvas, userEvent }) => {
    const sign = canvas.getByRole("checkbox", { name: "Sign commits" });
    const save = canvas.getByRole("checkbox", { name: "Auto-save" });
    const rows = canvas.getAllByRole("checkbox").map((one) => one.closest("label") as HTMLElement);
    const before = placed(rows);
    await userEvent.tab();
    await expect(sign).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(sign).toBeChecked());
    // A tick moves nothing: neither its own row nor the one under it.
    await expect(placed(rows)).toEqual(before);
    await userEvent.keyboard(" ");
    await waitFor(() => expect(sign).not.toBeChecked());
    await userEvent.tab();
    await expect(save).toHaveFocus();
    await expect(save).toHaveAccessibleDescription("Save on its own after a quiet spell.");
    await expect(canvas.getByRole("checkbox", { name: "Unavailable" })).toBeDisabled();
  },
};

function Chips() {
  const [on, setOn] = useState<ReadonlySet<string>>(new Set(["working"]));
  const tick = (value: string, to: boolean) =>
    setOn((was) => {
      const next = new Set(was);
      if (to) next.add(value);
      else next.delete(value);
      return next;
    });
  return (
    <CheckboxGroup roving aria-label="Show">
      {["needs you", "working", "done"].map((value, at) => (
        <RovingCheckbox
          key={value}
          at={at}
          variant="chip"
          // A count after the words (#3484), and none on a chip with nothing to count.
          count={[2, 3, undefined][at]}
          isSelected={on.has(value)}
          onChange={(to) => tick(value, to)}
        >
          {value}
        </RovingCheckbox>
      ))}
    </CheckboxGroup>
  );
}

/** One Tab stop for the row; the arrows move along it; Space ticks the chip the keyboard is on.
 *  A chip's count follows its words and describes it, never part of its name (#3484). */
export const ChipsOverAList: Story = {
  render: () => <Chips />,
  play: async ({ canvas, userEvent }) => {
    const needs = canvas.getByRole("checkbox", { name: "needs you" });
    const working = canvas.getByRole("checkbox", { name: "working" });
    await expect(needs).toHaveAccessibleDescription("2");
    await userEvent.tab();
    await expect(needs).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => working, "Right moves the keyboard to working");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(working).not.toBeChecked());
    await userEvent.keyboard(" ");
    await waitFor(() => expect(working).toBeChecked());
    // Tab leaves the row in one press.
    await userEvent.tab();
    await expect(canvas.getByRole("group", { name: "Show" })).not.toContainElement(
      document.activeElement as HTMLElement | null,
    );
    await userEvent.tab({ shift: true });
    await expect(working).toHaveFocus();
  },
};

function Radios() {
  const [value, setValue] = useState("comfortable");
  return (
    <RadioGroup aria-label="Density" value={value} onChange={setValue}>
      <Radio value="comfortable" description="Rows and controls at their full height.">
        Comfortable
      </Radio>
      <Radio value="compact" description="Rows and controls a step shorter.">
        Compact
      </Radio>
      <Radio value="spacious" isDisabled description="Unavailable here.">
        Spacious
      </Radio>
    </RadioGroup>
  );
}

/** One Tab stop on the option picked; an arrow moves the pick with the focus, past one that is
 *  unavailable. */
export const RadioGroupOfOptions: Story = {
  render: () => <Radios />,
  play: async ({ canvas, userEvent }) => {
    const comfortable = canvas.getByRole("radio", { name: "Comfortable" });
    const compact = canvas.getByRole("radio", { name: "Compact" });
    await userEvent.tab();
    await expect(comfortable).toHaveFocus();
    await expect(comfortable).toHaveAccessibleDescription(
      "Rows and controls at their full height.",
    );
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(compact).toBeChecked());
    await expect(compact).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(comfortable).toBeChecked());
  },
};

function Switches() {
  const [on, setOn] = useState(false);
  return (
    <Column>
      <Switch isSelected={on} onChange={setOn}>
        Auto-save
      </Switch>
      <Switch isSelected>Sign commits</Switch>
      <Switch isDisabled>Unavailable</Switch>
      <Switch isDisabled isSelected>
        Unavailable and on
      </Switch>
    </Column>
  );
}

/** Space turns it on and off; on, its thumb is at the end; turning it moves nothing. */
export const SwitchesOnAndOff: Story = {
  render: () => <Switches />,
  play: async ({ canvas, userEvent }) => {
    const auto = canvas.getByRole("switch", { name: "Auto-save" });
    const rows = canvas.getAllByRole("switch").map((one) => one.closest("label") as HTMLElement);
    const row = rows[0];
    const track = row.querySelector('[data-part="track"]') as HTMLElement;
    const thumb = row.querySelector('[data-part="thumb"]') as HTMLElement;
    // R63, whatever the theme's control height and durations: a 34 x 20 pill, a 14px thumb 3px
    // in from its edge, sliding in 150ms (none under reduced motion).
    const pill = track.getBoundingClientRect();
    await expect(pill.width).toBeCloseTo(px(34), 0);
    await expect(pill.height).toBeCloseTo(px(20), 0);
    const knob = thumb.getBoundingClientRect();
    await expect(knob.width).toBeCloseTo(px(14), 0);
    await expect(knob.height).toBeCloseTo(px(14), 0);
    await expect(knob.left - pill.left).toBeCloseTo(px(3), 0);
    await expect(knob.top - pill.top).toBeCloseTo(px(3), 0);
    // Its duration is the knob's token, `quick`: 150ms in Purlis.
    const reduced = document.documentElement.dataset.motion === "reduced";
    if (cellInTest()?.theme === "purlis")
      await expect(inMs(getComputedStyle(thumb).transitionDuration)).toBe(reduced ? 0 : 150);
    const before = placed(rows);
    await userEvent.tab();
    await expect(auto).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(auto).toBeChecked());
    // On, the thumb ends 3px in from the track's other end.
    await waitFor(() =>
      expect(pill.right - thumb.getBoundingClientRect().right).toBeCloseTo(px(3), 0),
    );
    // Turning it moves nothing: neither its own row nor the ones under it.
    await expect(placed(rows)).toEqual(before);
    await userEvent.keyboard(" ");
    await waitFor(() => expect(auto).not.toBeChecked());
  },
};

function Segments({ size }: { size: "sm" | "md" }) {
  const [level, setLevel] = useState("you");
  return (
    <SegmentedControl size={size} aria-label="Level" value={level} onChange={setLevel}>
      <Segment value="you">You</Segment>
      <Segment value="project" count={2}>
        Project
      </Segment>
      <Segment value="workspace" count={3}>
        Workspace
      </Segment>
    </SegmentedControl>
  );
}

/** One Tab stop; the arrows move the pick along; a pick changes no segment's width. */
const segmentsByKeyboard: Story["play"] = async ({ canvas, userEvent }) => {
  const you = canvas.getByRole("radio", { name: "You" });
  const project = canvas.getByRole("radio", { name: "Project 2" });
  const group = canvas.getByRole("radiogroup", { name: "Level" });
  // Its size's height, as the button family draws the same size: md the control's, sm the
  // chrome's and never under 24px.
  const tall =
    group.dataset.size === "sm"
      ? drawnHeight("max(24px, var(--size-chrome), var(--space-7))")
      : drawnHeight("var(--size-control)");
  await expect(group.getBoundingClientRect().height).toBeCloseTo(tall, 0);
  const segments = canvas.getAllByRole("radio").map((one) => one.closest("label") as Element);
  const before = placed(segments);
  await userEvent.tab();
  await expect(you).toHaveFocus();
  await userEvent.keyboard("{ArrowRight}");
  await waitFor(() => expect(project).toBeChecked());
  await expect(project).toHaveFocus();
  expectFocusInSight();
  // A pick moves no segment.
  await expect(placed(segments)).toEqual(before);
};

export const SegmentedControlMd: Story = {
  render: () => <Segments size="md" />,
  play: segmentsByKeyboard,
};

export const SegmentedControlSm: Story = {
  render: () => <Segments size="sm" />,
  play: segmentsByKeyboard,
};

/**
 * **Every choice beside a small button** (#4173, WCAG 2.5.8): the compact density draws the
 * chrome at 18px, so a choice's row and a small segmented control take the sm button's floor, 24
 * CSS px. Each is a 24px target by itself in every cell: a box's, a chip's, a radio's and a
 * switch's row is its label, which takes the press; a slider's press lands on its area, which is
 * 24px tall around the track; and a segment's reaches through the control's padding. Each is
 * hit-tested at its edges, so axe's `target-size` passes with no spacing to lean on.
 */
function BesideASmallButton() {
  const [level, setLevel] = useState("you");
  return (
    <div style={{ display: "grid", gap: "var(--space-1)", justifyItems: "start", width: 360 }}>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1)" }}>
        <Checkbox>Sign commits</Checkbox>
        <Button size="sm">Apply</Button>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1)" }}>
        <Checkbox variant="chip">working</Checkbox>
        <Switch>Auto-save</Switch>
      </div>
      <RadioGroup aria-label="Density" value="compact" onChange={() => {}}>
        <Radio value="compact">Compact</Radio>
      </RadioGroup>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1)" }}>
        <SegmentedControl size="sm" aria-label="Level" value={level} onChange={setLevel}>
          <Segment value="you">You</Segment>
          <Segment value="workspace">Workspace</Segment>
        </SegmentedControl>
        <Button size="sm">Reset</Button>
      </div>
      <Slider
        label="Window text size"
        minValue={10}
        maxValue={24}
        value={14}
        onChange={() => {}}
        shown={(at) => `${at}px`}
        spoken={(at) => `${at} pixels`}
      />
    </div>
  );
}

export const ChoicesBesideASmallButton: Story = {
  render: () => <BesideASmallButton />,
  play: async ({ canvas, canvasElement }) => {
    const rows = [
      ...canvasElement.querySelectorAll<HTMLElement>(
        '[data-scope="checkbox"][data-part="root"], [data-scope="radio"][data-part="root"], [data-scope="switch"][data-part="root"], [data-scope="slider"][data-part="root"]',
      ),
    ];
    // A box, a chip, a radio, a switch and a slider.
    await expect(rows).toHaveLength(5);
    for (const one of [...rows, ...canvas.getAllByRole("button")])
      await expect(one.getBoundingClientRect().height).toBeGreaterThanOrEqual(24);
    // The slider's press lands on its area, which is the target, not its row: a press at the
    // area's top and bottom edges is the area's.
    const area = canvasElement.querySelector<HTMLElement>(
      '[data-scope="slider"][data-part="area"]',
    ) as HTMLElement;
    const pressed = area.getBoundingClientRect();
    await expect(pressed.height).toBeGreaterThanOrEqual(24);
    for (const y of [pressed.top + 0.5, pressed.bottom - 0.5]) {
      const hit = document.elementFromPoint(pressed.left + pressed.width / 2, y);
      await expect(hit?.closest('[data-scope="slider"][data-part="area"]')).toBe(area);
    }
    // A segment is drawn inside the control's padding; a press lands on it through the padding,
    // so its target is the control's whole height, at least 24px.
    const control = canvas.getByRole("radiogroup", { name: "Level" });
    const outer = control.getBoundingClientRect();
    await expect(outer.height).toBeGreaterThanOrEqual(24);
    const segments = [
      ...control.querySelectorAll<HTMLElement>('[data-scope="segment"][data-part="root"]'),
    ];
    await expect(segments).toHaveLength(2);
    for (const one of segments) {
      const x = one.getBoundingClientRect().left + one.getBoundingClientRect().width / 2;
      for (const y of [outer.top + 0.5, outer.bottom - 0.5]) {
        const hit = document.elementFromPoint(x, y);
        await expect(hit?.closest('[data-scope="segment"][data-part="root"]')).toBe(one);
      }
    }
  },
};

/**
 * **A choice with no words is a 24px target across too** (#4197, WCAG 2.5.8): a row's selection
 * box, a radio or a switch named by `aria-label` is drawn its mark's size (a 14px box), and its
 * press reaches a 24 by 24 square centred on the mark (#4303), which changes nothing it draws.
 * Each is hit-tested at that square's corners, measured from the mark.
 */
function Unlabelled() {
  // Far enough apart that one's square is not another's, and in from the page's edge, so each
  // square's corners are on the page.
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-8)",
        padding: "var(--space-8)",
      }}
    >
      <Checkbox aria-label="Choose the row" />
      <RadioGroup aria-label="Density" value="compact" onChange={() => {}}>
        <Radio value="compact" aria-label="Compact" />
      </RadioGroup>
      <Switch aria-label="Auto-save" />
    </div>
  );
}

export const ChoicesWithNoWords: Story = {
  render: () => <Unlabelled />,
  play: async ({ canvas }) => {
    for (const control of [
      canvas.getByRole("checkbox", { name: "Choose the row" }),
      canvas.getByRole("radio", { name: "Compact" }),
      canvas.getByRole("switch", { name: "Auto-save" }),
    ]) {
      const root = control.closest('[data-part="root"]') as HTMLElement;
      const mark = root.querySelector(
        '[data-part="box"], [data-part="mark"], [data-part="track"]',
      ) as HTMLElement;
      // Measured from the mark, not its root, which may keep room for words it has none of (#4303).
      const drawn = mark.getBoundingClientRect();
      const [x, y] = [drawn.left + drawn.width / 2, drawn.top + drawn.height / 2];
      // The corners of the 24 by 24 square around the mark's centre, half a px in.
      for (const [dx, dy] of [
        [-11.5, -11.5],
        [11.5, -11.5],
        [-11.5, 11.5],
        [11.5, 11.5],
      ]) {
        const hit = document.elementFromPoint(x + dx, y + dy);
        await expect(hit?.closest('[data-part="root"]'), root.dataset.scope).toBe(root);
      }
    }
  },
};

function TextSize() {
  const [px, setPx] = useState(14);
  return (
    <div style={{ width: 360 }}>
      <Slider
        label="Window text size"
        minValue={10}
        maxValue={24}
        value={px}
        onChange={setPx}
        shown={(at) => `${at}px`}
        spoken={(at) => `${at} pixels`}
      />
    </div>
  );
}

/** The arrows step it, Home and End go to its ends, and it says its value in words. */
export const Sliders: Story = {
  render: () => <TextSize />,
  play: async ({ canvas, userEvent }) => {
    const slider = canvas.getByRole("slider", { name: "Window text size" });
    await userEvent.tab();
    await expect(slider).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(slider).toHaveValue("15"));
    await waitFor(() => expect(slider).toHaveAttribute("aria-valuetext", "15 pixels"));
    await userEvent.keyboard("{End}");
    await waitFor(() => expect(slider).toHaveValue("24"));
    await userEvent.keyboard("{Home}");
    await waitFor(() => expect(slider).toHaveValue("10"));
  },
};

const HARNESSES = [
  {
    id: "claude",
    icon: <Bot />,
    detail: "Billed to whatever Claude Code is signed in with. The harness's default model.",
  },
  {
    id: "codex",
    icon: <Code />,
    detail: "Billed to whatever Codex is signed in with.",
  },
  {
    id: "shell",
    icon: <SquareTerminal />,
    detail: "A shell in the workspace, no harness.",
  },
] as const;

function Harnesses() {
  const [picked, setPicked] = useState("claude");
  return (
    <div style={{ width: 360 }}>
      <OptionCards aria-label="Harness" value={picked} onChange={setPicked}>
        {HARNESSES.map((one) => (
          <OptionCard
            key={one.id}
            value={one.id}
            title={one.id}
            icon={one.icon}
            detail={one.detail}
          />
        ))}
      </OptionCards>
    </div>
  );
}

/** Equal-height cards, no radio circle; the arrows move the pick, which is told by its edge and
 *  its check as well as its wash. */
export const OptionCardsOfHarnesses: Story = {
  render: () => <Harnesses />,
  play: async ({ canvas, userEvent }) => {
    const claude = canvas.getByRole("radio", { name: "claude" });
    const codex = canvas.getByRole("radio", { name: "codex" });
    const cards = canvas.getAllByRole("radio").map((one) => one.closest("label") as Element);
    await expect(new Set(placed(cards).map(([, , , height]) => height)).size).toBe(1);
    const before = placed(cards);
    await userEvent.tab();
    await expect(claude).toHaveFocus();
    await expect(claude).toHaveAccessibleDescription(HARNESSES[0].detail);
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(codex).toBeChecked());
    expectFocusInSight();
    // A pick moves no card.
    await expect(placed(cards)).toEqual(before);
  },
};

function Tiles() {
  const [picked, setPicked] = useState("");
  return (
    <OptionCards
      variant="tile"
      aria-label="Icon"
      orientation="horizontal"
      value={picked}
      onChange={setPicked}
    >
      <OptionCard variant="tile" value="" aria-label="Initials" icon="ST" />
      <OptionCard variant="tile" value="bot" aria-label="bot" icon={<Bot />} />
      <OptionCard variant="tile" value="code" aria-label="code" icon={<Code />} />
    </OptionCards>
  );
}

/** A row of tiles, each named by its label; the arrows move the pick. */
export const TilesOfAMark: Story = {
  render: () => <Tiles />,
  play: async ({ canvas, userEvent }) => {
    const initials = canvas.getByRole("radio", { name: "Initials" });
    await userEvent.tab();
    await expect(initials).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(canvas.getByRole("radio", { name: "bot" })).toBeChecked());
    expectFocusInSight();
  },
};
