// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The card family** (#2912): a card with a title and its words, and an ask that holds Allow
 * beside Keep blocked, walked by the keyboard, on the panel's ground it stands on; a skill's
 * Overview facts, the grade plan's four totals, wide and narrow, and a running grade's counts,
 * which change without moving a tile (#3673); and a card pressed whole, the welcome's doors and a
 * repo to get (#3655). Each held to axe and compared with its baseline
 * (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { CloudDownload, FolderOpen, FolderPlus } from "lucide-react";
import { expect, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Badge } from "../badge/Badge";
import { Button, ButtonRow } from "../button/Button";
import { Card, CardTitle } from "./Card";
import { CardButton } from "./CardButton";
import { Fact, Facts } from "./Facts";
import { Tile, Totals } from "./Totals";

/** A side panel's ground, which a card stands one step off. */
function OnAPanel({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        inlineSize: "22rem",
        padding: "var(--space-5)",
        background: "var(--surface-raised)",
      }}
    >
      {children}
    </div>
  );
}

/** An ask: its words, then Allow and Keep blocked, which say what was answered. */
function AnAsk() {
  const [said, setSaid] = useState("Nothing answered yet");
  return (
    <OnAPanel>
      <Card variant="ask" aria-label="steward 4 asks">
        <CardTitle>steward 4 wants to reach api.github.com</CardTitle>
        <p style={{ margin: 0 }}>{said}</p>
        <ButtonRow size="sm">
          <Button variant="grants" onPress={() => setSaid("Allowed for this chat")}>
            Allow for this chat
          </Button>
          <Button onPress={() => setSaid("Kept blocked")}>Keep blocked</Button>
        </ButtonRow>
      </Card>
    </OnAPanel>
  );
}

const meta = {
  component: AnAsk,
} satisfies Meta<typeof AnAsk>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab reaches Allow and then Keep blocked, each in sight; a press says what was answered. */
export const Ask: Story = {
  play: async ({ canvas, userEvent }) => {
    const allow = canvas.getByRole("button", { name: "Allow for this chat" });
    const keep = canvas.getByRole("button", { name: "Keep blocked" });
    await userEvent.tab();
    await expect(allow).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(keep).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByText("Kept blocked")).toBeInTheDocument());
  },
};

/** A card that only says something: its title and its words, no edge of the ask's. */
export const Raised: Story = {
  render: () => (
    <OnAPanel>
      <Card aria-label="A report">
        <CardTitle>The sample project saved</CardTitle>
        <p style={{ margin: 0 }}>3 files on purlis/save/this-machine, pushed a minute ago.</p>
      </Card>
    </OnAPanel>
  ),
  play: async ({ canvas }) => {
    await expect(canvas.getByText("The sample project saved")).toBeInTheDocument();
    await expect(canvas.queryByRole("button")).toBeNull();
  },
};

/** **A skill's Overview** (#3673): who gets it, with its fix where it is left out, and the rest. */
function Overview({ inlineSize }: { inlineSize: string }) {
  const [said, setSaid] = useState("Nothing granted yet");
  return (
    <div style={{ inlineSize, maxInlineSize: "100%" }}>
      <Facts aria-label="About deploy">
        <Fact label="Who gets it">
          Left out of steward 5: its persona has no grant for it.
          <Button size="sm" onPress={() => setSaid("Granted to steward")}>
            Grant it to steward
          </Button>
        </Fact>
        <Fact label="Where it lives">
          <code>.claude/skills/deploy/SKILL.md</code>
        </Fact>
        <Fact label="Owner">you</Fact>
        <Fact label="Grade">
          86% with it, 68% without <Badge variant="accent">sonnet</Badge>
        </Fact>
        <Fact label="Runs">14 in the last 30 days</Fact>
      </Facts>
      <p role="status" style={{ marginBlockStart: "var(--space-5)" }}>
        {said}
      </p>
    </div>
  );
}

