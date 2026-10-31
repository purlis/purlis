// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The table family** (#2911): a table of hosts that chooses, sorts and opens a row's menu under
 * its bar (the settings prototype's hosts table, which Settings' record lists take in #2919), a
 * table of data whose cells hold a button, a table with no rows, a table of controls, wide
 * and stacked into a form, and the Skill studio's skills, whose short columns keep one line beside
 * a long one (#3671), wide and in a narrow split. Each is walked by the keyboard, held to axe and compared with its
 * baseline in Chromium and WebKit, in every theme's cell (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo, useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { Save } from "lucide-react";
import { expectFocusInSight, inAWindow, untilFocused } from "../../stories.testkit";
import { Button, IconButton } from "../button/Button";
import { TextField } from "../field/Field";
import { ListFilter, noMatchSaid, useListFilter } from "../list/ListFilter";
import { Menu, MenuItem, MenuSeparator, MenuWords } from "../menu/Menu";
import {
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
  type Selection,
  type SortDescriptor,
} from "./Table";
import { TableBar } from "./TableBar";
import {
  ControlBody,
  ControlCell,
  ControlColumn,
  ControlHead,
  ControlRow,
  ControlRowHeading,
  ControlTable,
} from "./ControlTable";

type Host = { host: string; port: number; waiting: boolean; added: number; by: string };

/** Nine hosts a chat reached, two of them waiting for a Confirm (a table over seven is filtered). */
const HOSTS: readonly Host[] = [
  { host: "api.github.com", port: 443, waiting: true, added: 1, by: "steward 3" },
  { host: "registry.npmjs.org", port: 443, waiting: false, added: 4, by: "web-dev 1" },
  { host: "pypi.org", port: 443, waiting: false, added: 9, by: "data-eng 2" },
  { host: "objects.githubusercontent.com", port: 443, waiting: true, added: 2, by: "steward 3" },
  { host: "crates.io", port: 443, waiting: false, added: 12, by: "infra 1" },
  { host: "index.docker.io", port: 443, waiting: false, added: 6, by: "devops 4" },
  { host: "proxy.golang.org", port: 443, waiting: false, added: 15, by: "platform 2" },
  { host: "dl.google.com", port: 443, waiting: false, added: 21, by: "mobile 1" },
  { host: "localhost", port: 5432, waiting: false, added: 30, by: "you" },
];

const HOST = { one: "host", many: "hosts" };

/** **The hosts table**: waiting rows first, a filter, Confirm all, bulk actions and row menus. */
function Hosts() {
  const { query, setQuery, shown } = useListFilter(HOSTS, (one) => one.host);
  const [chosen, setChosen] = useState<Selection>(new Set());
  const [sort, setSort] = useState<SortDescriptor>({ column: "added", direction: "ascending" });
  const [said, setSaid] = useState("Nothing done yet");
  const rows = useMemo(() => {
    const sorted = [...shown].sort((a, b) =>
      sort.column === "host" ? a.host.localeCompare(b.host) : a.added - b.added,
    );
    if (sort.direction === "descending") sorted.reverse();
    // Waiting for a Confirm first, whatever the order (R65).
    return [...sorted.filter((one) => one.waiting), ...sorted.filter((one) => !one.waiting)];
  }, [shown, sort]);
  const count = chosen === "all" ? rows.length : chosen.size;
  const waiting = HOSTS.filter((one) => one.waiting).length;
  return (
    <div style={{ inlineSize: "44rem", maxInlineSize: "100%" }}>
      <TableBar
        label="Hosts"
        filter={<ListFilter noun={HOST} total={HOSTS.length} query={query} onQuery={setQuery} />}
        chosen={count}
        bulk={
          <>
            <Button onPress={() => setSaid(`Confirmed ${count}`)}>Confirm</Button>
            <Button variant="ghost" onPress={() => setSaid(`Removed ${count}`)}>
              Remove
            </Button>
          </>
        }
        actions={
          <>
            <Button variant="primary" onPress={() => setSaid("Confirmed every waiting host")}>
              {`Confirm all ${waiting} waiting`}
            </Button>
            <Button onPress={() => setSaid("Adding a host")}>Add host</Button>
          </>
        }
      />
      <Table
        aria-label="Hosts"
        selectionMode="multiple"
        selectedKeys={chosen}
        onSelectionChange={setChosen}
        sortDescriptor={sort}
        onSortChange={setSort}
        rowMenus
      >
        <TableHeader>
          <TableColumn id="host" isRowHeader allowsSorting>
            Host
          </TableColumn>
          <TableColumn id="status">Status</TableColumn>
          <TableColumn id="added" allowsSorting>
            Added
          </TableColumn>
        </TableHeader>
        <TableBody empty={<p style={{ margin: 0 }}>{noMatchSaid(HOST, query)}</p>}>
          {rows.map((one) => (
            <TableRow
              key={one.host}
              id={one.host}
              textValue={one.host}
              menu={() => (
                <Menu
                  aria-label={one.host}
                  onAction={(key) => setSaid(`${String(key)} ${one.host}`)}
                >
                  {one.waiting && (
                    <MenuItem id="confirm" textValue="Confirm">
                      <MenuWords said="Confirm" />
                    </MenuItem>
                  )}
                  <MenuItem id="copy" textValue="Copy">
                    <MenuWords said="Copy" />
                  </MenuItem>
                  <MenuSeparator />
                  <MenuItem id="remove" textValue="Remove" variant="danger">
                    <MenuWords said="Remove" />
                  </MenuItem>
                </Menu>
              )}
            >
              <TableCell>{`${one.host}:${one.port}`}</TableCell>
              <TableCell>{one.waiting ? "Waiting for you" : "Confirmed"}</TableCell>
              <TableCell>{`${one.added} days ago · ${one.by}`}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <p role="status" style={{ marginBlockStart: "var(--space-4)" }}>
        {said}
      </p>
    </div>
  );
}

const meta = {
  component: Hosts,
} satisfies Meta<typeof Hosts>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **A table of data that chooses**: Tab reaches the filter, then the table as one stop on its
 * first row; ↓ and ↑ move a row and stop at the ends, → goes into a row's cells and to its ⋯;
 * Space chooses a row and the bar says how many and offers what to do to them; a column's name
 * sorts; Shift+F10 opens a row's menu on its first row; the filter narrows the table to its
 * empty line. The picture is the filter, its bar, and a chosen row.
 */
export const OfHosts: Story = {
  play: async ({ canvas, userEvent }) => {
    const filter = canvas.getByRole("searchbox", { name: "Filter hosts" });
    await expect(filter).toHaveAttribute("placeholder", "Filter 9 hosts");
    await userEvent.tab();
    await untilFocused(() => filter, "Of hosts: Tab lands on the filter");
    // The bar's two offers, then the table.
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.tab();
    const grid = canvas.getByRole("grid", { name: "Hosts" });
    const rows = () => within(grid).getAllByRole("row").slice(1);
    await untilFocused(() => rows()[0], "Of hosts: Tab lands on the table's first row");
    // Waiting rows first.
    await expect(rows()[0]).toHaveAccessibleName(/api\.github\.com/);
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows()[1], "Of hosts: Down moves to the next row");
    await userEvent.keyboard("{Control>}{End}{/Control}");
    await untilFocused(() => rows()[8], "Of hosts: Ctrl+End moves to the last row");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => rows()[8], "Of hosts: Down stops at the last row");
    await userEvent.keyboard("{Control>}{Home}{/Control}");
    await untilFocused(() => rows()[0], "Of hosts: Ctrl+Home moves to the first row");
    // A page down and back up: the table is shorter than a page, so its last row and its first.
    await userEvent.keyboard("{PageDown}");
    await untilFocused(() => rows()[8], "Of hosts: Page Down moves a page, to the last row");
    // A page back reaches the grid's own first row, the columns' names, whose first cell is the
    // check that chooses every row; Down comes back to the first row of hosts.
    await userEvent.keyboard("{PageUp}");
    await untilFocused(
      () => within(grid).getByRole("checkbox", { name: "Select All" }),
      "Of hosts: Page Up moves a page back, to the columns' row",
    );
    // Down goes to the cell under it, the first row's check, and Left to the row itself.
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(rows()[0]).getByRole("checkbox"),
      "Of hosts: Down comes back to the first row's check",
    );
    await userEvent.keyboard("{ArrowLeft}");
    await untilFocused(() => rows()[0], "Of hosts: Left comes back to the first row");
    // → walks into the row: its check, its cells, its ⋯.
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(rows()[0]).getByRole("checkbox"),
      "Of hosts: Right reaches the row's check",
    );
    // The check's target is a small control's, 24 px, with its 16 px box inside (WCAG 2.5.8).
    const check = within(rows()[0]).getByRole("checkbox").closest('[data-part="select"]');
    const target = (check as HTMLElement).getBoundingClientRect();
    await expect(target.width).toBeGreaterThanOrEqual(24);
    await expect(target.height).toBeGreaterThanOrEqual(24);
    // The choice family's box (#3362) takes a press across that whole 24 px square (#4197).
    const box = within(rows()[0]).getByRole("checkbox").closest("label") as HTMLElement;
    await expect(box).toHaveAttribute("data-scope", "checkbox");
    const [x, y] = [target.left + target.width / 2, target.top + target.height / 2];
    const drawn = (
      box.querySelector('[data-scope="checkbox"][data-part="box"]') as HTMLElement
    ).getBoundingClientRect();
    await expect(drawn.left + drawn.width / 2).toBeCloseTo(x, 0);
    await expect(drawn.top + drawn.height / 2).toBeCloseTo(y, 0);
    for (const [dx, dy] of [
      [-11.5, -11.5],
      [11.5, -11.5],
      [-11.5, 11.5],
      [11.5, 11.5],
    ])
      await expect(document.elementFromPoint(x + dx, y + dy)?.closest("label")).toBe(box);
    await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}{ArrowRight}");
    await untilFocused(
      () => within(rows()[0]).getByRole("button", { name: "More for api.github.com" }),
      "Of hosts: Right reaches the row's ⋯ at its end",
    );
    // Left back to the row's start, and Left again to the row itself.
    await userEvent.keyboard("{ArrowLeft}{ArrowLeft}{ArrowLeft}{ArrowLeft}{ArrowLeft}");
    await untilFocused(() => rows()[0], "Of hosts: Left comes back to the row");
    // Space chooses; the bar says so, and the filter has not moved.
    await userEvent.keyboard(" ");
    await expect(rows()[0]).toHaveAttribute("aria-selected", "true");
    await waitFor(() => expect(canvas.getByText("1 selected")).toBeVisible());
    await expect(canvas.getByRole("button", { name: "Remove" })).toBeVisible();
    // A row's menu from the keys, on its first row.
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "api.github.com" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: "Confirm" }),
      "The row's menu: the keyboard lands on Confirm",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    // Sorting by a column's name.
    const host = within(grid).getByRole("columnheader", { name: "Host" });
    await userEvent.click(host);
    await expect(host).toHaveAttribute("aria-sort", "ascending");
    // The filter narrows the table, to its empty line where nothing matches.
    await userEvent.click(filter);
    await userEvent.type(filter, "zz");
    await waitFor(() => expect(canvas.getByText("No host is called “zz”.")).toBeVisible());
    await userEvent.clear(filter);
    await waitFor(() => expect(rows()).toHaveLength(9));
    await expect(rows()[0]).toHaveAttribute("aria-selected", "true");
  },
};

