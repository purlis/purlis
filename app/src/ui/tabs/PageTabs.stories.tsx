// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Page tabs** (#3668, the skill page prototype's variant A): a skill's five tabs with their
 * counts, walked by the keyboard, held to axe in Chromium and WebKit in every cell, and measured
 * so that choosing a tab, or a count going from 9 to 10, moves no tab. Narrow, they scroll along
 * their row and never wrap.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Button } from "../button/Button";
import { PageTabs } from "./PageTabs";

function Skill({ width }: { width: number }) {
  const [tab, setTab] = useState("overview");
  const [grades, setGrades] = useState(9);
  return (
    <div style={{ inlineSize: width }}>
      <Button size="sm" onPress={() => setGrades((was) => was + 1)}>
        One more grade
      </Button>
      <PageTabs
        label="deploy's sections"
        tabs={[
          { id: "overview", label: "Overview" },
          { id: "text", label: "Text" },
          { id: "history", label: "History", count: 4, countSaid: "4 versions" },
          { id: "grades", label: "Grades", count: grades, countSaid: `${grades} grades` },
          { id: "runs", label: "Runs", count: 41, countSaid: "41 runs" },
        ]}
        selected={tab}
        onSelect={setTab}
      >
        {(id) => (
          <p style={{ margin: 0, padding: "var(--space-6)" }}>{`deploy's ${id}, as the page draws it.`}</p>
        )}
      </PageTabs>
    </div>
  );
}

const meta = { component: Skill } satisfies Meta<typeof Skill>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Each tab's box, rounded, to tell that none moved. */
function boxes(root: HTMLElement): string {
  return [...root.querySelectorAll('[role="tab"]')]
    .map((one) => {
      const box = one.getBoundingClientRect();
      return `${Math.round(box.left)} ${Math.round(box.width)}`;
    })
    .join(", ");
}

/** **Five tabs with counts**: one Tab stop, the arrows choose, and nothing moves. */
export const SkillPage: Story = {
  args: { width: 640 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const before = boxes(canvasElement);
    const overview = canvas.getByRole("tab", { name: "Overview" });
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => overview, "the chosen tab");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
    const grades = canvas.getByRole("tab", { name: "Grades" });
    await untilFocused(() => grades, "Grades");
    await waitFor(() => expect(grades).toHaveAttribute("aria-selected", "true"));
    await expect(canvas.getByRole("tabpanel")).toHaveAccessibleName("Grades");
    await expect(boxes(canvasElement)).toBe(before);
    // One Tab stop: Tab leaves the tabs for the panel.
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("tabpanel"), "the panel");
    // A count from 9 to 10 widens nothing.
    await userEvent.click(canvas.getByRole("button", { name: "One more grade" }));
    await waitFor(() => expect(grades).toHaveAccessibleDescription("10 grades"));
    await expect(boxes(canvasElement)).toBe(before);
    // From the button the count was changed by, Tab lands on the chosen tab again.
    await userEvent.tab();
    await untilFocused(() => grades, "back on the chosen tab");
    await userEvent.keyboard("{End}");
    await waitFor(() =>
      expect(canvas.getByRole("tab", { name: "Runs" })).toHaveAttribute("aria-selected", "true"),
    );
  },
};

/** **Narrow**: the tabs keep one row and scroll along it, the keyboard bringing each into view. */
export const Narrow: Story = {
  args: { width: 220 },
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("tablist");
    const tabs = canvas.getAllByRole("tab");
    const tops = new Set(tabs.map((one) => Math.round(one.getBoundingClientRect().top)));
    await expect(tops.size).toBe(1);
    await expect(list.scrollWidth).toBeGreaterThan(list.clientWidth);
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.keyboard("{End}");
    await untilFocused(() => canvas.getByRole("tab", { name: "Runs" }), "the last tab");
    expectFocusInSight();
  },
};