/** Whether each fact's label stands beside its value (wide) or over it (narrow). */
async function expectFactsBeside(canvas: HTMLElement, beside: boolean) {
  const terms = within(canvas).getAllByRole("term");
  const values = within(canvas).getAllByRole("definition");
  for (const [at, term] of terms.entries()) {
    const label = term.getBoundingClientRect();
    const value = values[at].getBoundingClientRect();
    if (beside) await expect(value.left).toBeGreaterThan(label.right);
    else await expect(value.top).toBeGreaterThanOrEqual(label.bottom - 1);
  }
}

/**
 * **A skill's Overview facts** (#3673): each label beside its value in one column of labels; a
 * value holds a badge, a path and one fix, which the keyboard reaches and presses.
 */
export const OverviewFacts: Story = {
  render: () => <Overview inlineSize="40rem" />,
  play: async ({ canvas, canvasElement, userEvent }) => {
    await expect(canvas.getAllByRole("term").map((one) => one.textContent)).toEqual([
      "Who gets it",
      "Where it lives",
      "Owner",
      "Grade",
      "Runs",
    ]);
    await expectFactsBeside(canvasElement, true);
    // One label column: every value starts where the first does.
    const starts = canvas.getAllByRole("definition").map((one) => one.getBoundingClientRect().left);
    for (const start of starts) await expect(Math.abs(start - starts[0])).toBeLessThan(1);
    await userEvent.tab();
    const fix = canvas.getByRole("button", { name: "Grant it to steward" });
    await untilFocused(() => fix, "Overview facts: Tab lands on the fix");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Granted"));
  },
};

/** **The same facts where they are narrow** (#3673): each label over its value. */
export const OverviewFactsNarrow: Story = {
  render: () => <Overview inlineSize="18rem" />,
  play: async ({ canvasElement }) => {
    await expectFactsBeside(canvasElement, false);
  },
};

/** **The grade plan's four totals** (#3673). */
function GradePlan({ inlineSize }: { inlineSize: string }) {
  return (
    <div style={{ inlineSize, maxInlineSize: "100%" }}>
      <Totals aria-label="What the grade may spend">
        <Tile label="Chats" value="36" line="12 tests × 3 × 1 arm" />
        <Tile label="At most" value="$3.00" line="then it stops" />
        <Tile label="Estimate" value="$2.88" line="$0.08 a chat" />
        <Tile label="Runs on" value="Claude Code" line="sonnet" />
      </Totals>
    </div>
  );
}

/** The tiles' boxes, in their order. */
const tilesOf = (canvas: HTMLElement) =>
  [...canvas.querySelectorAll('[data-scope="card"][data-part="tile"]')].map((one) =>
    one.getBoundingClientRect(),
  );

/**
 * **The grade plan's totals, wide** (#3673): four tiles share the row, each as wide as the next,
 * their values a size larger in figures of one width.
 */
export const PlanTotals: Story = {
  render: () => <GradePlan inlineSize="56rem" />,
  play: async ({ canvas, canvasElement }) => {
    await expect(canvas.getByRole("group", { name: "What the grade may spend" })).toBeVisible();
    const tiles = tilesOf(canvasElement);
    await expect(tiles).toHaveLength(4);
    for (const tile of tiles) {
      await expect(Math.abs(tile.top - tiles[0].top)).toBeLessThan(1);
      await expect(Math.abs(tile.width - tiles[0].width)).toBeLessThan(1);
    }
  },
};

/** **The same totals where they are narrow** (#3673): the tiles wrap to more rows, never sideways. */
export const PlanTotalsNarrow: Story = {
  render: () => <GradePlan inlineSize="24rem" />,
  play: async ({ canvasElement }) => {
    const tiles = tilesOf(canvasElement);
    await expect(tiles[3].top).toBeGreaterThan(tiles[0].bottom - 1);
    const row = canvasElement.querySelector('[data-part="totals"]') as HTMLElement;
    await expect(row.scrollWidth).toBeLessThanOrEqual(row.clientWidth);
  },
};

