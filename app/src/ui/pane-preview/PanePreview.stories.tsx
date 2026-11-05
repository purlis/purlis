// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a drag draws, each kind of preview** (#5291, spec #5288), in every cell of the theme
 * matrix. The drag's own logic is window tests' (`paneMakingRoom.window.test.tsx`); each story here
 * is one still of it, drawn with the marks and the boxes a drag writes, so no picture catches a
 * state by timing: nothing moves, and what animates as it appears is taken to its end.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import type { CSSProperties, ReactNode } from "react";
import {
  PanePreviewArriving,
  PanePreviewHeld,
  PanePreviewLine,
  PanePreviewRefused,
  PanePreviewSize,
  PanePreviewWords,
} from "./PanePreview";

const WIDTH = 640;
const HEIGHT = 240;

/** A pane as the window draws it, at a box in the room, doing what a drag has it do. */
function Pane({
  name,
  left,
  top,
  width,
  height,
  doing,
}: {
  name: string;
  left: number;
  top: number;
  width: number;
  height: number;
  doing?: "lifted" | "moved";
}) {
  const box: CSSProperties = { position: "absolute", left, top, width, height };
  return (
    <div className="pane-frame pane-headed" data-pane-preview={doing} style={box}>
      <div className="pane-top">
        <div className="pane-chips">
          <span className="pane-name">{name}</span>
        </div>
      </div>
      <div className="pane-body" />
    </div>
  );
}

/**
 * The room the panes have. What a drag draws is fixed to the window: a transform makes this the
 * box they are fixed to, as the window is for the real ones.
 */
function Room({ children }: { children: ReactNode }) {
  return (
    <div
      data-testid="room"
      style={{
        position: "relative",
        width: WIDTH,
        height: HEIGHT + 56,
        overflow: "hidden",
        transform: "translateZ(0)",
      }}
    >
      {children}
    </div>
  );
}

const LEFT = { left: 0, top: 0, width: WIDTH / 2 - 2, height: HEIGHT };
const RIGHT = { left: WIDTH / 2 + 2, top: 0, width: WIDTH / 2 - 2, height: HEIGHT };

const meta: Meta = { title: "Panes/Moving a pane" };
export default meta;
type Story = StoryObj;

/**
 * A pane picked up is dimmed on purpose while it is held, its words with it: it says what is being
 * moved by being faint, its name is at full strength on the label by the pointer, and a reader
 * hears it from the window's status line. So contrast is off on these stories alone.
 */
const DIMMED = { a11y: { options: { rules: { "color-contrast": { enabled: false } } } } };

/** What animates as it appears is taken to its end, so no picture has it half way. */
const still: Story["play"] = async ({ canvasElement }) => {
  for (const one of canvasElement.getAnimations({ subtree: true })) one.finish();
  await expect(canvasElement.querySelector('[data-testid="room"]')).not.toBeNull();
};

/** A pane picked up, over no place: it dims, and letting go cancels. */
export const PickedUp: Story = {
  parameters: DIMMED,
  play: still,
  render: () => (
    <Room>
      <Pane name="copy-past-crash" {...LEFT} />
      <Pane name="codeql-issues" {...RIGHT} doing="lifted" />
      <PanePreviewHeld at={{ left: 200, top: 120 }}>codeql-issues</PanePreviewHeld>
      <PanePreviewWords at={{ left: 200, top: 150 }}>
        Let go here to cancel. Esc also cancels.
      </PanePreviewWords>
    </Room>
  ),
};

/** Held at another pane's edge: the two make room, each with the size it will take. */
export const MakesRoom: Story = {
  play: still,
  render: () => (
    <Room>
      <Pane
        name="codeql-issues"
        left={0}
        top={0}
        width={WIDTH}
        height={HEIGHT / 2 - 2}
        doing="moved"
      />
      <Pane
        name="copy-past-crash"
        left={0}
        top={HEIGHT / 2 + 2}
        width={WIDTH}
        height={HEIGHT / 2 - 2}
      />
      <PanePreviewSize
        box={{ left: 0, top: 0, width: WIDTH, height: HEIGHT / 2 - 2 }}
        columns={78}
        rows={4}
      />
      <PanePreviewSize
        box={{ left: 0, top: HEIGHT / 2 + 2, width: WIDTH, height: HEIGHT / 2 - 2 }}
        columns={78}
        rows={4}
      />
      <PanePreviewHeld at={{ left: 60, top: 40 }}>codeql-issues</PanePreviewHeld>
      <PanePreviewWords at={{ left: 60, top: 70 }}>
        codeql-issues above copy-past-crash.
      </PanePreviewWords>
    </Room>
  ),
};