/** A workspace's repos, as Saving draws them: a button in a cell, which the arrows reach. */
function Repos() {
  const [saved, setSaved] = useState("Nothing saved");
  const repos = [
    { name: "api", branch: "feature/x", stage: "3 changed", goes: "pushed to origin" },
    { name: "web", branch: "main", stage: "Saved", goes: "pushed to origin" },
    { name: "docs", branch: "main", stage: "Off", goes: "nowhere: purlis does not save it" },
  ];
  return (
    <div style={{ inlineSize: "44rem", maxInlineSize: "100%" }}>
      <Table aria-label="Repos in alpha">
        <TableHeader>
          <TableColumn isRowHeader>Repo</TableColumn>
          <TableColumn>Branch</TableColumn>
          <TableColumn>Stage</TableColumn>
          <TableColumn>Save goes to</TableColumn>
          <TableColumn hideName>Save</TableColumn>
        </TableHeader>
        <TableBody>
          {repos.map((repo) => (
            <TableRow key={repo.name} id={repo.name} textValue={repo.name}>
              <TableCell>{repo.name}</TableCell>
              <TableCell>{repo.branch}</TableCell>
              <TableCell>{repo.stage}</TableCell>
              <TableCell>{repo.goes}</TableCell>
              <TableCell>
                <IconButton
                  size="sm"
                  aria-label={`Save ${repo.name}`}
                  isDisabled={repo.stage === "Off"}
                  onPress={() => setSaved(`Saved ${repo.name}`)}
                  icon={<Save aria-hidden="true" />}
                />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <p role="status">{saved}</p>
    </div>
  );
}

/**
 * **A table of data whose cells hold a button**: the arrows walk the cells into the button, Enter
 * presses it, and Tab leaves the table. The picture is the keyboard on a cell's button.
 */
export const WithAButtonInACell: Story = {
  render: () => <Repos />,
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    const grid = canvas.getByRole("grid", { name: "Repos in alpha" });
    const first = within(grid).getAllByRole("row")[1];
    await untilFocused(() => first, "With a button in a cell: Tab lands on the first row");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(first).getByRole("rowheader", { name: "api" }),
      "With a button in a cell: Right moves to the row's first cell",
    );
    await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}{ArrowRight}");
    const save = within(first).getByRole("button", { name: "Save api" });
    await untilFocused(() => save, "With a button in a cell: Right reaches the cell's button");
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Saved api");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(grid).getByRole("button", { name: "Save web" }),
      "With a button in a cell: Down moves to the button below",
    );
    expectFocusInSight();
  },
};