/** **A running grade's counts** (#3673): a chat further on changes them, live. */
function RunningCounts() {
  const [done, setDone] = useState(3);
  return (
    <div style={{ inlineSize: "40rem", maxInlineSize: "100%" }}>
      <Totals live aria-label="Grading deploy">
        <Tile label="Chats" value={`${done} of 36`} line="3 at a time" />
        <Tile label="Spent" value={`$${(done * 0.08).toFixed(2)}`} line="of at most $3.00" />
        <Tile label="Tokens" value={`${done * 9}k`} line="of 400k" />
      </Totals>
      <p style={{ marginBlockStart: "var(--space-5)" }}>
        <Button onPress={() => setDone((now) => now + 7)}>Run seven chats</Button>
      </p>
    </div>
  );
}

/**
 * **Running's counts change without moving a tile** (#3673): seven chats on, each tile keeps
 * its box, and the change is said once, politely.
 */
export const RunningTotals: Story = {
  render: () => <RunningCounts />,
  play: async ({ canvas, canvasElement, userEvent }) => {
    const before = tilesOf(canvasElement);
    await userEvent.tab();
    const run = canvas.getByRole("button", { name: "Run seven chats" });
    await untilFocused(() => run, "Running totals: Tab lands on Run seven chats");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByText("10 of 36")).toBeVisible());
    const after = tilesOf(canvasElement);
    for (const [at, tile] of after.entries()) {
      await expect(Math.abs(tile.width - before[at].width)).toBeLessThan(1);
      await expect(Math.abs(tile.height - before[at].height)).toBeLessThan(1);
      await expect(Math.abs(tile.left - before[at].left)).toBeLessThan(1);
    }
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent(
        "Chats 10 of 36, Spent $0.80, Tokens 90k",
      ),
    );
  },
};

/** **The welcome's doors** (#3655): three cards pressed whole, each a mark, a title and a line. */
function Doors() {
  const [said, setSaid] = useState("Nothing pressed yet");
  const doors = [
    {
      title: "Create a project",
      line: "On GitHub, on GitLab or only on this machine, with a template for its stack.",
      Mark: FolderPlus,
    },
    {
      title: "Get a project from your forge",
      line: "Clone your team's project from GitHub or GitLab, from your repos or by its URL.",
      Mark: CloudDownload,
    },
    {
      title: "Open a folder on this machine…",
      line: "Open a project you already have.",
      Mark: FolderOpen,
    },
  ];
  return (
    <div style={{ inlineSize: "44rem", maxInlineSize: "100%" }}>
      <ul
        aria-label="Ways to start"
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(12.5rem, 1fr))",
          gap: "var(--space-5)",
          margin: 0,
          padding: 0,
          listStyle: "none",
        }}
      >
        {doors.map(({ title, line, Mark }) => (
          <li key={title} style={{ display: "flex" }}>
            <CardButton
              title={title}
              line={line}
              mark={<Mark aria-hidden="true" />}
              onPress={() => setSaid(`Pressed ${title}`)}
            />
          </li>
        ))}
      </ul>
      <p role="status" style={{ marginBlockStart: "var(--space-5)" }}>
        {said}
      </p>
    </div>
  );
}

/**
 * **The doors** (#3655): Tab walks them in order, each in sight, named by its title and
 * described by its line; Enter presses one and Space another.
 */
export const PressableDoors: Story = {
  render: () => <Doors />,
  play: async ({ canvas, userEvent }) => {
    const names = [
      "Create a project",
      "Get a project from your forge",
      "Open a folder on this machine…",
    ];
    for (const name of names) {
      await userEvent.tab();
      const door = canvas.getByRole("button", { name });
      await untilFocused(() => door, `Doors: Tab lands on ${name}`);
      await expect(door).toHaveAccessibleDescription(/\w/);
      expectFocusInSight();
    }
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent(
        "Pressed Open a folder on this machine…",
      ),
    );
    await userEvent.tab({ shift: true });
    await userEvent.keyboard(" ");
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent("Pressed Get a project from your forge"),
    );
  },
};

