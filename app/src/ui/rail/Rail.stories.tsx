// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The project rail family** (#2915, R76–R89): the rail as `prototype/switcher` draws it, its
 * projects with their monograms in their own colours, a waiting badge, the current one ringed,
 * open a project and the hide at the bottom; and the peek over the content. Each held to axe and
 * compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { TOOLTIP_CLOSE_DELAY, TOOLTIP_DELAY } from "../tooltip/Tooltip";
import { Plus } from "lucide-react";
import { expectFocusInSight } from "../../stories.testkit";
import { Rail, RailAction, RailList, RailProject, RailSpacer, SidebarIcon } from "./Rail";
import { useProjectTints } from "./projectColour";

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

function TheRail({ peek = false }: { peek?: boolean }) {
  const [current, setCurrent] = useState("charter");
  const [said, setSaid] = useState("Nothing pressed");
  const tintOf = useProjectTints();
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
        <Rail name="Projects" tabs={PROJECTS.map((one) => `rail-${one.name}`)} peek={peek}>
          <RailList>
            {PROJECTS.map((one) => (
              <RailProject
                key={one.name}
                id={`rail-${one.name}`}
                role="tab"
                aria-selected={one.name === current}
                name={one.name}
                current={one.name === current}
                waiting={one.waiting}
                unsaved={one.unsaved}
                colour={tintOf(one.name)}
                onClick={() => setCurrent(one.name)}
              />
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
