// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The panel separator family** (#3143): a side's edge in the window's gutter and a split
 * between two panes inside the content, as the regions draw them (`App.css`), at rest, under the
 * pointer, dragged and with the keyboard. Drawn in every cell of the theme matrix, so Purlis shows
 * its quiet grip and Purlis Classic its accent line; walked by the keyboard, held to axe and
 * compared with its baseline (`docs/component-stories.md`). Nothing moves in any state.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Group, Panel } from "react-resizable-panels";
import { expect, waitFor } from "storybook/test";
import { expectFocusInSight } from "../../stories.testkit";
import { panelRoot } from "./Panel";
import { Separator } from "./Separator";

/** A row of regions: a side, its edge, and the content holding two panes split by a handle. */
function Regions() {
  return (
    <div style={{ display: "flex", blockSize: "14rem", inlineSize: "34rem" }}>
      <Group className="regions" orientation="vertical">
        <Panel id="upper" className="region-upper">
          <Group className="region-row" orientation="horizontal">
            <Panel
              id="side"
              defaultSize="35%"
              {...panelRoot("side")}
              role="region"
              aria-label="Side"
            >
              <p style={{ margin: "var(--space-3)" }}>Chats</p>
            </Panel>
            <Separator aria-label="Resize the side" />
            <Panel id="content" {...panelRoot("content")} role="region" aria-label="Content">
              <Group orientation="vertical">
                <Panel id="talk">
                  <p style={{ margin: "var(--space-3)" }}>talk</p>
                </Panel>
                <Separator aria-label="Resize the split" />
                <Panel id="review">
                  <p style={{ margin: "var(--space-3)" }}>review</p>
                </Panel>
              </Group>
            </Panel>
          </Group>
        </Panel>
      </Group>
    </div>
  );
}

const meta = { component: Regions } satisfies Meta<typeof Regions>;

export default meta;
type Story = StoryObj<typeof meta>;

type Canvas = Parameters<NonNullable<Story["play"]>>[0]["canvas"];

const edgeOf = (canvas: Canvas) => canvas.getByRole("separator", { name: "Resize the side" });
const gripOf = (handle: HTMLElement) =>
  handle.querySelector<HTMLElement>('[data-scope="panel-separator"][data-part="grip"]');

/** Where every handle and its grip are, and how big: the same in every state. */
function placesOf(root: HTMLElement) {
  return [...root.querySelectorAll<HTMLElement>('[data-scope="panel-separator"]')].map((one) => {
    const box = one.getBoundingClientRect();
    return [box.left, box.top, box.width, box.height];
  });
}

/** The side's edge, its middle: where a person grabs it. */
function middleOf(handle: HTMLElement) {
  const box = handle.getBoundingClientRect();
  return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
}

/** The colour a fill of `value` is drawn in here, read off a probe: `transparent`, or a token. */
function drawnAs(value: string) {
  const probe = document.createElement("span");
  probe.style.backgroundColor = value;
  document.body.append(probe);
  const drawn = getComputedStyle(probe).backgroundColor;
  probe.remove();
  return drawn;
}

/**
 * **What the handle draws in a state**: never the accent on its grip under the pointer or while
 * dragged, and something to see: Purlis's grip, or Purlis Classic's accent line.
 */
function expectAQuietMark(handle: HTMLElement) {
  const grip = getComputedStyle(gripOf(handle) as HTMLElement).backgroundColor;
  const line = getComputedStyle(handle).backgroundColor;
  const accent = drawnAs("var(--accent-base)");
  expect(grip).not.toBe(accent);
  expect(grip !== drawnAs("transparent") || line === accent).toBe(true);
}

/** **The keyboard is on `handle`**: its box is drawn, and its grip or its line says so in the accent. */
function expectFocusShown(handle: HTMLElement) {
  const box = handle.getBoundingClientRect();
  expect(Math.min(box.width, box.height)).toBeGreaterThanOrEqual(2);
  expectFocusInSight();
  const accent = drawnAs("var(--accent-base)");
  const grip = getComputedStyle(gripOf(handle) as HTMLElement).backgroundColor;
  expect(grip === accent || getComputedStyle(handle).backgroundColor === accent).toBe(true);
}

