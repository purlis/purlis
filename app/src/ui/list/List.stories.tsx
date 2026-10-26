// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The list family** (#2911): a list to choose from (Start a chat's personas, at the large
 * size, under its filter), a side view's dense list of rows that act (a panel's todos, under
 * their days, with a row's own action and its menu), a grid of cards, a list a field aims at,
 * and a list with nothing in it. Each is walked by the keyboard, held to axe
 * and compared with its baseline in Chromium and WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { CircleDashed, Star } from "lucide-react";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Button } from "../button/Button";
import { Menu, MenuItem, MenuWords } from "../menu/Menu";
import { List, ListBox, ListOption, ListRow, ListRowSection, type Selection } from "./List";
import { ListFilter, noMatchSaid, useListFilter } from "./ListFilter";
import { AimedList, AimedOption } from "./AimedList";
import { TextField } from "../field/Field";
import { RowDescription, RowLabel, RowLead, RowTrail } from "./Row";

/** Twenty-eight personas, as Start a chat lists them (R164: a list over about seven is filtered). */
const PERSONAS = [
  "steward",
  "web-dev",
  "reviewer",
  "release-captain",
  "security",
  "docs-writer",
  "designer",
  "devops",
  "data-eng",
  "qa",
  "triage",
  "support",
  "product",
  "researcher",
  "mobile",
  "infra",
  "platform",
  "growth",
  "billing",
  "search",
  "ml-ops",
  "compliance",
  "localiser",
  "accessibility",
  "perf",
  "on-call",
  "archivist",
  "scribe",
];

/** A persona's tile: the initials of its first two hyphen words (R19). */
const initials = (name: string) =>
  name
    .split("-")
    .slice(0, 2)
    .map((word) => word[0]?.toUpperCase() ?? "")
    .join("");

const PERSONA = { one: "persona", many: "personas" };

/** **Start a chat's personas**: the filter over them, the chosen one followed by the arrows. */
function Personas() {
  const { query, setQuery, shown } = useListFilter(PERSONAS, (name) => name);
  const [chosen, setChosen] = useState<Selection>(new Set(["steward"]));
  return (
    <div
      style={{
        inlineSize: "16rem",
        blockSize: "22rem",
        display: "grid",
        gridTemplateRows: "auto minmax(0, 1fr)",
      }}
    >
      <ListFilter noun={PERSONA} total={PERSONAS.length} query={query} onQuery={setQuery} />
      <ListBox
        aria-label="Personas"
        size="lg"
        selectionMode="single"
        disallowEmptySelection
        selectedKeys={chosen}
        onSelectionChange={setChosen}
        style={{ overflow: "auto" }}
        empty={<p style={{ margin: 0 }}>{noMatchSaid(PERSONA, query)}</p>}
      >
        {shown.map((name) => (
          <ListOption key={name} id={name} textValue={name}>
            <RowLead>{initials(name)}</RowLead>
            <RowLabel>{name}</RowLabel>
            {name === "steward" && <RowDescription>default</RowDescription>}
          </ListOption>
        ))}
      </ListBox>
    </div>
  );
}

const meta = {
  component: Personas,
} satisfies Meta<typeof Personas>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **An option is named by its label and described by its one line** (D-2911-22): the APG
 * listbox and React Aria's slots, which meets WCAG 2.5.3 since the name is the visible label.
 * axe's experimental `label-content-name-mismatch` compares the name with the whole of the
 * option's visible text (its mark and its line as well), so it is off on these stories alone.
 */
const NAMED_BY_ITS_LABEL = {
  a11y: { options: { rules: { "label-content-name-mismatch": { enabled: false } } } },
};

/**
 * **A list to choose from**: one Tab stop on the chosen row, the arrows choose as they move and
 * stop at the ends, typing finds a row, and the filter narrows the list to what is typed, with
 * its empty line where nothing matches. The picture is the chosen row under the keyboard.
 */