/** **A table with no rows** says so across it, in the quiet tier; Tab still reaches it. */
export const Empty: Story = {
  render: () => (
    <div style={{ inlineSize: "36rem", maxInlineSize: "100%" }}>
      <Table aria-label="Runs of release-notes">
        <TableHeader>
          <TableColumn isRowHeader>Started</TableColumn>
          <TableColumn>Persona</TableColumn>
          <TableColumn align="end">Tool calls</TableColumn>
        </TableHeader>
        <TableBody empty={<p style={{ margin: 0 }}>No run of release-notes yet.</p>}>
          {[]}
        </TableBody>
      </Table>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByText("No run of release-notes yet.")).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("grid", { name: "Runs of release-notes" }),
      "Empty: Tab lands on the table",
    );
  },
};

/** The dispatch limits, as Settings draws them: a box per limit, and the row's button. */
function Limits({ inlineSize }: { inlineSize: string }) {
  const [values, setValues] = useState<Record<string, string>>({
    "project:tasks": "3",
    "project:depth": "2",
    "devops:tasks": "1",
    "devops:depth": "",
  });
  const rows = [
    { id: "project", heading: "Project", whose: "the project" },
    { id: "devops", heading: "devops", whose: "the persona devops" },
  ];
  return (
    <div style={{ inlineSize, maxInlineSize: "100%" }}>
      <ControlTable aria-label="Dispatch limits" stacks>
        <ControlHead>
          <ControlColumn>Level</ControlColumn>
          <ControlColumn whole="How many tasks may run at once" about="A task waits beyond it.">
            Tasks
          </ControlColumn>
          <ControlColumn whole="How deep a task may dispatch again">Depth</ControlColumn>
          <ControlColumn actions hideName whole="Actions" />
        </ControlHead>
        <ControlBody>
          {rows.map((row) => (
            <ControlRow key={row.id}>
              <ControlRowHeading>{row.heading}</ControlRowHeading>
              {(["tasks", "depth"] as const).map((limit) => (
                <ControlCell
                  key={limit}
                  heading={
                    limit === "tasks"
                      ? "How many tasks may run at once"
                      : "How deep a task may dispatch again"
                  }
                >
                  <TextField
                    aria-label={`${limit === "tasks" ? "Tasks" : "Depth"} for ${row.whose}`}
                    value={values[`${row.id}:${limit}`] ?? ""}
                    onChange={(to) => setValues({ ...values, [`${row.id}:${limit}`]: to })}
                  />
                </ControlCell>
              ))}
              <ControlCell actions>
                {row.id !== "project" && <Button>Remove override</Button>}
              </ControlCell>
            </ControlRow>
          ))}
        </ControlBody>
      </ControlTable>
    </div>
  );
}

