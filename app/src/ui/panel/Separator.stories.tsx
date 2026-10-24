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

/** **At rest**: no grip; a flush window's pane split keeps its hairline. */
export const AtRest: Story = {
  play: async ({ canvas }) => {
    const edge = edgeOf(canvas);
    await expect(edge).toHaveAttribute("data-scope", "panel-separator");
    await expect(edge).toHaveAttribute("data-part", "root");
    // No fill: what a span with no fill of its own draws.
    const bare = document.createElement("span");
    document.body.append(bare);
    const none = getComputedStyle(bare).backgroundColor;
    bare.remove();
    await expect(getComputedStyle(gripOf(edge) as HTMLElement).backgroundColor).toBe(none);
  },
};

/**
 * **Under the pointer**: Purlis's grip, a short faint mark at the edge's middle, and no line the
 * window's length; Purlis Classic's accent line. The pointer is held at the side's own edge, inside
 * the handle's hit area, which is wider than a flush window's handle.
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
  },
};

/**
 * **Dragged**: the grip in the muted tier, clear but not loud; Purlis Classic's accent line. The
 * press is held, as a person's is while they drag.
 */
export const Dragged: Story = {
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
  },
};

/** **With the keyboard**: Tab lands on the edge with the window's 2px ring, and an arrow moves it. */
export const WithTheKeyboard: Story = {
  play: async ({ canvas, userEvent }) => {
    const edge = edgeOf(canvas);
    await userEvent.tab();
    await expect(edge).toHaveFocus();
    expectFocusInSight();
    const ring = getComputedStyle(edge);
    await expect(ring.outlineStyle).toBe("solid");
    await expect(ring.outlineWidth).toBe("2px");
    // Outside the handle, which is thinner than its ring.
    await expect(ring.outlineOffset).toBe("2px");
    const side = canvas.getByRole("region", { name: "Side" });
    const was = side.getBoundingClientRect().width;
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(side.getBoundingClientRect().width).toBeGreaterThan(was));
    await expect(edge).toHaveFocus();
  },
};
