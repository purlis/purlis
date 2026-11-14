// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The project rail family** (#2915, R76–R89): the rail as `prototype/switcher` draws it, its
 * projects with their monograms in their own colours, a waiting badge, the current one ringed,
 * open a project and the hide at the bottom; and the peek over the content. Each held to axe and
 * compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Fragment, useState } from "react";
import { expect, waitFor } from "storybook/test";
import { TOOLTIP_CLOSE_DELAY, TOOLTIP_DELAY } from "../tooltip/Tooltip";
import { Button } from "../button/Button";
import { Plus } from "lucide-react";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import {
  Rail,
  RailAction,
  RailList,
  RailProject,
  RailProjectWay,
  RailSpacer,
  SidebarIcon,
} from "./Rail";
import { useProjectTints } from "./projectColour";
import { AsStripTab, useTabStrip } from "../tabs/TabStrip";

/** `value`, which the story has drawn: it fails here, by name, where it is not. */
function drawn<T>(value: T | null | undefined, what: string): T {
  if (value === null || value === undefined) throw new Error(`${what} is not drawn`);
  return value;
}

/** The prototype's projects; one has work not saved, and one a save that is blocked (S8). */
const PROJECTS: {
  name: string;
  waiting: number;
  unsaved?: { blocked: boolean; says: string };
}[] = [
  { name: "charter", waiting: 1, unsaved: { blocked: false, says: "3 files to save" } },
  { name: "volaticloud", waiting: 2 },
  { name: "easydmarc-umbrella", waiting: 3 },
  { name: "elastic-press", waiting: 0, unsaved: { blocked: true, says: "The save is blocked" } },
  { name: "dotfiles", waiting: 0 },
  { name: "notes", waiting: 0 },
];

/** How each project's load is going at a launch (#5323): two still coming, one of them for a
 *  while, and one back with only some of its chats. */
const LOADS: Record<string, { loading: boolean; says?: string }> = {
  volaticloud: { loading: true, says: "Loading…" },
  "easydmarc-umbrella": { loading: true, says: "Still loading…" },
  "elastic-press": { loading: false, says: "2 of 3 chats back" },
};

/** The rows that fell short (#5324): what each says, and the ways it offers. */
const SHORT: Record<string, { says: string; failed?: boolean; dismisses?: boolean }> = {
  volaticloud: { says: "The folder is not readable.", failed: true, dismisses: true },
  "easydmarc-umbrella": { says: "Still loading…" },
  "elastic-press": { says: "2 of 3 chats back" },
  dotfiles: {
    says: "The record of what was open could not be read, so no chat was put back.",
    failed: true,
  },
};