export const ToChooseFrom: Story = {
  parameters: NAMED_BY_ITS_LABEL,
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("searchbox", { name: "Filter personas" });
    await expect(box).toHaveAttribute("placeholder", "Filter 28 personas");
    await userEvent.tab();
    await untilFocused(() => box, "To choose from: Tab lands on the filter");
    await userEvent.tab();
    const list = canvas.getByRole("listbox", { name: "Personas" });
    await untilFocused(
      () => within(list).getByRole("option", { name: "steward" }),
      "To choose from: Tab lands on the chosen persona",
    );
    await userEvent.keyboard("{ArrowDown}");
    const second = within(list).getByRole("option", { name: "web-dev" });
    await untilFocused(() => second, "To choose from: Down moves to web-dev");
    await expect(second).toHaveAttribute("aria-selected", "true");
    expectFocusInSight();
    await userEvent.keyboard("{End}");
    await untilFocused(
      () => within(list).getByRole("option", { name: "scribe" }),
      "To choose from: End moves to the last persona",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(list).getByRole("option", { name: "scribe" }),
      "To choose from: Down stops at the last persona",
    );
    await userEvent.keyboard("{Home}");
    await untilFocused(
      () => within(list).getByRole("option", { name: "steward" }),
      "To choose from: Home moves to the first persona",
    );
    // The filter, with its empty line.
    await userEvent.click(box);
    await userEvent.type(box, "zz");
    await waitFor(() => expect(canvas.getByText("No persona is called “zz”.")).toBeVisible());
    await userEvent.clear(box);
    await userEvent.type(box, "re");
    // A case-insensitive part of each name (R12).
    await waitFor(() => {
      const left = within(list).getAllByRole("option");
      expect(left.length).toBeGreaterThan(0);
      // Its name is its label's words (the option's `label` slot).
      for (const one of left)
        expect(one.querySelector('[data-part="label"]')?.textContent ?? "").toMatch(/re/i);
    });
    await userEvent.clear(box);
    await userEvent.tab();
    await untilFocused(
      () => within(list).getByRole("option", { name: "steward" }),
      "To choose from: Tab comes back to the chosen persona",
    );
  },
};

/** A dated list of todos, as a side view draws one: dense rows under their days. */
function Todos() {
  const [said, setSaid] = useState("Nothing run");
  return (
    <div style={{ inlineSize: "18rem", display: "grid", gap: "var(--space-3)" }}>
      <List aria-label="Todos" size="sm" onAction={(key) => setSaid(`Opened ${String(key)}`)}>
        <ListRowSection title="Today" named>
          <ListRow
            id="write"
            textValue="Write the release notes"
            menu={() => (
              <Menu
                aria-label="Write the release notes"
                onAction={(key) => setSaid(`Chose ${String(key)}`)}
              >
                <MenuItem id="done" textValue="Mark done">
                  <MenuWords said="Mark done" />
                </MenuItem>
                <MenuItem id="forget" textValue="Forget" variant="danger">
                  <MenuWords said="Forget" />
                </MenuItem>
              </Menu>
            )}
          >
            <RowLead>
              <CircleDashed aria-hidden="true" />
            </RowLead>
            <RowLabel>Write the release notes</RowLabel>
            <RowTrail>
              <Button size="sm" onPress={() => setSaid("Asked for a review")}>
                Review
              </Button>
            </RowTrail>
          </ListRow>
          <ListRow id="ship" textValue="Ship the list family">
            <RowLead>
              <Star aria-hidden="true" />
            </RowLead>
            <RowLabel>Ship the list family</RowLabel>
          </ListRow>
        </ListRowSection>
        <ListRowSection title="Yesterday" named>
          <ListRow id="audit" textValue="Read the prototype audit" isDisabled>
            <RowLead>
              <CircleDashed aria-hidden="true" />
            </RowLead>
            <RowLabel>Read the prototype audit</RowLabel>
            <RowDescription>done</RowDescription>
          </ListRow>
        </ListRowSection>
      </List>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

/**
 * **Rows that act**: Enter runs the row, → reaches its own button, the arrows pass a row that
 * cannot be used, and Shift+F10 opens the row's menu on its first row; Escape gives the keyboard
 * back to the row. The picture is the row under the keyboard, its ring inside it.
 */
export const RowsThatAct: Story = {
  render: () => <Todos />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Todos" });
    const write = within(list).getByRole("row", { name: "Write the release notes" });
    await userEvent.tab();
    await untilFocused(() => write, "Rows that act: Tab lands on the first row");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Opened write"));
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(write).getByRole("button", { name: "Review" }),
      "Rows that act: Right reaches the row's own button",
    );
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Asked for a review"));
    await userEvent.keyboard("{ArrowLeft}");
    await untilFocused(() => write, "Rows that act: Left goes back to the row");
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "Write the release notes" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /Mark done/ }),
      "Rows that act: Shift+F10 opens the row's menu on its first row",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    await untilFocused(() => write, "Rows that act: Escape gives the keyboard back to the row");
    await userEvent.keyboard("{ArrowDown}");
    const ship = within(list).getByRole("row", { name: "Ship the list family" });
    await untilFocused(() => ship, "Rows that act: Down moves to the next row");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => ship, "Rows that act: Down passes over a row that cannot be used");
    expectFocusInSight();
  },
};

