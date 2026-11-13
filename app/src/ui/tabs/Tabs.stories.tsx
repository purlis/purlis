// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The session tabs, docked on the content panel** (#2914): the strip as the operator's
 * switcher prototype draws it, each state that draws differently, held to axe, walked by the
 * keyboard and compared with its baseline in Chromium and WebKit (`docs/component-stories.md`).
 *
 * The strip here is laid out by the window's own strip rules (`.bar`, `.tabs` in `App.css`) and
 * is the family's tablist on React Aria (`TabStrip.tsx`), as the chat strip is, so what is drawn
 * and how the keyboard moves along it are what the window does. Its behaviour beyond choosing a
 * tab (the drag, the collapse into show-more, the menus) is the chat strip's, held by its own
 * tests.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Fragment, useState, type CSSProperties } from "react";
import { expect, screen, waitFor } from "storybook/test";
import { Bot, Plus } from "lucide-react";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { useTabIds } from "./TabStrip";
import { ChatMark } from "../chat-mark/ChatMark";
import type { ChatState } from "../../shownState";
import { Button, IconButton } from "../button/Button";
import { TIPS_BELOW } from "../tooltip/Tooltip";
import { pictureOf } from "../../layoutPicture";
import type { Layout } from "../../tabs";
import {
  editorToolbar,
  TabCell,
  TabPicture,
  TabCloser,
  tabPart,
  tabsPart,
  ToolbarFile,
  type TabVariant,
} from "./Tabs";
import { Group, Panel } from "react-resizable-panels";
import { panelRoot } from "../panel/Panel";
import { Separator } from "../panel/Separator";
import { StripTab, TabStripRow, useTabStrip } from "./TabStrip";

/** One tab as a story draws it: its name, what it holds, its chat's state, if any, and its
 *  layout where it is split. */
type Shown = { name: string; variant?: TabVariant; state?: ChatState; layout?: Layout };

const CHATS: Shown[] = [
  { name: "steward 1", state: "working" },
  { name: "talk", state: "waiting" },
  { name: "review the tab strip prototype" },
  { name: "purlis.toml", variant: "file" },
];

/**
 * **A docked strip and the panel under it.** The arrows move to the tab beside the one with the
 * keyboard, Home and End to the first and the last, and Enter or Space chooses it, as on the
 * chat strip (React Aria's tabs, manual activation).
 */