function TheRail({
  peek = false,
  loading = false,
  short = false,
  waits,
  steps,
}: {
  peek?: boolean;
  loading?: boolean;
  short?: boolean;
  /** How many wait in each project, in the list's order, in place of the prototype's. */
  waits?: number[];
  /** The counts the first project's waiting steps through, one per press of "Next count". */
  steps?: number[];
}) {
  const [current, setCurrent] = useState("charter");
  const [step, setStep] = useState(0);
  const waitingOf = (name: string, at: number) =>
    steps !== undefined && at === 0
      ? steps[step % steps.length]
      : (waits?.[at] ?? PROJECTS.find((one) => one.name === name)?.waiting ?? 0);
  const [said, setSaid] = useState("Nothing pressed");
  const tintOf = useProjectTints();
  const strip = useTabStrip({ keys: PROJECTS.map((one) => one.name), selected: current });
  return (
    <div style={{ display: "flex", gap: "var(--space-6)", alignItems: "stretch" }}>
      <div
        style={{
          position: "relative",
          blockSize: "26rem",
          inlineSize: peek ? "12rem" : "52px",
          background: "var(--surface-deep)",
        }}
      >
        <Rail
          name="Projects"
          strip={strip}
          tabs={PROJECTS.map((one) => `rail-${one.name}`)}
          peek={peek}
        >
          <RailList>
            {PROJECTS.map((one, at) => (
              <Fragment key={one.name}>
                <AsStripTab strip={strip} tabKey={one.name} selected={one.name === current}>
                  {(tab) => (
                    <RailProject
                      {...tab}
                      id={`rail-${one.name}`}
                      name={one.name}
                      current={one.name === current}
                      waiting={waitingOf(one.name, at)}
                      unsaved={one.unsaved}
                      colour={tintOf(one.name)}
                      load={
                        short && SHORT[one.name] !== undefined
                          ? { loading: false, ...SHORT[one.name] }
                          : loading
                            ? LOADS[one.name]
                            : undefined
                      }
                      onClick={() => setCurrent(one.name)}
                    />
                  )}
                </AsStripTab>
                {short && SHORT[one.name] !== undefined && (
                  <RailProjectWay
                    doing="retry"
                    name={one.name}
                    onPress={() => setSaid(`Retry ${one.name}`)}
                  />
                )}
                {short && SHORT[one.name]?.dismisses && (
                  <RailProjectWay
                    doing="dismiss"
                    name={one.name}
                    onPress={() => setSaid(`Dismiss ${one.name}`)}
                  />
                )}
              </Fragment>
            ))}
            <RailAction
              aria-label="Open a project…"
              icon={<Plus aria-hidden="true" />}
              onClick={() => setSaid("Open a project")}
            />
          </RailList>
          <RailSpacer />
          {peek ? (
            <RailAction
              aria-label="Keep the projects sidebar shown"
              icon={<SidebarIcon />}
              onClick={() => setSaid("Kept")}
            />
          ) : (
            <RailAction
              aria-label="Hide the projects sidebar"
              tooltip="Shows again at the window's left edge"
              icon={<SidebarIcon />}
              onClick={() => setSaid("Hidden")}
            />
          )}
        </Rail>
      </div>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
      {steps !== undefined && (
        <Button variant="secondary" onPress={() => setStep((was) => was + 1)}>
          Next count
        </Button>
      )}
    </div>
  );
}

/** The tooltip family's one tooltip, or `null` while none is drawn. */
const tooltip = () => document.querySelector<HTMLElement>('[role="tooltip"][data-scope="tooltip"]');

/**
 * A hovered button's tooltip is drawn and says `says` (#3971). It opens only after
 * {@link TOOLTIP_DELAY}, so the wait is that plus room for a loaded runner, where `waitFor`'s own
 * second ran out before the tooltip was there and the text was asked of nothing. It still fails
 * when no tooltip is drawn, or one says something else.
 */
const untilTooltipSays = (says: string) =>
  waitFor(
    () => {
      const shown = tooltip();
      expect(shown, "a tooltip is drawn").not.toBeNull();
      expect(shown).toHaveTextContent(says);
    },
    { timeout: TOOLTIP_DELAY + 2_000 },
  );

/** No tooltip is drawn, given the room its close delay takes ({@link TOOLTIP_CLOSE_DELAY}). */
const untilNoTooltip = () =>
  waitFor(() => expect(tooltip()).toBeNull(), { timeout: TOOLTIP_CLOSE_DELAY + 2_000 });

/** Every animation in the document has finished: the picture is of a rail at rest. */
const atRest = () =>
  waitFor(() =>
    expect(document.getAnimations().filter((one) => one.playState === "running")).toHaveLength(0),
  );

const meta = {
  component: TheRail,
} satisfies Meta<typeof TheRail>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * Every button is 36px with corners of 9, the current one ringed; a press on another makes it
 * current with nothing moving; the hide is the last button, pressed by the keyboard.
 */