/**
 * **A table of controls**: each box is its own Tab stop and keeps ← and → for its caret, which
 * a grid would take. The picture is the keyboard in a box.
 */
export const OfControls: Story = {
  render: () => <Limits inlineSize="40rem" />,
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByRole("table", { name: "Dispatch limits" })).toBeVisible();
    // A shortened column name is a Tab stop of its own, so its whole words in its tooltip are the
    // keyboard's too (WCAG 1.4.13): Tasks, then Depth, then the boxes.
    await userEvent.tab();
    await untilFocused(() => canvas.getByText("Tasks"), "Of controls: Tab lands on Tasks' name");
    await waitFor(() =>
      expect(screen.getByRole("tooltip")).toHaveTextContent("How many tasks may run at once"),
    );
    await userEvent.tab();
    await untilFocused(() => canvas.getByText("Depth"), "Of controls: Tab moves to Depth's name");
    await waitFor(() =>
      expect(screen.getByRole("tooltip")).toHaveTextContent("How deep a task may dispatch again"),
    );
    await userEvent.tab();
    const tasks = canvas.getByRole("textbox", { name: "Tasks for the project" });
    await untilFocused(() => tasks, "Of controls: Tab then lands on the first box");
    await userEvent.keyboard("{End}{ArrowLeft}");
    await untilFocused(() => tasks, "Of controls: Left stays in the box, for its caret");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("textbox", { name: "Depth for the project" }),
      "Of controls: Tab moves to the next box",
    );
    await expect(
      canvas.getByRole("columnheader", { name: "How many tasks may run at once" }),
    ).toHaveTextContent("Tasks");
  },
};