/** **At rest**: no grip, and no line of its own between two panes. */
export const AtRest: Story = {
  play: async ({ canvas }) => {
    const edge = edgeOf(canvas);
    await expect(edge).toHaveAttribute("data-scope", "panel-separator");
    await expect(edge).toHaveAttribute("data-part", "root");
    await expect(getComputedStyle(gripOf(edge) as HTMLElement).backgroundColor).toBe(
      drawnAs("transparent"),
    );
  },
};

/**
 * **Under the pointer**: Purlis's grip, a short faint mark at the edge's middle, and no line the
 * window's length; Purlis Classic's accent line. The pointer is held at the side's own edge,
 * inside the handle's hit area.
 */
export const UnderThePointer: Story = {
  play: async ({ canvas, canvasElement }) => {
    const before = placesOf(canvasElement);
    const side = canvas.getByRole("region", { name: "Side" });
    const box = side.getBoundingClientRect();
    // The library hears the pointer on the document, by where it is.
    side.dispatchEvent(
      new PointerEvent("pointermove", {
        bubbles: true,
        clientX: box.right - 1,
        clientY: box.top + box.height / 2,
        isPrimary: true,
        pointerType: "mouse",
      }),
    );
    const edge = edgeOf(canvas);
    await waitFor(() => expect(edge).toHaveAttribute("data-separator", "hover"));
    expect(placesOf(canvasElement)).toEqual(before);
    expectAQuietMark(edge);
  },
};

/** The press a story holds, let go once its picture is taken, so no later story starts dragged. */
function letGo() {
  return () => {
    document.dispatchEvent(
      new PointerEvent("pointerup", { bubbles: true, isPrimary: true, pointerType: "mouse" }),
    );
  };
}

/**
 * **Dragged**: the grip in the muted tier, clear but not loud; Purlis Classic's accent line. The
 * press is held for the picture, as a person's is while they drag, and let go after it.
 */
export const Dragged: Story = {
  beforeEach: letGo,
  play: async ({ canvas, canvasElement }) => {
    const before = placesOf(canvasElement);
    const edge = edgeOf(canvas);
    const { x, y } = middleOf(edge);
    edge.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        cancelable: true,
        clientX: x,
        clientY: y,
        button: 0,
        buttons: 1,
        isPrimary: true,
        pointerType: "mouse",
      }),
    );
    await waitFor(() => expect(edge).toHaveAttribute("data-separator", "active"));
    expect(placesOf(canvasElement)).toEqual(before);
    expectAQuietMark(edge);
  },
};

/**
 * **With the keyboard on the side's edge**: Tab lands on a handle that is drawn, and its grip in
 * the accent says so (Purlis Classic: its accent line), not a ring the window's height; an arrow
 * moves it.
 */
export const WithTheKeyboard: Story = {
  play: async ({ canvas, userEvent }) => {
    const edge = edgeOf(canvas);
    await userEvent.tab();
    await expect(edge).toHaveFocus();
    expectFocusShown(edge);
    const side = canvas.getByRole("region", { name: "Side" });
    const was = side.getBoundingClientRect().width;
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(side.getBoundingClientRect().width).toBeGreaterThan(was));
    await expect(edge).toHaveFocus();
  },
};

/**
 * **With the keyboard on a pane split**: two panes sit flush, so the split takes no room, and
 * still the keyboard lands on a handle that is drawn, straddling the line between them.
 */
export const SplitWithTheKeyboard: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const before = placesOf(canvasElement);
    const split = canvas.getByRole("separator", { name: "Resize the split" });
    await userEvent.tab();
    await userEvent.tab();
    await expect(split).toHaveFocus();
    expect(placesOf(canvasElement)).toEqual(before);
    expectFocusShown(split);
    const talk = canvas.getByText("talk").parentElement as HTMLElement;
    const was = talk.getBoundingClientRect().height;
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(talk.getBoundingClientRect().height).toBeGreaterThan(was));
    await expect(split).toHaveFocus();
  },
};
