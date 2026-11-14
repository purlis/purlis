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
import { type CSSProperties, type ReactNode, useLayoutEffect, useRef, useState } from "react";
import { useTabIds } from "../tabs/TabStrip";
import { TabCell, tabPart, tabsPart } from "../tabs/Tabs";
import { StripTab, TabStripRow, useTabStrip } from "../tabs/TabStrip";
import {
  PanePreviewArriving,
  PanePreviewHeld,
  PanePreviewLine,
  PanePreviewOpening,
  PanePreviewRefused,
  PanePreviewSize,
  PanePreviewWords,
  type PreviewBox,
} from "./PanePreview";

const WIDTH = 640;
const HEIGHT = 240;

/** A terminal's lines, as bars: its text is on a canvas, so what dims here is drawn the same way,
 *  with no words for a contrast gate to read. */
function TerminalLines() {
  return (
    <div style={{ padding: "var(--space-4)" }}>
      {["60%", "85%", "40%", "70%"].map((width) => (
        <div
          key={width}
          style={{
            width,
            height: "var(--space-2)",
            marginBlockEnd: "var(--space-2)",
            backgroundColor: "var(--terminal-foreground)",
          }}
        />
      ))}
    </div>
  );
}

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
        <div className="pane-head">
          <div className="pane-chips">
            <span className="pane-name ui-text-heading">{name}</span>
          </div>
        </div>
      </div>
      <div className="pane-body">
        <TerminalLines />
      </div>
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

/** What animates as it appears is taken to its end, so no picture has it half way. */
const still: Story["play"] = async ({ canvasElement }) => {
  for (const one of canvasElement.getAnimations({ subtree: true })) one.finish();
  await expect(canvasElement.querySelector('[data-testid="room"]')).not.toBeNull();
};

/** A pane picked up, over no place: it dims, and letting go cancels. */
export const PickedUp: Story = {
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

/** How tall the strip is drawn above the panes in {@link RestingOnATab}. */
const STRIP_HEIGHT = 36;

/**
 * Two tabs on a strip, drawn by the tabs family, over two panes: the second tab wears the mark a
 * drag resting on it draws, measured from where the strip draws it, as the window measures it.
 */
function RestingOnTheStrip() {
  const names = ["copy-past-crash", "performance"];
  const idOf = useTabIds();
  const along = useTabStrip({ keys: names, selected: names[0] });
  const room = useRef<HTMLDivElement>(null);
  const resting = useRef<HTMLDivElement>(null);
  const [mark, setMark] = useState<PreviewBox>();
  useLayoutEffect(() => {
    const tab = resting.current?.getBoundingClientRect();
    const at = room.current?.getBoundingClientRect();
    if (tab === undefined || at === undefined) return;
    setMark({
      left: tab.left - at.left,
      top: tab.bottom - at.top - 2,
      width: tab.width,
      height: 2,
    });
  }, []);
  const below = (box: typeof LEFT) => ({ ...box, top: box.top + STRIP_HEIGHT });
  return (
    <div ref={room} style={{ position: "absolute", inset: 0 }}>
      <div className="editor" {...tabsPart("layout")} style={{ blockSize: STRIP_HEIGHT }}>
        <header className="bar" {...tabsPart("strip")}>
          <TabStripRow
            strip={along}
            name="Chats"
            ids={names.map((_, at) => idOf(at))}
            className="tabs"
          >
            {names.map((name, at) => (
              <TabCell
                key={name}
                ref={at === 1 ? resting : undefined}
                className="tab"
                variant="chat"
                selected={at === 0}
                first={at === 0}
              >
                <StripTab
                  strip={along}
                  tabKey={name}
                  selected={at === 0}
                  tip={undefined}
                  type="button"
                  id={idOf(at)}
                >
                  <span className="tab-name" {...tabPart("label")}>
                    {name}
                  </span>
                </StripTab>
              </TabCell>
            ))}
          </TabStripRow>
        </header>
      </div>
      <Pane name="copy-past-crash" {...below(LEFT)} />
      <Pane name="codeql-issues" {...below(RIGHT)} doing="lifted" />
      {mark !== undefined && <PanePreviewOpening box={mark} />}
      <PanePreviewHeld at={{ left: 300, top: 40 }}>codeql-issues</PanePreviewHeld>
      <PanePreviewWords at={{ left: 300, top: 70 }}>A new tab with codeql-issues.</PanePreviewWords>
    </div>
  );
}

/**
 * Resting on another tab (#5294): it wears a mark along its bottom until it comes forward, and
 * the words say what letting go there would do.
 */
export const RestingOnATab: Story = {
  play: async (context) => {
    await still?.(context);
    await expect(
      context.canvasElement.querySelector('[data-scope="pane-preview"][data-part="opening"]'),
    ).not.toBeNull();
  },
  render: () => (
    <Room>
      <RestingOnTheStrip />
    </Room>
  ),
};

/** Over its own place: nothing red and nothing dashed. */
export const OverItself: Story = {
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