/** The launcher's rows at `inlineSize`, as tall as the tallest (`.welcome-doors`, #4602). */
const launcherRowsAt = (inlineSize: string) => (
  <ul
    style={{
      listStyle: "none",
      margin: 0,
      padding: 0,
      inlineSize,
      display: "grid",
      gridAutoRows: "1fr",
      gap: "var(--space-1)",
    }}
  >
    {[
      {
        title: "Create a project",
        line: "On GitHub, on GitLab or only on this Mac.",
        Mark: FolderPlus,
      },
      {
        title: "Get a project from your forge",
        line: "Clone your team's from GitHub or GitLab.",
        Mark: CloudDownload,
      },
      {
        title: "Open a folder on this machine…",
        line: "Open a project you already have.",
        Mark: FolderOpen,
      },
    ].map(({ title, line, Mark }, at) => (
      <li key={title} style={{ display: "grid" }}>
        <CardButton
          variant="row"
          title={title}
          line={line}
          current={at === 1}
          mark={<Mark aria-hidden="true" />}
          onPress={() => undefined}
        />
      </li>
    ))}
  </ul>
);

/** **The launcher's ways in**: rows of the same family, the one whose page is showing marked. */
export const LauncherRows: Story = { render: () => launcherRowsAt("18rem") };

/** **At the narrowest window**: a line that wraps leaves every row the same height (#4602). */
export const LauncherRowsNarrow: Story = {
  render: () => launcherRowsAt("13rem"),
  play: async ({ canvasElement }) => {
    const rows = canvasElement.querySelectorAll('[data-scope="card-button"][data-part="root"]');
    const heights = [...rows].map((row) => row.getBoundingClientRect().height);
    await expect(new Set(heights).size).toBe(1);
  },
};

/** **Repos to get** (#3655): a pick a line of a list, one unavailable while another is on its way. */
function Picks() {
  const [got, setGot] = useState<string | undefined>(undefined);
  const repos = [
    { name: "purlis", owner: "purlis", project: true },
    { name: "website", owner: "purlis", project: false },
  ];
  // The list scrolls as the Get page's does (`.repo-picks`), with no padding of its own, so the
  // picture shows a pick's ring whole where a scroller would clip one drawn outside it.
  return (
    <OnAPanel>
      <ul
        aria-label="Your repos"
        style={{
          maxBlockSize: "14rem",
          overflowY: "auto",
          margin: 0,
          padding: 0,
          listStyle: "none",
        }}
      >
        {repos.map(({ name, owner, project }) => (
          <li key={name}>
            <CardButton
              variant="pick"
              title={`${owner}/${name}`}
              line="github.com · aaron"
              isDisabled={got !== undefined && got !== name}
              onPress={() => setGot(name)}
            >
              {project && <Badge>purlis project</Badge>}
            </CardButton>
          </li>
        ))}
      </ul>
      <p role="status" style={{ margin: 0 }}>
        {got === undefined ? "Nothing to get yet" : `Getting ${got}`}
      </p>
    </OnAPanel>
  );
}

/**
 * **The picks** (#3655): Tab reaches each repo by the words it shows, described by its forge;
 * Enter gets one, and the other is unavailable while it comes, out of the tab sequence. They
 * stand in a list that scrolls, as on the Get page, and the focused one's ring is inside it.
 */
export const PressablePicks: Story = {
  render: () => <Picks />,
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    const first = canvas.getByRole("button", { name: "purlis/purlis purlis project" });
    await untilFocused(() => first, "Picks: Tab lands on the first repo");
    await expect(first).toHaveAccessibleDescription("github.com · aaron");
    expectFocusInSight();
    await userEvent.tab();
    const second = canvas.getByRole("button", {
      name: "purlis/website",
    });
    await untilFocused(() => second, "Picks: Tab lands on the second repo");
    // Its ring is drawn inside the pick, so the list that scrolls never clips it.
    await expect(parseFloat(getComputedStyle(second, "::after").outlineOffset)).toBeLessThan(0);
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Getting website"));
    await expect(first).toBeDisabled();
  },
};