export const Shown: Story = {
  play: async ({ canvas, userEvent }) => {
    const tabs = canvas.getAllByRole("tab");
    for (const one of tabs) {
      const box = one.getBoundingClientRect();
      await expect(Math.round(box.height)).toBe(36);
      await expect(Math.round(box.width)).toBe(36);
    }
    await expect(tabs[0]).toHaveAttribute("data-selected");
    const before = tabs.map((one) => Math.round(one.getBoundingClientRect().top));
    await userEvent.click(tabs[1]);
    await waitFor(() => expect(tabs[1]).toHaveAttribute("data-selected"));
    await expect(tabs.map((one) => Math.round(one.getBoundingClientRect().top))).toEqual(before);

    // Its name is its tooltip, under the pointer, beside it (R89).
    await userEvent.hover(tabs[2]);
    await untilTooltipSays("easydmarc-umbrella");
    await userEvent.unhover(tabs[2]);
    await untilNoTooltip();

    const hide = canvas.getByRole("button", { name: "Hide the projects sidebar" });
    hide.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Hidden"));
    // Its tooltip, which the keyboard opened, taken down by Esc: the picture is of the rail.
    await userEvent.keyboard("{Escape}");
    await untilNoTooltip();
    await atRest();
  },
};

/** The peek over the content: 56px, its right corners rounded, its last button keeps it. */
export const Peek: Story = {
  args: { peek: true },
  play: async ({ canvas, userEvent }) => {
    const keep = canvas.getByRole("button", { name: "Keep the projects sidebar shown" });
    const column = keep.closest<HTMLElement>('[data-part="column"]');
    await waitFor(() => expect(Math.round(column?.getBoundingClientRect().width ?? 0)).toBe(56));
    await userEvent.click(keep);
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Kept"));
    // The pointer off the rail, so no button is drawn under it, and its slide in over.
    await userEvent.unhover(keep);
    await untilNoTooltip();
    await atRest();
  },
};

/**
 * The keyboard on the current project (S2): its focus ring is drawn inside the button, its
 * selection ring outside, so the list's clip and the next button leave both whole; nothing
 * changes size.
 */
export const FocusedCurrent: Story = {
  play: async ({ canvas, userEvent }) => {
    const current = canvas.getAllByRole("tab")[0];
    const before = current.getBoundingClientRect();
    await userEvent.tab();
    await expect(current).toHaveFocus();
    await expect(current).toHaveAttribute("data-focus-visible");
    const style = getComputedStyle(current);
    // Inside the button: nothing it draws reaches past the selection ring.
    await expect(Number.parseFloat(style.outlineOffset)).toBeLessThan(0);
    const after = current.getBoundingClientRect();
    await expect([after.width, after.height]).toEqual([before.width, before.height]);
    // Its tooltip, which the keyboard opened, taken down by Esc: the picture is of the rings.
    await userEvent.keyboard("{Escape}");
    await untilNoTooltip();
    await atRest();
  },
};

/**
 * **A project whose load fell short** (#5324): one that failed has a "!" out of its corner and says
 * its error as its description, and it, one still loading after 15 s and one short of chats each
 * have a Retry under the button; one whose open failed has a Dismiss too. Nothing changes size.
 */
export const FellShort: Story = {
  args: { short: true },
  play: async ({ canvas, userEvent }) => {
    const failed = canvas.getByRole("tab", { name: "volaticloud" });
    await expect(failed).not.toHaveAttribute("aria-busy");
    await expect(failed).toHaveAccessibleDescription("The folder is not readable., 2 waiting");
    await expect(canvas.getByRole("tab", { name: "elastic-press" })).toHaveAccessibleDescription(
      "2 of 3 chats back, saving is blocked",
    );
    for (const one of canvas.getAllByRole("tab")) {
      const box = one.getBoundingClientRect();
      await expect([Math.round(box.width), Math.round(box.height)]).toEqual([36, 36]);
    }
    // The way off a row is the next Tab stop, and Enter presses it.
    const retry = canvas.getByRole("button", { name: "Retry volaticloud" });
    retry.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Retry volaticloud"));
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Dismiss volaticloud" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent("Dismiss volaticloud"),
    );
    await userEvent.keyboard("{Escape}");
    await atRest();
  },
};

/**
 * **Projects coming back at a launch** (#5323): a loading project has a dashed ring inside its
 * button, which a shape tells from the current one's solid ring, is busy, and says how its load is
 * going as its description; one back with only some of its chats says how many. The keyboard
 * reaches a loading project as any other, and nothing changes size.
 */