/** The theme gallery's cards: a grid the arrows walk in two directions. */
function Cards() {
  const [chosen, setChosen] = useState<Selection>(new Set(["purlis"]));
  const themes = ["purlis", "purlis-classic", "dusk", "team"];
  return (
    <ListBox
      aria-label="Themes"
      layout="grid"
      selectionMode="single"
      selectionBehavior="toggle"
      disallowEmptySelection
      selectedKeys={chosen}
      onSelectionChange={setChosen}
      style={{ gridTemplateColumns: "repeat(2, 10rem)" }}
    >
      {themes.map((theme) => (
        <ListOption key={theme} id={theme} textValue={theme}>
          <RowLabel>{theme}</RowLabel>
          <RowDescription>Built in</RowDescription>
        </ListOption>
      ))}
    </ListBox>
  );
}

/**
 * **A grid of cards**: the arrows walk across and down, Space chooses the card the keyboard is
 * on, and the chosen card is ringed all round. The picture is the chosen card.
 */
export const AGridOfCards: Story = {
  parameters: NAMED_BY_ITS_LABEL,
  render: () => <Cards />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("listbox", { name: "Themes" });
    await userEvent.tab();
    await untilFocused(
      () => within(list).getByRole("option", { name: "purlis" }),
      "A grid of cards: Tab lands on the chosen card",
    );
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(list).getByRole("option", { name: "dusk" }),
      "A grid of cards: Down moves to the card below",
    );
    await userEvent.keyboard("{ArrowRight}");
    const team = within(list).getByRole("option", { name: "team" });
    await untilFocused(() => team, "A grid of cards: Right moves to the card beside");
    await userEvent.keyboard(" ");
    await waitFor(() => expect(team).toHaveAttribute("aria-selected", "true"));
    expectFocusInSight();
  },
};

/** **A list with nothing in it** says so in its own words, and still takes the keyboard. */
export const Empty: Story = {
  render: () => (
    <div style={{ inlineSize: "16rem" }}>
      <List aria-label="Browsers" items={[] as { id: string }[]} empty="No browser is open.">
        {(one) => <ListRow id={one.id}>{one.id}</ListRow>}
      </List>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Browsers" });
    await expect(list).toHaveTextContent("No browser is open.");
    await userEvent.tab();
    await untilFocused(() => list, "Empty: Tab lands on the list");
    expectFocusInSight();
  },
};

/** The palette's rows: a field that keeps the keyboard and aims at a row of its list. */
function Aimed() {
  const actions = ["Open a chat", "Start a task", "End this chat"];
  const [at, setAt] = useState(0);
  const [said, setSaid] = useState("Nothing run");
  return (
    <div style={{ inlineSize: "18rem", display: "grid", gap: "var(--space-3)" }}>
      <TextField
        aria-label="Run an action"
        role="combobox"
        aria-expanded="true"
        aria-controls="story-actions"
        aria-activedescendant={`story-action-${at}`}
        value=""
        onChange={() => {}}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") setAt((was) => Math.min(actions.length - 1, was + 1));
          else if (e.key === "ArrowUp") setAt((was) => Math.max(0, was - 1));
          else if (e.key === "Enter") setSaid(`Ran ${actions[at]}`);
          else return;
          e.preventDefault();
        }}
      />
      <AimedList id="story-actions" aria-label="Actions">
        {actions.map((one, index) => (
          <AimedOption
            key={one}
            id={`story-action-${index}`}
            aimed={index === at}
            unavailable={index === 2}
            onPress={() => setSaid(`Ran ${one}`)}
          >
            <RowLabel>{one}</RowLabel>
            {index === 2 && <RowTrail>No chat is in front.</RowTrail>}
          </AimedOption>
        ))}
      </AimedList>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

