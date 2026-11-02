// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The pointer gone from the window, and the window from the front** (#3095), in Chromium and
 * WebKit: leaving by each edge, as each engine's own event objects say it (dispatched here, since
 * a story's mouse cannot leave the page), the window's blur, and the first move back, which arms
 * the next trip out. The picture is what the story heard.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { useWindowLeft, type WindowLeft } from "./windowLeft";

function Heard() {
  const [heard, setHeard] = useState<readonly WindowLeft[]>([]);
  useWindowLeft((why) => setHeard((was) => [...was, why]));
  return <p data-testid="heard">{heard.length === 0 ? "Nothing yet" : heard.join(", ")}</p>;
}

const meta = { component: Heard } satisfies Meta<typeof Heard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Just past each edge of the window: where the pointer's last event says it went. */
const EDGES = (): readonly { clientX: number; clientY: number }[] => [
  { clientX: -1, clientY: 100 },
  { clientX: window.innerWidth, clientY: 100 },
  { clientX: 100, clientY: -1 },
  { clientX: 100, clientY: window.innerHeight },
];

const moveIn = () =>
  document.dispatchEvent(new PointerEvent("pointermove", { clientX: 100, clientY: 100 }));

export const EveryEdge: Story = {
  play: async ({ canvas }) => {
    const heard = canvas.getByTestId("heard");
    moveIn();
    const edges = EDGES();
    for (const at of edges) {
      // All an engine may say of one trip out, which is told once.
      document.dispatchEvent(new PointerEvent("pointerout", { ...at, relatedTarget: null }));
      document.dispatchEvent(new MouseEvent("mouseout", { ...at, relatedTarget: null }));
      document.documentElement.dispatchEvent(new PointerEvent("pointerleave", at));
      // The first move back arms the next.
      moveIn();
    }
    await waitFor(() => expect(heard.textContent).toBe(edges.map(() => "pointer").join(", ")));
    // A move from one element to another is no trip out.
    document.body.dispatchEvent(
      new PointerEvent("pointerout", { bubbles: true, relatedTarget: heard }),
    );
    window.dispatchEvent(new Event("blur"));
    await waitFor(() =>
      expect(heard.textContent).toBe([...edges.map(() => "pointer"), "focus"].join(", ")),
    );
  },
};
