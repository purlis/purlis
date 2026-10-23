// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The status item family and the status bar** (#2913, N28, R115): the status pills at the
 * title bar's right in each variant, the bar's two ends ordered by priority, and its menu, where
 * the person hides an item and shows it again, by the keyboard. Each held to axe and compared with
 * its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { ArrowUpCircle, Bell, Hand, Info, OctagonX } from "lucide-react";
import { expectFocusInSight } from "../../stories.testkit";
import { drawWindowText, LEAST_TEXT } from "../../textSize";
import { StatusBar, type StatusContribution } from "./StatusBar";
import { StatusCell, StatusItem } from "./StatusItem";

/**
 * The title bar's right end, in the bar's order (#2913): what comes and goes on the inner side
 * (what waits, an update on offer), then the save, the kill switch and About at the end. Right
 * aligned, as the bar is, so a pill that appears moves only what is left of it.
 */
function Pills() {
  const [said, setSaid] = useState("Nothing pressed");
  const [offered, setOffered] = useState(false);
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <div
        role="group"
        aria-label="Status pills"
        style={{
          display: "flex",
          justifyContent: "flex-end",
          inlineSize: "36rem",
          gap: "var(--space-3)",
          padding: "var(--space-3)",
          background: "var(--surface-deep)",
        }}
      >
        <StatusItem
          variant="waiting"
          aria-label="2 things wait on you"
          icon={<Hand aria-hidden="true" />}
          count={2}
          onPress={() => setSaid("Inbox")}
        />
        {offered && (
          <StatusItem
            variant="accent"
            icon={<ArrowUpCircle aria-hidden="true" />}
            onPress={() => setSaid("Updates")}
          >
            0.4.2-dev.1037
          </StatusItem>
        )}
        <StatusItem
          variant="pill"
          tone="success"
          icon={<Bell aria-hidden="true" />}
          onPress={() => setSaid("Saving")}
        >
          Saved
        </StatusItem>
        <StatusItem
          variant="pill"
          aria-label="Stop every chat"
          icon={<OctagonX aria-hidden="true" />}
          onPress={() => setSaid("Stopped")}
        />
        <StatusItem
          variant="pill"
          icon={<Info aria-hidden="true" />}
          onPress={() => setSaid("About")}
        >
          About
        </StatusItem>
      </div>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
      <button type="button" onClick={() => setOffered((was) => !was)}>
        Offer an update
      </button>
    </div>
  );
}

const meta = {
  component: Pills,
} satisfies Meta<typeof Pills>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * Every pill is 24px tall with full corners, one Tab stop each; Enter and Space press it. An
 * update arriving on the inner side moves none of the pills that stay.
 */
export const TitleBarPills: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = canvas.getByRole("group", { name: "Status pills" });
    const pills = within(group).getAllByRole("button");
    for (const one of pills) {
      const box = one.getBoundingClientRect();
      await expect(Math.round(box.height)).toBe(24);
      await expect(getComputedStyle(one).borderTopLeftRadius).toBe("9999px");
    }
    await userEvent.tab();
    await expect(pills[0]).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Inbox"));
    await userEvent.tab();
    await expect(pills[1]).toHaveFocus();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Saving"));
    for (const one of pills.slice(2)) {
      await userEvent.tab();
      await expect(one).toHaveFocus();
      expectFocusInSight();
    }

    const staying = ["Saved", "Stop every chat", "About"].map((name) =>
      within(group).getByRole("button", { name }),
    );
    const before = staying.map((one) => Math.round(one.getBoundingClientRect().right));
    await userEvent.click(canvas.getByRole("button", { name: "Offer an update" }));
    await waitFor(() =>
      expect(within(group).getByRole("button", { name: "0.4.2-dev.1037" })).toBeVisible(),
    );
    const after = staying.map((one) => Math.round(one.getBoundingClientRect().right));
    await expect(after).toEqual(before);
    await userEvent.click(canvas.getByRole("button", { name: "Offer an update" }));
  },
};

const ITEMS = (open: () => void): StatusContribution[] => [
  // Given out of order: the bar orders each end by priority.
  {
    id: "purlis.todos",
    name: "Todos",
    side: "left",
    priority: 80,
    item: <StatusCell count={3}>todo</StatusCell>,
  },
  {
    id: "purlis.project",
    name: "Project",
    side: "left",
    priority: 100,
    separated: true,
    item: <StatusCell strong>purlis</StatusCell>,
  },
  {
    id: "extension:prs/open",
    name: "PRs (Forge)",
    side: "left",
    priority: 40,
    label: "PRs",
    value: "2",
    tooltip: "PRs, from Forge's facts file",
  },
  {
    id: "purlis.notices",
    name: "Notices",
    side: "right",
    priority: 30,
    hideable: false,
    item: (
      <StatusItem
        aria-label="Notices: 2"
        tone="waiting"
        icon={<Bell aria-hidden="true" />}
        count={2}
        onPress={open}
      >
        Notices
      </StatusItem>
    ),
  },
];

/** The bar, whose hidden items are the story's to keep, as the window keeps them. */
function Bar() {
  const [hidden, setHidden] = useState<string[]>([]);
  const [opened, setOpened] = useState(0);
  return (
    <div style={{ inlineSize: "36rem", display: "grid", gap: "var(--space-3)" }}>
      <StatusBar
        aria-label="Status"
        items={ITEMS(() => setOpened((was) => was + 1))}
        hidden={hidden}
        onHide={(id, hide) =>
          setHidden((was) => (hide ? [...was, id] : was.filter((one) => one !== id)))
        }
      />
      <p role="status" style={{ margin: 0 }}>{`Inbox opened ${opened} times`}</p>
    </div>
  );
}