function DockedStrip({
  tabs = CHATS,
  chosen = 1,
  width = 640,
  split = false,
  marks = false,
}: {
  tabs?: Shown[];
  chosen?: number;
  width?: number;
  /** The chosen tab split in two, the second pane with the keyboard. */
  split?: boolean;
  /** The strip's other marks, as the chat strip draws them: `N more` at the row's end (#3361). A
   *  tab draws no skill run chip since #5524: it is on its chat's pane head. */
  marks?: boolean;
}) {
  const [inFront, setInFront] = useState(chosen);
  const [open, setOpen] = useState(tabs);
  const idOf = useTabIds();
  const front = open[inFront];
  const along = useTabStrip({ keys: open.map((one) => one.name), selected: front?.name });
  return (
    <div className="editor" {...tabsPart("layout")} style={{ inlineSize: width, blockSize: 220 }}>
      <header className="bar" {...tabsPart("strip")} {...TIPS_BELOW}>
        <TabStripRow
          strip={along}
          name="Chats"
          ids={open.map((_, at) => idOf(at))}
          className="tabs"
          data-strip="Chats"
          // The strip's floor, as the chat strip is handed it (`fits.ts`).
          style={{ "--least": "88px" } as CSSProperties}
        >
          {open.map((one, at) => (
            <TabCell
              key={one.name}
              className="tab"
              variant={one.variant ?? "chat"}
              selected={at === inFront}
              first={at === 0}
            >
              <StripTab
                strip={along}
                tabKey={one.name}
                selected={at === inFront}
                tip={undefined}
                type="button"
                id={idOf(at)}
                onClick={() => setInFront(at)}
              >
                {one.state && <ChatMark state={one.state} who={<Bot aria-hidden="true" />} />}
                <span className="tab-name" {...tabPart("label")}>
                  {one.name}
                </span>
                <SplitPicture layout={one.layout} />
              </StripTab>
              <TabCloser
                label={`Close ${one.name}`}
                // The front tab's × is the one Tab stop after the strip, as on the chat strip.
                reachable={at === inFront}
                onPress={() => {
                  setOpen(open.filter((_, there) => there !== at));
                  setInFront(Math.max(0, Math.min(inFront, open.length - 2)));
                }}
              />
            </TabCell>
          ))}
          <span className="strip-doing">
            <IconButton variant="quiet" size="sm" icon={<Plus />} aria-label="New tab" />
          </span>
        </TabStripRow>
        {marks && (
          <div className="more">
            <Button variant="quiet" size="sm" className="show-more" tooltip="2 tabs past the edge">
              2 more
            </Button>
          </div>
        )}
      </header>
      <div
        className="panes"
        {...panelRoot("content")}
        role="tabpanel"
        aria-labelledby={idOf(inFront)}
      >
        {split ? (
          /* The panes' split as the window draws it (`LayoutPanes.tsx`): the panel separator
             family's handle between two of the library's panels. */
          <Group orientation="horizontal">
            {["talk", "review"].map((name, at) => (
              <Fragment key={name}>
                {at > 0 && <Separator className="pane-split" aria-label="Resize the split" />}
                <Panel id={name}>
                  <div
                    className="pane-frame pane-headed"
                    data-testid={`pane ${name}`}
                    data-focused={at === 1 || undefined}
                  >
                    {/* As `PaneFrame` draws a chat's pane: its header, then the terminal (#3142). */}
                    <div className="pane-top">
                      <div className="pane-head">
                        <div className="pane-chips">
                          <span className="pane-from">{name}</span>
                        </div>
                      </div>
                    </div>
                    <div className="pane-body">
                      <div className={at === 1 ? "pane focused" : "pane"}>
                        <p style={{ margin: 0, padding: "var(--space-7) var(--space-5)" }}>
                          {name}
                        </p>
                      </div>
                    </div>
                  </div>
                </Panel>
              </Fragment>
            ))}
          </Group>
        ) : (
          /* The focused pane, as the window draws it (`.pane.focused`): what the chosen tab
            joins, with no line between them. */
          <div className="pane focused">
            {front?.variant === "file" ? (
              <div className="piece-file">
                <header {...editorToolbar("piece-files-head")}>
                  <ToolbarFile path={`~/IdeaProjects/purlis/${front.name}`} />
                </header>
              </div>
            ) : (
              <p style={{ margin: 0, padding: "var(--space-5)" }}>{front?.name}</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/** A split tab's picture after its name, as the chat strip draws it (`ChatRows.tsx`). */
function SplitPicture({ layout }: { layout?: Layout }) {
  const picture = layout === undefined ? undefined : pictureOf(layout);
  if (picture === undefined) return null;
  return (
    <span className="tab-split-more" data-mark="split">
      <TabPicture picture={picture} />
    </span>
  );
}

/** A chat's pane: the id tells it from the others in one tab. */
const leaf = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "session", session: pane, chat: String(pane) },
});
const row = (...children: [Layout, Layout]): Layout => ({
  kind: "split",
  direction: "row",
  children,
});
const column = (...children: [Layout, Layout]): Layout => ({
  kind: "split",
  direction: "column",
  children,
});

const meta = {
  component: DockedStrip,
} satisfies Meta<typeof DockedStrip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Whether the tabs are docked on the panel in the cell's theme (Purlis), not on a strip of
 *  their own (Purlis Classic's parts): only then does nothing run between a tab and its pane. */
function docked(canvas: HTMLElement): boolean {
  const layout = canvas.querySelector('[data-scope="tabs"][data-part="layout"]');
  return (
    layout !== null && getComputedStyle(layout).getPropertyValue("--tabs-joined").trim() === "1"
  );
}

/** Where each tab is drawn, by its name. */
function placesOf(canvas: HTMLElement): Map<string, { x: number; width: number }> {
  return new Map(
    [...canvas.querySelectorAll<HTMLElement>('[data-scope="tabs"][data-part="tab"]')].map(
      (cell) => {
        const box = cell.getBoundingClientRect();
        return [cell.textContent ?? "", { x: box.x, width: box.width }];
      },
    ),
  );
}

/**
 * **The chosen tab on the panel, its flares joining it.** Tab lands on the chosen tab with its
 * ring in sight; the arrows, Home and End move to another, which Enter chooses; and **no tab
 * moves when the choice changes**: each is measured before and after every choice, and none has
 * moved or changed its width by a pixel.
 */
export const Docked: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const chosen = canvas.getByRole("tab", { name: /talk/ });
    await userEvent.tab();
    await expect(chosen).toHaveFocus();
    expectFocusInSight();
    const before = placesOf(canvasElement);
    // Moving is not choosing (manual activation, D-3042-2): the first move leaves talk chosen.
    await userEvent.keyboard("{ArrowRight}");
    await expect(chosen).toHaveAttribute("aria-selected", "true");
    await expect(document.activeElement).toHaveAttribute("aria-selected", "false");
    await userEvent.keyboard("{ArrowLeft}");
    await expect(chosen).toHaveFocus();
    for (const key of ["{ArrowRight}", "{ArrowRight}", "{Home}", "{End}", "{ArrowLeft}"]) {
      await userEvent.keyboard(key);
      await userEvent.keyboard("{Enter}");
      await untilFocused(
        () => canvas.getByRole("tab", { selected: true }),
        `Tabs: ${key} then Enter chooses the tab the keyboard is on`,
      );
      expect(placesOf(canvasElement)).toEqual(before);
    }
    // Past the last tab is the first (WAI-ARIA Tabs, D-3042-4).
    await userEvent.keyboard("{End}{ArrowRight}");
    await expect(canvas.getByRole("tab", { name: /steward 1/ })).toHaveFocus();
    expectFocusInSight();
    // Back on the second, which is what the baseline shows.
    await userEvent.keyboard("{Home}{ArrowRight}{Enter}");
    await expect(chosen).toHaveAttribute("aria-selected", "true");
    await expect(chosen).toHaveFocus();
    // The chosen tab joins the pane under it: the focused pane draws no line along its top,
    // where the tab and its flares meet it, and keeps its focus on its other edges.
    const pane = canvasElement.querySelector<HTMLElement>(".pane.focused");
    expect(pane).not.toBeNull();
    const drawn = getComputedStyle(pane as HTMLElement);
    expect(drawn.borderTopWidth).toBe(docked(canvasElement) ? "0px" : "1px");
    // Purlis draws no line around a pane at all (the operator's ruling); Classic its hairline.
    expect(drawn.borderLeftWidth).toBe(docked(canvasElement) ? "0px" : "1px");
    const tab = chosen.closest<HTMLElement>('[data-part="tab"]');
    expect(tab?.getBoundingClientRect().bottom).toBeCloseTo(
      (pane as HTMLElement).getBoundingClientRect().top,
      0,
    );
  },
};

/** **The first tab chosen**: on the panel's edge, the panel's top-left corner square under it,
 *  and a flare on its right only. */
export const FirstChosen: Story = {
  args: { chosen: 0 },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("tab", { name: /steward 1/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  },
};

/** **Under the pointer**: an unchosen tab's pill, inset from its cell so it never touches the
 *  panel, and its words in the primary tier. Nothing moves. */
export const Hovered: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const before = placesOf(canvasElement);
    const tab = canvas.getByRole("tab", { name: /review the tab strip/ });
    await userEvent.hover(tab);
    await waitFor(() =>
      expect(tab.closest('[data-part="tab"]')).toHaveAttribute("data-hovered", "true"),
    );
    // Its × has come over the end of its name (#5341): the picture is after its fade.
    await waitFor(() => expect(seen(closerOf(canvas, /review the tab strip/))).toBe("1"));
    expect(placesOf(canvasElement)).toEqual(before);
  },
};

/** **A file's tab, chosen**: it joins the editor's toolbar under it, which names the file. */
export const FileChosen: Story = {
  args: { chosen: 3 },
  play: async ({ canvas, canvasElement }) => {
    await expect(canvas.getByRole("tab", { name: "purlis.toml" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(canvas.getByText("~/IdeaProjects/purlis/purlis.toml")).toBeVisible();
    // The tab, the toolbar and the code under it are one surface, with no line between.
    const tab = canvas.getByRole("tab", { name: "purlis.toml" }).closest('[data-part="tab"]');
    const toolbar = canvasElement.querySelector('[data-scope="editor"][data-part="toolbar"]');
    expect(tab && getComputedStyle(tab).backgroundColor).toBe(
      toolbar && getComputedStyle(toolbar).backgroundColor,
    );
    expect(
      getComputedStyle(canvasElement.querySelector(".pane.focused") as Element).borderTopWidth,
    ).toBe(docked(canvasElement) ? "0px" : "1px");
  },
};

/** **A narrow strip**: the tabs shrink, each name cut short with an ellipsis, before anything
 *  else gives way; the `+` still ends the strip. */
export const Narrow: Story = {
  args: {
    width: 660,
    tabs: [
      ...CHATS,
      { name: "dispatch the docked tabs lane" },
      { name: "steward 2", state: "working" },
    ],
  },
  play: async ({ canvas, canvasElement }) => {
    const label = canvas.getByText("review the tab strip prototype");
    await waitFor(() => expect(label.scrollWidth).toBeGreaterThan(label.clientWidth));
    const strip = canvasElement.querySelector<HTMLElement>('[data-strip="Chats"]');
    const add = canvas.getByRole("button", { name: "New tab" });
    expect(add.getBoundingClientRect().right).toBeLessThanOrEqual(
      strip?.getBoundingClientRect().right ?? 0,
    );
  },
};

/**
 * **A split, the second pane with the keyboard**: no line around either pane in Purlis, so the
 * words in the corner of the pane with the keyboard are primary, and the split parts the two: by
 * the window's gutter in Purlis, by its hairline in Purlis Classic (#3143).
 */
export const Split: Story = {
  args: { split: true },
  play: async ({ canvas }) => {
    const words = (name: string) =>
      canvas.getByTestId(`pane ${name}`).querySelector(".pane-from") as Element;
    // The pane with the keyboard says so in primary words; the other keeps its muted tier.
    await waitFor(() =>
      expect(getComputedStyle(words("review")).color).not.toBe(
        getComputedStyle(words("talk")).color,
      ),
    );
    // The split parts the two panes (#3143): they sit its room apart, and what it draws in that
    // room is not the panel's ground. Purlis floats them a gutter apart, in the window's colour;
    // Purlis Classic parts them by its hairline.
    const split = canvas.getByRole("separator", { name: "Resize the split" });
    const talk = canvas.getByTestId("pane talk").getBoundingClientRect();
    const review = canvas.getByTestId("pane review").getBoundingClientRect();
    expect(review.left - talk.right).toBeGreaterThan(0);
    const panel = getComputedStyle(canvas.getByRole("tabpanel")).backgroundColor;
    const drawn = getComputedStyle(split);
    const gutter = parseFloat(drawn.getPropertyValue("--window-gutter")) || 0;
    if (gutter > 0) {
      const probe = document.createElement("span");
      probe.style.backgroundColor = "var(--surface-deep)";
      document.body.append(probe);
      const deep = getComputedStyle(probe).backgroundColor;
      probe.remove();
      expect(Math.round(review.left - talk.right)).toBe(gutter);
      expect(drawn.backgroundImage).toContain(deep);
      expect(deep).not.toBe(panel);
    } else {
      expect(drawn.backgroundColor).not.toBe(panel);
    }
  },
};

/** The `×` of the tab named like `name`, and the opacity it is drawn at. */
function closerOf(canvas: { getByRole: typeof screen.getByRole }, name: RegExp) {
  const cell = canvas.getByRole("tab", { name }).closest('[data-part="tab"]');
  const closer = cell?.querySelector<HTMLElement>('[data-part="closer"]');
  if (!closer) throw new Error(`no × on the tab ${String(name)}`);
  return closer;
}
const seen = (closer: HTMLElement) => getComputedStyle(closer).opacity;
/** The width a tab's name is drawn at. */
const nameWidth = (canvas: { getByRole: typeof screen.getByRole }, name: RegExp) =>
  (
    canvas.getByRole("tab", { name }).querySelector('[data-part="label"]') as HTMLElement
  ).getBoundingClientRect().width;

/**
 * **The `×` at rest** (#5341): seen on the front tab only, standing over the end of its name, so
 * every other tab's name has that room. Waited for until its fade has run: the picture is what
 * stays, never a frame of the fade.
 */
export const CloseAtRest: Story = {
  // The long name in front, so its × stands over the end of it.
  args: { chosen: 2 },
  play: async ({ canvas }) => {
    const long = /review the tab strip/;
    await waitFor(() => expect(seen(closerOf(canvas, long))).toBe("1"));
    for (const name of [/steward 1/, /talk/, /purlis\.toml/])
      await expect(seen(closerOf(canvas, name))).toBe("0");
    // It takes no room of its own: the name runs on to the ×, or under it where the tab is cut
    // short, and the × stands inside the tab's cell.
    const name = canvas
      .getByRole("tab", { name: long })
      .querySelector('[data-part="label"]') as HTMLElement;
    const closer = closerOf(canvas, long).getBoundingClientRect();
    expect(name.getBoundingClientRect().right).toBeGreaterThanOrEqual(closer.left - 1);
    const cell = name.closest('[data-part="tab"]') as HTMLElement;
    expect(closer.right).toBeLessThanOrEqual(cell.getBoundingClientRect().right);
  },
};

/**
 * **A short name in front is never cut by its `×`** (#5341): where the strip has room, the end of
 * the name is kept clear of the `×` and its fade, whether the theme shares the strip out or sizes
 * each tab to its words.
 */
export const ShortNameInFront: Story = {
  play: async ({ canvas }) => {
    await waitFor(() => expect(seen(closerOf(canvas, /talk/))).toBe("1"));
    const name = canvas
      .getByRole("tab", { name: /talk/ })
      .querySelector('[data-part="label"]') as HTMLElement;
    await expect(name.scrollWidth).toBeLessThanOrEqual(name.clientWidth);
    // Nor a short name behind it: the room kept for its × costs a tab with room none of its name.
    const behind = canvas
      .getByRole("tab", { name: /steward 1/ })
      .querySelector('[data-part="label"]') as HTMLElement;
    await expect(behind.scrollWidth).toBeLessThanOrEqual(behind.clientWidth);
    expect(name.getBoundingClientRect().right).toBeLessThanOrEqual(
      closerOf(canvas, /talk/).getBoundingClientRect().left,
    );
  },
};

/**
 * **Under the pointer, a background tab's `×` comes over the end of its name**, and the name keeps
 * its width: nothing moves. Waited for until it is drawn whole.
 */
export const CloseUnderThePointer: Story = {
  play: async ({ canvas, userEvent }) => {
    const long = /review the tab strip/;
    const before = nameWidth(canvas, long);
    await userEvent.hover(canvas.getByRole("tab", { name: long }));
    await waitFor(() => expect(seen(closerOf(canvas, long))).toBe("1"));
    expect(nameWidth(canvas, long)).toBe(before);
  },
};

/**
 * **With the keyboard on a background tab, its `×` comes too**, and Tab reaches the front tab's,
 * the one `×` in the keyboard's order.
 */
export const CloseWithTheKeyboard: Story = {
  play: async ({ canvas, userEvent }) => {
    const front = canvas.getByRole("tab", { name: /talk/ });
    await userEvent.tab();
    await untilFocused(() => front, "Tab onto the strip, on the front tab");
    await userEvent.tab();
    await untilFocused(() => closerOf(canvas, /talk/), "Tab on to the front tab's ×");
    await userEvent.tab({ shift: true });
    await untilFocused(() => front, "Back onto the front tab");
    // Left to a short name: a cut name's tooltip comes up after a rest, which no picture waits
    // on by timing.
    await userEvent.keyboard("{ArrowLeft}");
    const next = /steward 1/;
    await untilFocused(() => canvas.getByRole("tab", { name: next }), "Left to the tab before");
    await waitFor(() => expect(seen(closerOf(canvas, next))).toBe("1"));
  },
};

/** The vertical centre of `element`'s box, in px. */
function middleOf(element: Element): number {
  const box = element.getBoundingClientRect();
  return box.top + box.height / 2;
}

/**
 * **The strip's marks on the tabs' line, their tooltips clear of it** (#3361): the `+` and
 * `N more` are each centred on a tab's line, within a pixel, in every cell; and their tooltips,
 * opened from the keyboard, stand below the strip, never on its row.
 */
export const MarksOnTheTabsLine: Story = {
  args: { marks: true },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const tab = canvasElement.querySelector('[data-scope="tabs"][data-part="tab"]');
    if (tab === null) throw new Error("no tab drawn");
    const line = middleOf(tab);
    const plus = canvas.getByRole("button", { name: "New tab" });
    const more = canvas.getByRole("button", { name: "2 more" });
    for (const [what, one] of [
      ["the +", plus],
      ["N more", more],
    ] as const)
      await expect(
        Math.abs(middleOf(one) - line),
        `${what} off the tabs' line`,
      ).toBeLessThanOrEqual(1);
    // Each mark's tooltip, opened by the keyboard's focus (at once), stands off the strip's row:
    // Tab along the strip's stops (the strip's tab, the `+`, `N more`), and each mark's
    // is measured where the focus reaches it, then taken down with Esc.
    const strip = canvasElement.querySelector("header.bar") as HTMLElement;
    const shown = () =>
      document.querySelector<HTMLElement>('[role="tooltip"][data-scope="tooltip"]');
    const marks = new Map<Element, [string, string]>([
      [plus, ["the +", "New tab"]],
      [more, ["N more", "2 tabs past the edge"]],
    ]);
    const measured: string[] = [];
    for (let stops = 0; stops < 10 && measured.length < marks.size; stops += 1) {
      await userEvent.tab();
      const mark = document.activeElement === null ? undefined : marks.get(document.activeElement);
      if (mark === undefined) continue;
      const [what, says] = mark;
      await waitFor(() => expect(shown()).toHaveTextContent(says), { timeout: 3000 });
      const tip = (shown() as HTMLElement).getBoundingClientRect();
      const row = strip.getBoundingClientRect();
      await expect(
        tip.top >= row.bottom || tip.bottom <= row.top,
        `${what}'s tooltip sits on the strip's row`,
      ).toBe(true);
      await userEvent.keyboard("{Escape}");
      await waitFor(() => expect(shown()).toBeNull(), { timeout: 3000 });
      measured.push(what);
    }
    await expect(measured).toEqual(["the +", "N more"]);
  },
};

/**
 * **A split tab's small picture of its layout** (#5342, spec #5339), after its name in place of
 * `+1`: a tile for each pane (#5577), two side by side, three with the right half split again,
 * four in a quad, and past four a 3 × 3 grid. Each picture is the tab's own shape, in the quiet tier, and never cut: a name gives way
 * first. A screen reader hears how many panes in words.
 */
export const SplitPictures: Story = {
  args: {
    width: 900,
    chosen: 0,
    tabs: [
      { name: "steward 1", state: "working", layout: row(leaf(1), leaf(2)) },
      { name: "talk", state: "needs", layout: row(leaf(1), column(leaf(2), leaf(3))) },
      {
        name: "review",
        state: "idle",
        layout: row(column(leaf(1), leaf(2)), column(leaf(3), leaf(4))),
      },
      {
        name: "devops 2",
        state: "waiting",
        layout: row(leaf(1), row(leaf(2), row(leaf(3), row(leaf(4), row(leaf(5), leaf(6)))))),
      },
      { name: "purlis.toml", variant: "file" },
    ],
  },
  play: async ({ canvas, canvasElement }) => {
    const pictures = [
      ...canvasElement.querySelectorAll<SVGElement>('[data-scope="tabs"][data-part="picture"]'),
    ];
    // One per split tab, none on the tab of one pane, each its own shape.
    // A tile for each pane (#5577), and past four the 3 × 3 grid, which no quad draws.
    await expect(pictures.map((one) => one.querySelectorAll("rect").length)).toEqual([2, 3, 4, 9]);
    for (const [name, panes] of [
      ["steward 1", 2],
      ["talk", 3],
      ["review", 4],
      ["devops 2", 6],
    ] as const) {
      // Its name says its mark's state first, then its own name and how many panes.
      const tab = canvas.getByRole("tab", {
        name: new RegExp(`${name}.*split, ${panes} panes`),
      });
      // After the name, on its line, and drawn whole.
      const label = tab.querySelector('[data-part="label"]') as HTMLElement;
      const picture = tab.querySelector('[data-part="picture"]') as SVGElement;
      const drawn = picture.getBoundingClientRect();
      await expect(drawn.left).toBeGreaterThanOrEqual(label.getBoundingClientRect().right);
      await expect(drawn.width).toBeGreaterThan(0);
      await expect(Math.abs(middleOf(picture) - middleOf(label))).toBeLessThanOrEqual(1);
    }
  },
};