/**
 * **A list a field aims at** (the palette): the keyboard stays in the field, ↓ moves the aim,
 * and Enter runs the aimed row. The picture is the aim on the second row.
 */
export const AimedByAField: Story = {
  render: () => <Aimed />,
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("combobox", { name: "Run an action" });
    await userEvent.tab();
    await untilFocused(() => box, "Aimed by a field: Tab lands on the field");
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() =>
      expect(canvas.getByRole("option", { name: "Start a task" })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );
    await expect(box).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Ran Start a task"));
    expectFocusInSight();
  },
};

/** What a todo's tooltip says: the whole of its words and when it was written, a few lines. */
const TODO_TIP =
  "Write the release notes for the list family, with the rows that act, the cards and the filter, and say what changed for themes. Written 2026-10-24 09:12.";

/**
 * **A bottom panel across the window, the left panel closed** (#3328): its rows run from the
 * rail to the window's right edge, so a row has no room at either side of it.
 */
function AcrossTheWindow() {
  return (
    <div
      style={{
        position: "fixed",
        insetInline: "110px 0",
        insetBlockEnd: 0,
        padding: "var(--space-3)",
        background: "var(--surface-raised)",
      }}
    >
      <List aria-label="Todos" size="sm">
        <ListRow id="notes" textValue="Write the release notes" tip={TODO_TIP}>
          <RowLead>
            <CircleDashed aria-hidden="true" />
          </RowLead>
          <RowLabel>Write the release notes</RowLabel>
        </ListRow>
        <ListRow
          id="audit"
          textValue="Read the whole prototype audit and file what is missing from the list family"
          tip={TODO_TIP}
        >
          <RowLead>
            <CircleDashed aria-hidden="true" />
          </RowLead>
          <RowLabel>
            Read the whole prototype audit and file what is missing from the list family
          </RowLabel>
        </ListRow>
      </List>
    </div>
  );
}

/** The tooltip shown now, which is drawn in the page and not in the story's frame. */
const tipShown = () => document.querySelector<HTMLElement>('[role="tooltip"]');

/** The tooltip keeps its width (its words fill its widest) and stays in the window. */
async function expectReadable(where: string) {
  const tip = await waitFor(() => {
    const up = tipShown();
    if (up === null) throw new Error(`${where}: the tooltip is not up`);
    return up;
  });
  const box = tip.getBoundingClientRect();
  await expect(box.width, `${where}: the tooltip keeps its width`).toBeGreaterThanOrEqual(300);
  await expect(box.left, `${where}: inside the window`).toBeGreaterThanOrEqual(0);
  await expect(box.right, `${where}: inside the window`).toBeLessThanOrEqual(window.innerWidth);
}

/**
 * **A row across the window keeps its tooltip readable** (#3328, R166): the tooltip stands by
 * the pointer or by the row's words, at their end, and under or over the row where neither side
 * has room; never a column of words against the window's edge. The picture is the second row
 * under the keyboard, its words cut, its tooltip over it.
 */
export const ARowAcrossTheWindow: Story = {
  parameters: { screenshot: "page" },
  render: () => <AcrossTheWindow />,
  play: async ({ canvas, userEvent }) => {
    const notes = canvas.getByRole("row", { name: "Write the release notes" });
    const box = notes.getBoundingClientRect();
    await userEvent.pointer({
      target: notes,
      coords: { clientX: box.left + box.width / 2, clientY: box.top + box.height / 2 },
    });
    await expectReadable("Across the window, under the pointer");
    await userEvent.unhover(notes);
    await waitFor(() => expect(tipShown()).toBeNull());
    await userEvent.tab();
    await untilFocused(() => notes, "Across the window: Tab lands on the first row");
    await expectReadable("Across the window, by the row's words");
    await userEvent.keyboard("{ArrowDown}");
    const audit = canvas.getByRole("row", { name: /Read the whole prototype audit/ });
    await untilFocused(() => audit, "Across the window: Down moves to the row whose words are cut");
    await expectReadable("Across the window, a row whose words fill it");
    expectFocusInSight();
  },
};