/**
 * **A table of controls where its frame is narrow**: stacked into a form, each box under its
 * column's whole words, and the row's button on a line of its own. Nothing is cut.
 */
export const OfControlsStacked: Story = {
  render: () => <Limits inlineSize="20rem" />,
  play: async ({ canvas, userEvent }) => {
    const table = canvas.getByRole("table", { name: "Dispatch limits" });
    await waitFor(() =>
      expect(getComputedStyle(table.querySelector("thead") as Element).display).toBe("none"),
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("textbox", { name: "Tasks for the project" }),
      "Of controls stacked: Tab lands on the first box",
    );
    expectFocusInSight();
  },
};

/** The Skill studio's skills (#3671): the operator's screenshot of 2026-10-28, its columns wrapped. */
const SKILLS = [
  {
    name: "deploy",
    does: "Ships a branch to the staging cluster, waits for its checks to pass and rolls it back where they fail, says each step it took in the chat as it goes, asks before it touches production or a branch it did not start, and keeps a note of what it shipped in the workspace's memory.",
    chat: "Every chat in smart-ide",
    works: "Claude Code · Codex",
    level: "Workspace",
  },
  {
    name: "release-notes",
    does: "Writes the release notes from the merged pull requests.",
    chat: "When asked",
    works: "Claude Code",
    level: "Project",
  },
  {
    name: "review",
    does: "Reviews a pull request against the repo's own standards and the issue it closes, and says what it would change before it says what it likes.",
    chat: "Every chat of the persona reviewer",
    works: "Claude Code · Codex · Gemini",
    level: "Persona",
  },
];