/**
 * Context on the left by priority, health on the right; a cell with a hint is a Tab stop that
 * shows it; the menu key on an item opens the bar's
 * menu, a check hides an item, the same check shows it again, and Escape closes the menu.
 */
export const StatusBarAndItsMenu: Story = {
  render: () => <Bar />,
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const bar = canvas.getByRole("contentinfo", { name: "Status" });
    await expect((bar.textContent ?? "").replace(/\s+/g, " ")).toMatch(
      /^purlistodo ?3PRs ?2Notices/,
    );
    const notices = canvas.getByRole("button", { name: "Notices: 2" });
    // A cell with a hint is a Tab stop, so its hint is the keyboard's too (#2908): the PRs
    // badge comes first, its tooltip open, and then Notices.
    const prs = within(bar).getByText("PRs").closest<HTMLElement>('[data-part="root"]');
    await userEvent.tab();
    await expect(prs).toHaveFocus();
    await waitFor(() =>
      expect(within(document.body).getByRole("tooltip")).toHaveTextContent(
        "PRs, from Forge's facts file",
      ),
    );
    await userEvent.tab();
    await expect(notices).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("opened 1 times"));
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await within(document.body).findByRole("menu");
    const todos = within(menu).getByRole("menuitemcheckbox", { name: "Todos" });
    await expect(todos).toHaveAttribute("aria-checked", "true");
    todos.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(bar.textContent).not.toContain("todo"));
    // The menu gives the keyboard back to the item it was opened from.
    await waitFor(() => expect(notices).toHaveFocus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const again = await within(document.body).findByRole("menuitemcheckbox", { name: "Todos" });
    await expect(again).toHaveAttribute("aria-checked", "false");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(within(document.body).queryByRole("menu")).toBeNull());
    await expect(notices).toHaveFocus();
  },
};

/** A bar whose pin comes and goes, and whose project name is longer than its room. */
function Narrow({ pinned }: { pinned: boolean }) {
  const items: StatusContribution[] = [
    {
      id: "purlis.project",
      name: "Project",
      side: "left",
      priority: 100,
      separated: true,
      item: (
        <StatusCell strong shrinks data-testid="project">
          purlis-infrastructure-migration-of-the-whole-estate-and-more
        </StatusCell>
      ),
    },
    {
      id: "purlis.workspace",
      name: "Workspace",
      side: "left",
      priority: 90,
      item: (
        <StatusCell strong data-testid="workspace">
          alpha
        </StatusCell>
      ),
    },
    {
      id: "purlis.notices",
      name: "Notices",
      side: "right",
      priority: 30,
      hideable: false,
      item: <StatusItem icon={<Bell aria-hidden="true" />}>Notices</StatusItem>,
    },
  ];
  if (pinned)
    items.push({
      id: "purlis.pin",
      name: "Pin",
      side: "right",
      priority: 40,
      item: <StatusItem tone="waiting">Pin 0.4.1</StatusItem>,
    });
  return (
    <div style={{ inlineSize: "22rem" }}>
      <StatusBar aria-label="Status" items={items} />
    </div>
  );
}

/** The narrow bar, and a way to make its pin drift. */
function Drifting() {
  const [pinned, setPinned] = useState(false);
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <Narrow pinned={pinned} />
      <button type="button" onClick={() => setPinned((was) => !was)}>
        Drift the pin
      </button>
    </div>
  );
}

/**
 * A long project name gives way, cut with an ellipsis, and the workspace stays in sight; a pin that
 * starts to drift arrives inside Notices, which stays where it was.
 */
export const NarrowWithAPinArriving: Story = {
  render: () => <Drifting />,
  play: async ({ canvas, userEvent }) => {
    const label = canvas
      .getByTestId("project")
      .querySelector<HTMLElement>('[data-part="label"]') as HTMLElement;
    await expect(label.scrollWidth).toBeGreaterThan(label.clientWidth);
    const bar = canvas.getByRole("contentinfo", { name: "Status" }).getBoundingClientRect();
    const workspace = canvas.getByTestId("workspace").getBoundingClientRect();
    await expect(workspace.right).toBeLessThanOrEqual(bar.right);
    await expect(workspace.width).toBeGreaterThan(0);
    const before = canvas.getByRole("button", { name: "Notices" }).getBoundingClientRect().right;
    await userEvent.click(canvas.getByRole("button", { name: "Drift the pin" }));
    await waitFor(() => expect(canvas.getByRole("button", { name: "Pin 0.4.1" })).toBeVisible());
    const after = canvas.getByRole("button", { name: "Notices" }).getBoundingClientRect().right;
    await expect(Math.round(after)).toBe(Math.round(before));
  },
};

/** At the smallest text size a person can set, every pill is still a 24px target. */
export const PillsAtTheSmallestText: Story = {
  render: () => (
    <div style={{ display: "flex", gap: "var(--space-3)" }}>
      <StatusItem
        variant="pill"
        aria-label="Stop every chat"
        icon={<OctagonX aria-hidden="true" />}
      />
      <StatusItem variant="pill" icon={<Info aria-hidden="true" />}>
        About
      </StatusItem>
    </div>
  ),
  play: async ({ canvas }) => {
    const root = document.documentElement;
    const was = root.style.fontSize;
    // The smallest the text-size setting goes, drawn as the window draws it.
    drawWindowText(LEAST_TEXT, root);
    try {
      for (const pill of canvas.getAllByRole("button")) {
        const box = pill.getBoundingClientRect();
        await expect(box.height).toBeGreaterThanOrEqual(24);
        await expect(box.width).toBeGreaterThanOrEqual(24);
      }
    } finally {
      root.style.fontSize = was;
    }
  },
};
