// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A sidebar view on the panel family's view** (#3340, the sidebar-views prototype's Q1 A): one
 * scroll; the heading line with its actions, then the field, held at the top on the panel's fill;
 * each group's name held under the top; 30px rows. A long list and a short one, walked by the
 * keyboard in the order the top stacks them, and scrolled to show what stays put.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { Archive, Plus, Search } from "lucide-react";
import { expectFocusInSight, untilFocusWithin } from "../../stories.testkit";
import { IconButton } from "../button/Button";
import { TextField } from "../field/Field";
import { List, ListRow, ListRowSection } from "../list/List";
import { RowLabel, RowTrail } from "../list/Row";
import { InPanelTop, Panel, PanelSection, PanelSectionHead, PanelTitle } from "./Panel";

const two = (n: number) => String(n).padStart(2, "0");

const DAYS: [string, string[]][] = [
  [
    "Today",
    [
      "Follow main CI after every merge",
      "Merge only PRs tested on current main",
      "gh pr ready starts more checks",
      "Sandboxed agents cannot run terminal-bound tests",
    ],
  ],
  [
    "Yesterday",
    [
      "Throttle parallel agents by machine load",
      "A copied node_modules hangs on macOS",
      "Never run an unguarded cleanup script",
      "Agent worktree targets fill the disk",
      "Worktree git config writes the shared clone",
    ],
  ],
  [
    "Earlier",
    [
      "Grill and decide, escalate only major calls",
      "Never idle the loop on pending asks",
      "A stale PR is yours to land",
      "Never lose ideas or findings",
      "Separate PRs, not trains",
      "Dev releases are fine, stable is his",
    ],
  ],
];

/** A view in a side panel 276px wide, its region the one scroll, as `Panels.tsx` draws it. */
function View({ days }: { days: [string, string[]][] }) {
  const [query, setQuery] = useState("");
  const [made, setMade] = useState(0);
  const wanted = query.trim().toLowerCase();
  return (
    <>
      <p role="status" style={{ margin: 0 }}>{`${made} made`}</p>
      <Panel variant="side" aria-label="Memory" role="region" data-testid="side">
        <div
          data-testid="scroll"
          style={{ blockSize: "20rem", inlineSize: "276px", overflow: "auto" }}
        >
          <PanelSection
            view
            top={
              <PanelSectionHead>
                <PanelTitle>Memory</PanelTitle>
                <IconButton
                  size="sm"
                  aria-label="New memory"
                  icon={<Plus aria-hidden="true" />}
                  onPress={() => setMade((was) => was + 1)}
                />
                <IconButton size="sm" aria-label="Archive" icon={<Archive aria-hidden="true" />} />
              </PanelSectionHead>
            }
          >
            <InPanelTop>
              <TextField
                type="search"
                icon={<Search aria-hidden="true" />}
                aria-label="Search memory"
                placeholder="Search memory"
                value={query}
                onChange={setQuery}
              />
            </InPanelTop>
            <List aria-label="Memory" size="sm">
              {days.map(([day, rows], group) => {
                const shown = rows.filter((one) => one.toLowerCase().includes(wanted));
                return (
                  <ListRowSection key={day} id={day} title={day} named count={shown.length}>
                    {shown.map((one, at) => (
                      <ListRow key={one} id={one} textValue={one}>
                        <RowLabel>{one}</RowLabel>
                        {/* The day it was written, quiet at the row's end (Q1 A's `.rm`). */}
                        <RowTrail className="ui-text-meta">{`10-${two(26 - group * 3 - at)}`}</RowTrail>
                      </ListRow>
                    ))}
                  </ListRowSection>
                );
              })}
            </List>
          </PanelSection>
        </div>
      </Panel>
    </>
  );
}

const meta = { component: View } satisfies Meta<typeof View>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A long list: the top and the current group's name stay put while the rows scroll. */
export const LongList: Story = {
  args: { days: DAYS },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const scroll = canvas.getByTestId("scroll");
    const top = canvasElement.querySelector<HTMLElement>('[data-part="top"]');
    if (top === null) throw new Error("the view has no top");
    // Rows are the control's height and a step: 30px at the comfortable density.
    const first = canvas.getByRole("row", { name: DAYS[0][1][0] });
    const probe = document.createElement("div");
    probe.style.blockSize = "calc(var(--size-control) + var(--space-1))";
    canvasElement.append(probe);
    const tall = probe.getBoundingClientRect().height;
    probe.remove();
    await expect(Math.round(first.getBoundingClientRect().height)).toBe(Math.round(tall));
    // Tab: the heading's actions, then the field, then the rows.
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "New memory" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Archive" })).toHaveFocus();
    await userEvent.tab();
    await expect(canvas.getByRole("searchbox", { name: "Search memory" })).toHaveFocus();
    await userEvent.tab();
    // On a row, and the frame after it (#4026): React Aria scrolls the row the keyboard moved to
    // into sight on that frame, which would take back a scroll made before it, and the picture.
    await untilFocusWithin(() => first, "the first row, by Tab");
    // One scroll: the list itself does not scroll, the region does, and the top stays put.
    const list = canvas.getByRole("grid");
    await expect(list.scrollHeight).toBeLessThanOrEqual(list.clientHeight + 1);
    const at = top.getBoundingClientRect().top;
    scroll.scrollTop = 200;
    const scrolled = scroll.scrollTop;
    await expect(scrolled).toBeGreaterThan(0);
    // It stays there: nothing is left to move it before the picture.
    await new Promise((frame) => requestAnimationFrame(frame));
    await new Promise((frame) => requestAnimationFrame(frame));
    await expect(scroll.scrollTop).toBe(scrolled);
    await expect(Math.round(top.getBoundingClientRect().top)).toBe(Math.round(at));
    // The group in sight has its name held right under the top.
    const held = [...canvasElement.querySelectorAll<HTMLElement>('[data-part="header"]')].find(
      (one) => Math.abs(one.getBoundingClientRect().top - top.getBoundingClientRect().bottom) < 1,
    );
    await expect(held).toBeDefined();
  },
};

/** A short list: the same top, nothing to scroll. */
export const ShortList: Story = {
  args: { days: [DAYS[0]] },
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("1 made"));
    const scroll = canvas.getByTestId("scroll");
    await expect(scroll.scrollHeight).toBeLessThanOrEqual(scroll.clientHeight + 1);
  },
};
