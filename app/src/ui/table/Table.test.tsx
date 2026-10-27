// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The table family's wrappers** (#2911 part 3): what a person walks, chooses, sorts and opens
 * in a table of data, the keys a table of controls leaves to its controls, and the attributes a
 * theme's parts are keyed on (ADR 0103). The look itself is held by the stories, in a real
 * browser (`Table.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useMemo, useState } from "react";
import { Button } from "../button/Button";
import { TextField } from "../field/Field";
import { Menu, MenuItem, MenuWords } from "../menu/Menu";
import { drawnWith } from "../../cascade.testkit";
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

afterEach(cleanup);

const HOSTS = [
  { host: "api.github.com", status: "Waiting for you", added: 3 },
  { host: "registry.npmjs.org", status: "Confirmed", added: 1 },
  { host: "pypi.org", status: "Confirmed", added: 2 },
];

/** A table of hosts, as Settings' granted hosts will draw one (#2919). */
function Hosts({ empty = false }: { empty?: boolean }) {
  const [chosen, setChosen] = useState<Selection>(new Set());
  const [sort, setSort] = useState<SortDescriptor>({ column: "host", direction: "ascending" });
  const [said, setSaid] = useState("Nothing done");
  const rows = useMemo(() => {
    const sorted = [...(empty ? [] : HOSTS)].sort((a, b) =>
      sort.column === "added" ? a.added - b.added : a.host.localeCompare(b.host),
    );
    return sort.direction === "descending" ? sorted.reverse() : sorted;
  }, [sort, empty]);
  const count = chosen === "all" ? rows.length : chosen.size;
  return (
    <>
      <TableBar
        label="Hosts"
        chosen={count}
        bulk={<Button onPress={() => setSaid(`Removed ${count}`)}>Remove</Button>}
        actions={<Button onPress={() => setSaid("Adding")}>Add host</Button>}
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
          <TableColumn id="added" align="end" allowsSorting>
            Added
          </TableColumn>
        </TableHeader>
        <TableBody empty={<p>No host has been reached yet.</p>}>
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
                  <MenuItem id="confirm" textValue="Confirm">
                    <MenuWords said="Confirm" />
                  </MenuItem>
                  <MenuItem id="remove" textValue="Remove" variant="danger">
                    <MenuWords said="Remove" />
                  </MenuItem>
                </Menu>
              )}
            >
              <TableCell>{one.host}</TableCell>
              <TableCell>{one.status}</TableCell>
              <TableCell align="end">{`${one.added} days ago`}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <p role="status">{said}</p>
    </>
  );
}

const grid = () => screen.getByRole("grid", { name: "Hosts" });
/** A row by the host that names it: its accessible name starts with the host's words. */
const rowOf = (name: string) => screen.getByRole("row", { name: (said) => said.startsWith(name) });

describe("a table of data", () => {
  it("is a grid of its rows, named, each row headed by its host and drawn by its parts", () => {
    render(<Hosts />);
    expect(grid()).toHaveAttribute("data-scope", "table");
    expect(grid()).toHaveAttribute("data-part", "body");
    expect(grid().closest('[data-part="frame"]')).not.toBeNull();
    const heads = within(grid()).getAllByRole("columnheader");
    // The checks' column, the three named, and the menus'.
    expect(heads.map((one) => one.textContent)).toEqual(["", "Host", "Status", "Added", "Actions"]);
    for (const one of heads) expect(one).toHaveAttribute("data-part", "head");
    expect(within(grid()).getByRole("rowheader", { name: "api.github.com" })).toBeInTheDocument();
    const row = rowOf("api.github.com");
    expect(row).toHaveAttribute("data-part", "root");
    for (const cell of within(row).getAllByRole("gridcell"))
      expect(cell).toHaveAttribute("data-part", "cell");
  });

  it("walks its cells by the arrows, one Tab stop, and stops at its edges", async () => {
    render(
      <>
        <Hosts />
        <button type="button">After</button>
      </>,
    );
    await userEvent.tab();
    await userEvent.tab();
    // The bar's Add host first, then the table: its first row.
    expect(within(grid()).getAllByRole("row")[1]).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(rowOf("pypi.org")).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(rowOf("registry.npmjs.org")).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(rowOf("registry.npmjs.org")).toHaveFocus();
    // Right moves into the row's cells: the check first.
    await userEvent.keyboard("{ArrowRight}");
    expect(document.activeElement?.closest('[data-part="cell"]')).not.toBeNull();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
  });

  it("walks a row's cells by → to its ⋯, comes back by ←, and goes to the last row by Ctrl+End", async () => {
    render(<Hosts />);
    const first = within(grid()).getAllByRole("row")[1];
    act(() => first.focus());
    await userEvent.keyboard("{ArrowRight}");
    expect(within(first).getByRole("checkbox")).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}{ArrowRight}");
    expect(within(first).getByRole("button", { name: /^More for / })).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}{ArrowLeft}{ArrowLeft}{ArrowLeft}{ArrowLeft}");
    expect(first).toHaveFocus();
    await userEvent.keyboard("{Control>}{End}{/Control}");
    expect(within(grid()).getAllByRole("row").at(-1)).toHaveFocus();
    await userEvent.keyboard("{Control>}{Home}{/Control}");
    expect(first).toHaveFocus();
  });

  it("chooses rows by Space and by their checks, and the bar says how many and offers what to do", async () => {
    render(<Hosts />);
    expect(screen.getByRole("button", { name: "Add host" })).toBeInTheDocument();
    act(() => rowOf("api.github.com").focus());
    await userEvent.keyboard(" ");
    expect(rowOf("api.github.com")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("1 selected")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add host" })).toBeNull();
    await userEvent.click(within(grid()).getByRole("checkbox", { name: "Select All" }));
    await waitFor(() => expect(screen.getByText("3 selected")).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(screen.getByRole("status")).toHaveTextContent("Removed 3");
  });

  it("sorts by a column whose name is pressed, and says which way", async () => {
    render(<Hosts />);
    const added = within(grid()).getByRole("columnheader", { name: "Added" });
    expect(within(grid()).getByRole("columnheader", { name: "Host" })).toHaveAttribute(
      "aria-sort",
      "ascending",
    );
    await userEvent.click(added);
    expect(added).toHaveAttribute("aria-sort", "ascending");
    expect(
      within(grid())
        .getAllByRole("rowheader")
        .map((one) => one.textContent),
    ).toEqual(["registry.npmjs.org", "pypi.org", "api.github.com"]);
    await userEvent.click(added);
    expect(added).toHaveAttribute("aria-sort", "descending");
  });

  it("puts each row's menu under the ⋯ at its end, and opens it on a right-click and Shift+F10", async () => {
    render(<Hosts />);
    const row = rowOf("pypi.org");
    await userEvent.click(within(row).getByRole("button", { name: "More for pypi.org" }));
    const menu = await screen.findByRole("menu", { name: "pypi.org" });
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Confirm" }));
    expect(screen.getByRole("status")).toHaveTextContent("confirm pypi.org");
    fireEvent.contextMenu(row);
    expect(await screen.findByRole("menu", { name: "pypi.org" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    act(() => row.focus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const keyed = await screen.findByRole("menu", { name: "pypi.org" });
    await waitFor(() =>
      expect(within(keyed).getByRole("menuitem", { name: "Confirm" })).toHaveFocus(),
    );
  });

  it("opens a row's menu on the menu key too, on its first row", async () => {
    render(<Hosts />);
    const row = rowOf("pypi.org");
    act(() => row.focus());
    await userEvent.keyboard("{ContextMenu}");
    const menu = await screen.findByRole("menu", { name: "pypi.org" });
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: "Confirm" })).toHaveFocus(),
    );
  });

  it("chooses every row by Ctrl+A, and the bar says so", async () => {
    render(<Hosts />);
    act(() => rowOf("pypi.org").focus());
    await userEvent.keyboard("{Control>}a{/Control}");
    await waitFor(() => expect(screen.getByText("3 selected")).toBeInTheDocument());
    for (const row of within(grid()).getAllByRole("row").slice(1))
      expect(row).toHaveAttribute("aria-selected", "true");
  });

  // Page Up and Page Down measure the rows, which jsdom does not lay out: the stories' real
  // browsers hold them (`Table.stories.tsx`, OfHosts).
  it("finds a row by typing its first letters", async () => {
    render(<Hosts />);
    const rows = () => within(grid()).getAllByRole("row").slice(1);
    act(() => rows()[0].focus());
    await userEvent.keyboard("r");
    expect(rowOf("registry.npmjs.org")).toHaveFocus();
  });

  it("runs a row on a press while none is chosen, and chooses it once one is", async () => {
    render(<Hosts />);
    // Nothing chosen: a press runs the row, which opens its menu.
    await userEvent.click(within(rowOf("pypi.org")).getByText("Confirmed"));
    expect(await screen.findByRole("menu", { name: "pypi.org" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    // One chosen by its check: a press on another row chooses it too.
    await userEvent.click(within(rowOf("api.github.com")).getByRole("checkbox"));
    await userEvent.click(within(rowOf("pypi.org")).getByText("Confirmed"));
    expect(rowOf("pypi.org")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("2 selected")).toBeInTheDocument();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("opens a row's menu on Enter, on its first row, and gives the keyboard back to the row", async () => {
    render(<Hosts />);
    const row = rowOf("pypi.org");
    act(() => row.focus());
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "pypi.org" });
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: "Confirm" })).toHaveFocus(),
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(row).toHaveFocus());
  });

  it("takes no menu on a row of a table that draws no ⋯ column, and says so", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      render(
        <Table aria-label="Runs">
          <TableHeader>
            <TableColumn isRowHeader>Started</TableColumn>
          </TableHeader>
          <TableBody>
            <TableRow
              id="a"
              textValue="Monday"
              menu={() => <Menu aria-label="Monday">{null}</Menu>}
            >
              <TableCell>Monday</TableCell>
            </TableRow>
          </TableBody>
        </Table>,
      );
      const row = screen.getByRole("row", { name: "Monday" });
      // The browser's own menu stays: nothing here would open in its place.
      expect(fireEvent.contextMenu(row)).toBe(true);
      expect(warn).toHaveBeenCalledWith(expect.stringContaining("needs `rowMenus`"));
    } finally {
      warn.mockRestore();
    }
  });

  it("says what it has where it has no rows", () => {
    render(<Hosts empty />);
    expect(within(grid()).getByText("No host has been reached yet.")).toBeInTheDocument();
  });

  it("draws the row the view is about as chosen, and says so", () => {
    render(
      <Table aria-label="Runs">
        <TableHeader>
          <TableColumn isRowHeader>Started</TableColumn>
        </TableHeader>
        <TableBody>
          <TableRow id="a" textValue="Monday" current>
            <TableCell>Monday</TableCell>
          </TableRow>
          <TableRow id="b" textValue="Tuesday">
            <TableCell>Tuesday</TableCell>
          </TableRow>
        </TableBody>
      </Table>,
    );
    expect(screen.getByRole("row", { name: "Monday" })).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("row", { name: "Tuesday" })).not.toHaveAttribute("aria-current");
  });
});

describe("a table of controls", () => {
  function Limits() {
    const [tasks, setTasks] = useState("3");
    return (
      <ControlTable aria-label="Dispatch limits" stacks>
        <ControlHead>
          <ControlColumn>Level</ControlColumn>
          <ControlColumn whole="How many tasks may run at once">Tasks</ControlColumn>
          <ControlColumn actions hideName whole="Actions" />
        </ControlHead>
        <ControlBody>
          <ControlRow>
            <ControlRowHeading>Project</ControlRowHeading>
            <ControlCell heading="How many tasks may run at once">
              <TextField aria-label="Tasks for the project" value={tasks} onChange={setTasks} />
            </ControlCell>
            <ControlCell actions>
              <Button>Remove override</Button>
            </ControlCell>
          </ControlRow>
        </ControlBody>
      </ControlTable>
    );
  }

  it("is a plain table drawn by the family's parts, its shortened names whole to a reader", () => {
    render(<Limits />);
    const table = screen.getByRole("table", { name: "Dispatch limits" });
    expect(table).toHaveAttribute("data-scope", "table");
    expect(table.closest('[data-part="frame"]')).toHaveAttribute("data-stacks");
    expect(
      screen.getByRole("columnheader", { name: "How many tasks may run at once" }),
    ).toHaveAttribute("data-part", "head");
    expect(screen.getByRole("rowheader", { name: "Project" })).toHaveAttribute("data-part", "cell");
    expect(
      screen.getByRole("textbox", { name: "Tasks for the project" }).closest("td"),
    ).toHaveAttribute("data-heading", "How many tasks may run at once");
  });

  it("leaves ← and → to a field's caret, and each control is its own Tab stop", async () => {
    render(<Limits />);
    const box = screen.getByRole("textbox", { name: "Tasks for the project" });
    await userEvent.click(box);
    await userEvent.keyboard("{End}{ArrowLeft}");
    expect(box).toHaveFocus();
    expect((box as HTMLInputElement).selectionStart).toBe(0);
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Remove override" })).toHaveFocus();
  });
});

describe("a row's look as the cascade draws it", () => {
  it("keeps a chosen row's wash under the pointer, and draws the pointer's ground on one not chosen", () => {
    const row = document.createElement("tr");
    row.setAttribute("data-scope", "table");
    row.setAttribute("data-part", "root");
    expect(drawnWith(row, "background-color")).toBeUndefined();
    expect(drawnWith(row, "background-color", { hover: true })).toBe("var(--surface-hover)");
    row.setAttribute("data-selected", "true");
    expect(drawnWith(row, "background-color", { hover: true })).toBe("var(--list-selected)");
    expect(drawnWith(row, "color", { hover: true })).toBe("var(--text-primary)");
  });
});