export const Loading: Story = {
  args: { loading: true },
  play: async ({ canvas, userEvent }) => {
    const loading = canvas.getByRole("tab", { name: "volaticloud" });
    await expect(loading).toHaveAttribute("aria-busy", "true");
    await expect(loading).toHaveAccessibleDescription("Loading…, 2 waiting");
    await expect(
      canvas.getByRole("tab", { name: "easydmarc-umbrella" }),
    ).toHaveAccessibleDescription("Still loading…, 3 waiting");
    const partly = canvas.getByRole("tab", { name: "elastic-press" });
    await expect(partly).not.toHaveAttribute("aria-busy");
    await expect(partly).toHaveAccessibleDescription("2 of 3 chats back, saving is blocked");
    for (const one of canvas.getAllByRole("tab")) {
      const box = one.getBoundingClientRect();
      await expect([Math.round(box.width), Math.round(box.height)]).toEqual([36, 36]);
    }
    loading.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(loading).toHaveAttribute("data-selected"));
    // Its tooltip, which the keyboard opened, taken down by Esc: the picture is of the marks.
    await userEvent.keyboard("{Escape}");
    await untilNoTooltip();
    await atRest();
  },
};

/** The round count on a tab (#6342), or `null` where none is drawn. */
const roundOn = (tab: HTMLElement) =>
  tab.querySelector<HTMLElement>('[data-scope="count"][data-part="round"]');

/**
 * **Each tab's count is one small circle in the ✋'s colour** (#6342): 0 draws none, 1 and 9 as
 * they are, 10 and 120 as 9+, every circle 16px; each tab says the real number.
 */
export const RoundCounts: Story = {
  args: { waits: [0, 1, 9, 10, 120, 0] },
  play: async ({ canvas, userEvent }) => {
    const tabs = canvas.getAllByRole("tab");
    await expect(roundOn(tabs[0])).toBeNull();
    await expect(tabs.slice(1, 5).map((tab) => roundOn(tab)?.dataset.drawn)).toEqual([
      "1",
      "9",
      "9+",
      "9+",
    ]);
    for (const tab of tabs.slice(1, 5)) {
      const box = drawn(roundOn(tab), "a tab's count").getBoundingClientRect();
      await expect([Math.round(box.width), Math.round(box.height)]).toEqual([16, 16]);
    }
    await expect(tabs[4]).toHaveAccessibleDescription("120 waiting");
    await expect(tabs[3]).toHaveAccessibleDescription(/^10 waiting/);
    await userEvent.tab();
    await untilFocused(() => tabs[0], "Tab lands on the first project");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => tabs[1], "↓ to the next");
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await untilNoTooltip();
  },
};

/**
 * **A tab's changed count slides once, and the tab is not drawn again** (#6342): 2 to 3 slides
 * up, 3 to 12 slides again (its other keyframes), 12 to 40 is 9+ both times and does not.
 */
export const CountSlides: Story = {
  args: { steps: [2, 3, 12, 40] },
  parameters: { screenshot: false },
  play: async ({ canvas, userEvent }) => {
    const tab = canvas.getAllByRole("tab")[0];
    const circle = drawn(roundOn(tab), "a tab's count");
    const next = canvas.getByRole("button", { name: "Next count" });
    const slide = () => getComputedStyle(circle, "::before").animationName;
    await expect(slide()).toBe("none");
    next.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(circle).toHaveAttribute("data-drawn", "3"));
    await expect(slide()).toBe("count-round-up-a");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(circle).toHaveAttribute("data-drawn", "9+"));
    await expect(slide()).toBe("count-round-up-b");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(tab).toHaveAccessibleDescription(/^40 waiting/));
    await expect(slide()).toBe("count-round-up-b");
    await expect(roundOn(tab)).toBe(circle);
    await expect(canvas.getAllByRole("tab")[0]).toBe(tab);
  },
};