/** Into another's middle: the two change places. */
export const Swaps: Story = {
  play: still,
  render: () => (
    <Room>
      <Pane name="codeql-issues" {...LEFT} doing="moved" />
      <Pane name="copy-past-crash" {...RIGHT} />
      <PanePreviewHeld at={{ left: 120, top: 120 }}>codeql-issues</PanePreviewHeld>
      <PanePreviewWords at={{ left: 120, top: 150 }}>
        Swap codeql-issues and copy-past-crash.
      </PanePreviewWords>
    </Room>
  ),
};

/** A chat from the list, to an edge: room opens for it. */
export const Arrives: Story = {
  play: still,
  render: () => (
    <Room>
      <Pane name="copy-past-crash" {...LEFT} />
      <PanePreviewArriving box={RIGHT} name="performance" />
      <PanePreviewSize box={LEFT} columns={38} rows={9} />
      <PanePreviewSize box={RIGHT} columns={38} rows={9} />
      <PanePreviewHeld at={{ left: 400, top: 120 }}>performance</PanePreviewHeld>
      <PanePreviewWords at={{ left: 290, top: 150 }}>
        performance right of copy-past-crash.
      </PanePreviewWords>
    </Room>
  ),
};

/** A chat from the list, to a pane's middle: it shows there, and the old chat keeps running. */
export const TakesAPlace: Story = {
  play: still,
  render: () => (
    <Room>
      <PanePreviewArriving
        box={{ left: 0, top: 0, width: WIDTH, height: HEIGHT }}
        name="performance"
      />
      <PanePreviewHeld at={{ left: 200, top: 120 }}>performance</PanePreviewHeld>
      <PanePreviewWords at={{ left: 200, top: 150 }}>
        Show performance here. copy-past-crash keeps running in the sidebar.
      </PanePreviewWords>
    </Room>
  ),
};

/** A side that would fold: its rail, beside the pane that keeps the room. */
export const Folds: Story = {
  play: still,
  render: () => (
    <Room>
      <Pane
        name="copy-past-crash"
        left={0}
        top={0}
        width={WIDTH - 44}
        height={HEIGHT}
        doing="moved"
      />
      <PanePreviewArriving box={{ left: WIDTH - 40, top: 0, width: 40, height: HEIGHT }} />
    </Room>
  ),
};

/** A refused place: marked on its own pane, with the layout where it is. */
export const Refused: Story = {
  parameters: DIMMED,
  play: still,
  render: () => (
    <Room>
      <Pane name="copy-past-crash" {...LEFT} />
      <Pane name="codeql-issues" {...RIGHT} doing="lifted" />
      <PanePreviewRefused box={LEFT} />
      <PanePreviewHeld at={{ left: 20, top: 120 }}>codeql-issues</PanePreviewHeld>
      <PanePreviewWords at={{ left: 20, top: 150 }} refused>
        Too narrow: a pane here would be under about 40 columns. Drop it on a wider pane.
      </PanePreviewWords>
    </Room>
  ),
};

/** On the strip: the panes stay put, and a line says where the tab would go. */
export const OnTheStrip: Story = {
  parameters: DIMMED,
  play: still,
  render: () => (
    <Room>
      <Pane name="copy-past-crash" {...LEFT} />
      <Pane name="codeql-issues" {...RIGHT} doing="lifted" />
      <PanePreviewLine box={{ left: 120, top: 0, width: 2, height: 32 }} />
      <PanePreviewHeld at={{ left: 130, top: 40 }}>codeql-issues</PanePreviewHeld>
      <PanePreviewWords at={{ left: 130, top: 70 }}>A new tab with codeql-issues.</PanePreviewWords>
    </Room>
  ),
};

/** Over its own place: nothing red and nothing dashed. */
export const OverItself: Story = {
  parameters: DIMMED,
  play: still,
  render: () => (
    <Room>
      <Pane name="copy-past-crash" {...LEFT} />
      <Pane name="codeql-issues" {...RIGHT} doing="lifted" />
      <PanePreviewHeld at={{ left: 400, top: 120 }}>codeql-issues</PanePreviewHeld>
      <PanePreviewWords at={{ left: 290, top: 150 }}>
        Let go to leave it where it is.
      </PanePreviewWords>
    </Room>
  ),
};