/** **The skills as a table of data** whose view sized its columns (#3671). */
function Skills({ inlineSize }: { inlineSize: string }) {
  return (
    <div style={{ inlineSize, maxInlineSize: "100%" }}>
      <Table aria-label="Skills">
        <TableHeader>
          <TableColumn isRowHeader size="fit">
            Skill
          </TableColumn>
          <TableColumn size="fit">Level</TableColumn>
          <TableColumn size="fill" clamp={2} minWidth="12rem">
            What it does
          </TableColumn>
          <TableColumn cut maxWidth="12rem">
            In a chat
          </TableColumn>
          <TableColumn size="fit">Works on</TableColumn>
        </TableHeader>
        <TableBody>
          {SKILLS.map((skill) => (
            <TableRow key={skill.name} id={skill.name} textValue={skill.name}>
              <TableCell>{skill.name}</TableCell>
              <TableCell>{skill.level}</TableCell>
              <TableCell>{skill.does}</TableCell>
              <TableCell>{skill.chat}</TableCell>
              <TableCell>{skill.works}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

/** How many lines `words` are drawn on. */
function linesOf(words: Element): number {
  const tall = words.getBoundingClientRect().height;
  const line = parseFloat(getComputedStyle(words).lineHeight);
  return Math.round(tall / line);
}

/** The words of each cell of the skills' rows, by column. */
function skillWords(canvas: HTMLElement) {
  const grid = within(canvas).getByRole("grid", { name: "Skills" });
  const rows = within(grid).getAllByRole("row").slice(1);
  const of = (column: number) =>
    rows.map((row) => {
      const cells = row.querySelectorAll('[role="rowheader"], [role="gridcell"]');
      return cells[column].querySelector('[data-part="words"]') as HTMLElement;
    });
  return { grid, rows, of };
}

/**
 * The skills hold their columns: Skill, Level, In a chat and Works on keep one line each, the
 * description is two lines at most, and a cut cell's whole words are its tooltip as the
 * keyboard reaches it, which Esc takes down.
 */
async function playSkills(
  canvas: HTMLElement,
  userEvent: { tab: () => Promise<void>; keyboard: (keys: string) => Promise<void> },
) {
  const { rows, of } = skillWords(canvas);
  for (const column of [0, 1, 3, 4])
    for (const words of of(column))
      await expect(linesOf(words), `column ${column}: one line`).toBe(1);
  for (const words of of(2)) await expect(linesOf(words)).toBeLessThanOrEqual(2);
  // The first skill's description is longer than two lines: it is cut, and says so.
  const deploy = of(2)[0];
  await expect(deploy.scrollHeight).toBeGreaterThan(deploy.clientHeight);
  await userEvent.tab();
  await untilFocused(() => rows[0], "Skills: Tab lands on the first row");
  await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
  const cell = deploy.closest('[role="gridcell"]');
  await untilFocused(() => cell, "Skills: Right reaches the first description");
  await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent(SKILLS[0].does));
  expectFocusInSight();
  await userEvent.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
  await untilFocused(() => cell, "Skills: Esc takes the tooltip down and the keyboard stays");
}

/**
 * **A table whose short columns keep one line** (#3671): the operator's case, at a 1280 px
 * window. The picture is the description cut at two lines beside its one-line columns.
 */
export const OfSkills: Story = {
  render: () => <Skills inlineSize="100%" />,
  play: async ({ canvasElement, userEvent }) => {
    await inAWindow(1280, 720);
    await playSkills(canvasElement, userEvent);
  },
};

/**
 * **The same table in a narrow split** (#3671): its short columns still keep one line, the
 * description keeps its least, and the frame scrolls the table sideways.
 */
export const OfSkillsInANarrowSplit: Story = {
  render: () => <Skills inlineSize="24rem" />,
  play: async ({ canvasElement, userEvent }) => {
    const { grid } = skillWords(canvasElement);
    const frame = grid.closest('[data-part="frame"]') as HTMLElement;
    await expect(frame.scrollWidth).toBeGreaterThan(frame.clientWidth);
    await playSkills(canvasElement, userEvent);
  },
};

/** **The studio's own skills, a table of controls**, with the same column choices (#3671). */
function StudioSkills() {
  return (
    <div style={{ inlineSize: "40rem", maxInlineSize: "100%" }}>
      <ControlTable aria-label="Studio skills">
        <ControlHead>
          <ControlColumn size="fit">Skill</ControlColumn>
          <ControlColumn size="fill" clamp={2} minWidth="10rem">
            What it does
          </ControlColumn>
          <ControlColumn cut maxWidth="6rem">
            In a chat
          </ControlColumn>
          <ControlColumn actions hideName whole="Actions" />
        </ControlHead>
        <ControlBody>
          {SKILLS.map((skill) => (
            <ControlRow key={skill.name}>
              <ControlRowHeading>{skill.name}</ControlRowHeading>
              <ControlCell>{skill.does}</ControlCell>
              <ControlCell>{skill.chat}</ControlCell>
              <ControlCell actions>
                <Button size="sm">{`Change ${skill.name}`}</Button>
              </ControlCell>
            </ControlRow>
          ))}
        </ControlBody>
      </ControlTable>
    </div>
  );
}

/**
 * **A table of controls whose columns chose** (#3671): words cut short are a Tab stop of their
 * own, whose tooltip says them whole; the cells' buttons follow.
 */
export const OfControlsCut: Story = {
  render: () => <StudioSkills />,
  play: async ({ canvas, userEvent }) => {
    const table = canvas.getByRole("table", { name: "Studio skills" });
    const words = (text: string) =>
      within(table).getByText(text, { selector: '[data-part="words"]' });
    await userEvent.tab();
    await untilFocused(() => words(SKILLS[0].does), "Of controls cut: Tab lands on cut words");
    await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent(SKILLS[0].does));
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
    await userEvent.tab();
    await untilFocused(() => words(SKILLS[0].chat), "Of controls cut: Tab moves to the next");
    expectFocusInSight();
  },
};
