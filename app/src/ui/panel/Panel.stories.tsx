// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The panel family and its sections** (#2913): region panels flush, as Purlis Classic draws
 * them, and floating, as the Purlis theme's `window.gutter` 8 and `panel.corner-radius` 12 draw
 * them (#2805); and a section with its heading line, its title and an action, walked by the
 * keyboard. Each held to axe and compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type CSSProperties } from "react";
import { expect, waitFor } from "storybook/test";
import { ListTodo, Plus } from "lucide-react";
import { expectFocusInSight } from "../../stories.testkit";
import { IconButton } from "../button/Button";
import { Panel, PanelSection, PanelSectionHead, PanelSectionTitle } from "./Panel";

/**
 * Three panels in a window's frame: a side, the content and a half of the bottom panel, the gap
 * between them the theme's gutter, as the window's regions draw it (`App.css`). `geometry` stands
 * in for a theme's window geometry.
 */
function Window({ geometry }: { geometry?: CSSProperties }) {
  const gap = "var(--window-gutter)";
  return (
    <div
      style={{
        ...geometry,
        display: "grid",
        gridTemplateColumns: "10rem 18rem",
        gridTemplateRows: "8rem 5rem",
        gap,
        padding: `0 ${gap} ${gap}`,
        background: "var(--surface-deep)",
      }}
    >
      <Panel variant="side" aria-label="Side" role="region">
        <PanelSection>
          <PanelSectionHead>
            <PanelSectionTitle mark={ListTodo}>Todos</PanelSectionTitle>
          </PanelSectionHead>
        </PanelSection>
      </Panel>
      <Panel variant="content" aria-label="Content" role="region">
        <p style={{ margin: "var(--space-4)" }}>The chats and the views open in the middle.</p>
      </Panel>
      <Panel variant="bottom" aria-label="Bottom" role="region">
        <PanelSection>
          <PanelSectionHead>
            <PanelSectionTitle>Memory</PanelSectionTitle>
          </PanelSectionHead>
        </PanelSection>
      </Panel>
      <Panel variant="bottom" aria-label="Bottom right" role="region">
        <PanelSection>
          <PanelSectionHead>
            <PanelSectionTitle>History</PanelSectionTitle>
          </PanelSectionHead>
        </PanelSection>
      </Panel>
    </div>
  );
}

const meta = {
  component: Window,
} satisfies Meta<typeof Window>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Purlis Classic: no gutter, square corners, panels flush. */
export const Flush: Story = {
  args: {},
  play: async ({ canvas }) => {
    const side = canvas.getByRole("region", { name: "Side" });
    await expect(side).toHaveAttribute("data-variant", "side");
    await expect(getComputedStyle(side).borderTopLeftRadius).toBe("0px");
  },
};

/** The Purlis theme's geometry: panels 8 apart, 12px corners, no hairline. */
export const Floating: Story = {
  args: {
    geometry: {
      "--window-gutter": "8px",
      "--panel-corner-radius": "12px",
      "--panel-border-width": "0px",
    } as CSSProperties,
  },
  play: async ({ canvas }) => {
    const content = canvas.getByRole("region", { name: "Content" });
    await expect(getComputedStyle(content).borderTopLeftRadius).toBe("12px");
    const side = canvas.getByRole("region", { name: "Side" });
    const gap = content.getBoundingClientRect().left - side.getBoundingClientRect().right;
    await expect(Math.round(gap)).toBe(8);
  },
};

/** A section's heading line with its action: Tab reaches the action, Enter runs it. */
function SectionWithAnAction() {
  const [made, setMade] = useState(0);
  return (
    <Panel variant="side" aria-label="Personas" role="region">
      <div style={{ padding: "var(--space-3)" }}>
        <PanelSection>
          <PanelSectionHead>
            <PanelSectionTitle mark={ListTodo}>Personas</PanelSectionTitle>
            <IconButton
              size="sm"
              aria-label="New persona"
              icon={<Plus aria-hidden="true" />}
              onPress={() => setMade((was) => was + 1)}
            />
          </PanelSectionHead>
          <p role="status" style={{ margin: 0 }}>{`${made} made`}</p>
        </PanelSection>
      </div>
    </Panel>
  );
}

export const SectionWithItsAction: Story = {
  render: () => <SectionWithAnAction />,
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByRole("heading", { name: "Personas", level: 2 })).toBeInTheDocument();
    const add = canvas.getByRole("button", { name: "New persona" });
    await userEvent.tab();
    await expect(add).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("1 made"));
    await userEvent.tab({ shift: true });
    await expect(add).not.toHaveFocus();
  },
};
