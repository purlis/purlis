// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a view chooses for a column** (#3671): columns that keep one line, sized by the view,
 * a description wrapped to two lines, and the whole words of a cut cell in its tooltip, read as
 * the keyboard reaches it. jsdom draws nothing, so whether words are cut is said by the box's
 * measures, which a test sets; the look itself is the stories' (`Table.stories.tsx`).
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./Table";
import {
  ControlBody,
  ControlCell,
  ControlColumn,
  ControlHead,
  ControlRow,
  ControlTable,
} from "./ControlTable";
import { cellsOf, tipInTable } from "./table.testkit";
import { shownTip } from "../tooltip/tooltip.testkit";

afterEach(cleanup);

const SKILLS = [
  {
    name: "deploy",
    does: "Ships a branch to the staging cluster, waits for its checks, and rolls it back where they fail, with the steps it took said in the chat.",
    chat: "Every chat in smart-ide",
    works: "Claude Code, Codex",
  },
  { name: "review", does: "Reviews a pull request.", chat: "Asked for", works: "Claude Code" },
];

/** The studio's case (#3671): a long "What it does" beside "In a chat" and "Works on". */
function Skills({ chosen = true }: { chosen?: boolean }) {
  return (
    <Table aria-label="Skills">
      <TableHeader>
        <TableColumn isRowHeader size={chosen ? "fit" : undefined}>
          Skill
        </TableColumn>
        <TableColumn
          size={chosen ? "fill" : undefined}
          clamp={chosen ? 2 : undefined}
          minWidth={chosen ? "12rem" : undefined}
        >
          What it does
        </TableColumn>
        <TableColumn cut={chosen} maxWidth={chosen ? 160 : undefined}>
          In a chat
        </TableColumn>
        <TableColumn size={chosen ? "fit" : undefined} width={chosen ? "8rem" : undefined}>
          Works on
        </TableColumn>
      </TableHeader>
      <TableBody>
        {SKILLS.map((skill) => (
          <TableRow key={skill.name} id={skill.name} textValue={skill.name}>
            <TableCell>{skill.name}</TableCell>
            <TableCell>{skill.does}</TableCell>
            <TableCell>{skill.chat}</TableCell>
            <TableCell>{skill.works}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

/** The rows of the table, its columns' names left out. */
const rows = () =>
  within(screen.getByRole("grid", { name: "Skills" }))
    .getAllByRole("row")
    .slice(1);

/** The words a cell draws as its column chose. */
const wordsOf = (cell: Element) => cell.querySelector<HTMLElement>('[data-part="words"]');

/**
 * **The box measures jsdom has none of**: words whose `cut` box is narrower than they are, as
 * the column that cuts them draws them. Every other element measures nothing.
 */
function cutWords(cut: boolean) {
  const own = (element: Element) =>
    cut && element instanceof HTMLElement && element.dataset.part === "words";
  const measures = {
    scrollWidth: (element: Element) => (own(element) ? 400 : 0),
    clientWidth: (element: Element) => (own(element) ? 120 : 0),
  };
  for (const [name, measure] of Object.entries(measures))
    Object.defineProperty(HTMLElement.prototype, name, {
      configurable: true,
      get(this: Element) {
        return measure(this);
      },
    });
}

function measuresNothing() {
  for (const name of ["scrollWidth", "clientWidth"])
    Object.defineProperty(HTMLElement.prototype, name, { configurable: true, get: () => 0 });
}

describe("a table of data whose view chose nothing", () => {
  beforeEach(measuresNothing);

  it("draws its cells as it always has: their words as they are, and no sizes", () => {
    render(<Skills chosen={false} />);
    const heads = screen.getAllByRole("columnheader");
    for (const head of heads) {
      expect(head).not.toHaveAttribute("data-size");
      expect(head).not.toHaveAttribute("data-cut");
      expect(head).not.toHaveAttribute("data-clamp");
      expect(head).not.toHaveAttribute("data-least");
      expect(head).not.toHaveAttribute("style");
    }
    for (const cell of cellsOf(rows()[0])) {
      expect(wordsOf(cell)).toBeNull();
      expect(cell.firstChild?.nodeType).toBe(Node.TEXT_NODE);
    }
  });
});

describe("a column the view sized and cut (#3671)", () => {
  afterEach(measuresNothing);

  it("says its choices on its name and draws each of its cells' words by them", () => {
    cutWords(false);
    render(<Skills />);
    const [skill, does, chat, works] = screen.getAllByRole("columnheader");
    expect(skill).toHaveAttribute("data-size", "fit");
    expect(does).toHaveAttribute("data-size", "fill");
    expect(does).toHaveAttribute("data-clamp", "2");
    expect(chat).toHaveAttribute("data-cut");
    expect(works?.getAttribute("style")).toContain("inline-size: 8rem");
    const [name, what, where, on] = cellsOf(rows()[0]).map((cell) => wordsOf(cell));
    expect(name).toHaveAttribute("data-size", "fit");
    expect(what).toHaveAttribute("data-clamp", "2");
    expect(what?.getAttribute("style")).toContain("min-inline-size: max(100%, 12rem)");
    expect(where).toHaveAttribute("data-cut");
    expect(where?.getAttribute("style")).toContain("max-inline-size: 160px");
    expect(on).toHaveAttribute("data-size", "fit");
    // The words are the cell's, all of them: a reader hears them whole.
    expect(within(rows()[0]).getByRole("gridcell", { name: SKILLS[0].does })).toBeVisible();
  });

  it("says a cut cell's whole words in its tooltip as the keyboard reaches it", () => {
    cutWords(true);
    render(<Skills />);
    const [, what, where] = cellsOf(rows()[0]);
    expect(tipInTable(where)).toBe(SKILLS[0].chat);
    expect(tipInTable(what)).toBe(SKILLS[0].does);
  });

  it("shows no tooltip on words that fit", () => {
    cutWords(false);
    render(<Skills />);
    const [, , where] = cellsOf(rows()[0]);
    expect(where).not.toHaveAttribute("data-tooltip-target");
    expect(tipInTable(where)).toBeNull();
  });

  it("opens the tooltip on the keyboard's arrows, and Esc takes it down without moving them", async () => {
    cutWords(true);
    const user = userEvent.setup();
    render(<Skills />);
    await user.tab();
    expect(rows()[0]).toHaveFocus();
    await user.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
    const where = cellsOf(rows()[0])[2];
    expect(where).toHaveFocus();
    expect(shownTip()).toHaveTextContent(SKILLS[0].chat);
    await user.keyboard("{Escape}");
    expect(shownTip()).toBeNull();
    expect(where).toHaveFocus();
    // The next cell's tooltip is its own.
    await user.keyboard("{ArrowLeft}");
    expect(shownTip()).toHaveTextContent(SKILLS[0].does);
  });
});

/** The studio's skills as a table of controls draws them (#3671). */
function StudioSkills() {
  return (
    <ControlTable aria-label="Studio skills">
      <ControlHead>
        <ControlColumn size="fit">Skill</ControlColumn>
        <ControlColumn size="fill" clamp={2}>
          What it does
        </ControlColumn>
        <ControlColumn cut maxWidth="10rem">
          In a chat
        </ControlColumn>
        <ControlColumn>Change</ControlColumn>
      </ControlHead>
      <ControlBody>
        {SKILLS.map((skill) => (
          <ControlRow key={skill.name}>
            <ControlCell>{skill.name}</ControlCell>
            <ControlCell>{skill.does}</ControlCell>
            <ControlCell>{skill.chat}</ControlCell>
            <ControlCell>
              <button type="button">{`Change ${skill.name}`}</button>
            </ControlCell>
          </ControlRow>
        ))}
      </ControlBody>
    </ControlTable>
  );
}

describe("a table of controls whose columns chose (#3671)", () => {
  afterEach(measuresNothing);

  it("draws each cell's words by its column's choices, and a column that chose none as it was", () => {
    cutWords(false);
    render(<StudioSkills />);
    const table = screen.getByRole("table", { name: "Studio skills" });
    const [first] = within(table).getAllByRole("row").slice(1);
    const [skill, does, chat, change] = within(first).getAllByRole("cell");
    expect(wordsOf(skill)).toHaveAttribute("data-size", "fit");
    expect(wordsOf(does)).toHaveAttribute("data-clamp", "2");
    expect(wordsOf(chat)).toHaveAttribute("data-cut");
    expect(wordsOf(chat)?.getAttribute("style")).toContain("max-inline-size: 10rem");
    expect(wordsOf(change)).toBeNull();
    // Words that fit are no Tab stop.
    expect(wordsOf(chat)).not.toHaveAttribute("tabindex");
  });

  it("makes words cut short a Tab stop whose tooltip says them whole", async () => {
    cutWords(true);
    const user = userEvent.setup();
    render(<StudioSkills />);
    const table = screen.getByRole("table", { name: "Studio skills" });
    const [first] = within(table).getAllByRole("row").slice(1);
    const [, does, chat] = within(first).getAllByRole("cell");
    await user.tab();
    expect(wordsOf(does)).toHaveFocus();
    expect(shownTip()).toHaveTextContent(SKILLS[0].does);
    await user.tab();
    expect(wordsOf(chat)).toHaveFocus();
    expect(shownTip()).toHaveTextContent(SKILLS[0].chat);
    await user.keyboard("{Escape}");
    expect(shownTip()).toBeNull();
    expect(wordsOf(chat)).toHaveFocus();
  });

  it("shows its tooltip to the pointer after the window's delay", () => {
    cutWords(true);
    render(<StudioSkills />);
    const table = screen.getByRole("table", { name: "Studio skills" });
    const chat = wordsOf(within(table).getAllByRole("cell")[2]) as HTMLElement;
    act(() => {
      fireEvent.pointerEnter(chat, { pointerType: "mouse" });
    });
    expect(shownTip()).toBeNull();
    // The window's delay, waited out for real: short enough for a test.
    return new Promise<void>((resolve) =>
      setTimeout(() => {
        expect(shownTip()).toHaveTextContent(SKILLS[0].chat);
        resolve();
      }, 600),
    );
  });
});

describe("a table of data that stacks (#3671)", () => {
  it("says so on its frame and its body, and each cell's column over it", () => {
    render(
      <Table aria-label="Runs" stacks>
        <TableHeader>
          <TableColumn isRowHeader>Chat</TableColumn>
          <TableColumn>How it went</TableColumn>
        </TableHeader>
        <TableBody>
          <TableRow id="one" textValue="steward 5">
            <TableCell heading="Chat">steward 5</TableCell>
            <TableCell heading="How it went">finished</TableCell>
          </TableRow>
        </TableBody>
      </Table>,
    );
    const grid = screen.getByRole("grid", { name: "Runs" });
    expect(grid).toHaveAttribute("data-stacks");
    expect(grid.closest('[data-part="frame"]')).toHaveAttribute("data-stacks");
    const [chat, went] = cellsOf(within(grid).getAllByRole("row")[1]);
    expect(chat).toHaveAttribute("data-heading", "Chat");
    expect(went).toHaveAttribute("data-heading", "How it went");
  });
});
