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
import { expect, screen, waitFor, within, type userEvent as UserEvent } from "storybook/test";
import { CircleDashed, Star } from "lucide-react";
import {
  expectFocusInSight,
  keysOnTheEnginesKeyboard,
  tabInto,
  untilFocusInSight,
  untilFocused,
} from "../../stories.testkit";
import { Button } from "../button/Button";
import { Menu, MenuItem, MenuWords } from "../menu/Menu";
import { List, ListBox, ListOption, ListRow, ListRowSection, type Selection } from "./List";
import { ListFilter, noMatchSaid, useListFilter } from "./ListFilter";
import { AimedList, AimedOption } from "./AimedList";
import { TextArea, TextField } from "../field/Field";
import { Select } from "../select/Select";
import { RowBelow, RowDescription, RowLabel, RowLead, RowTrail } from "./Row";
import { Badge } from "../badge/Badge";
import { TOOLTIP_CLOSE_DELAY } from "../tooltip/Tooltip";

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
 * **A list to choose from**: one Tab stop on the chosen row, the arrows choose as they move and
 * stop at the ends, typing finds a row, and the filter narrows the list to what is typed, with
 * its empty line where nothing matches. The picture is the chosen row under the keyboard.
 */
export const ToChooseFrom: Story = {
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
    await untilFocusInSight("To choose from: web-dev is in sight");
    await userEvent.keyboard("{End}");
    await untilFocused(
      () => within(list).getByRole("option", { name: "scribe" }),
      "To choose from: End moves to the last persona",
    );
    // The list scrolls the last persona into sight after it takes the focus, which WebKit can
    // finish a frame later (#4078).
    await waitFor(() => expectFocusInSight());
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

/** Skills as the Skill studio's Library lists them: a name and its tags, then a line under. */
function LinesUnder() {
  return (
    <div style={{ inlineSize: "20rem" }}>
      <List aria-label="Skills" selectionMode="single" defaultSelectedKeys={["deploy"]}>
        <ListRow id="deploy" textValue="deploy">
          <RowLabel>deploy</RowLabel>
          <RowTrail>
            <Badge>project</Badge>
          </RowTrail>
          <RowBelow clamp={1} data-testid="below-deploy">
            Deploys a service to staging or production with the checks the team agreed on: tests
            green, a changelog entry and a tag.
          </RowBelow>
        </ListRow>
        <ListRow id="lint-rs" textValue="lint-rs">
          <RowLabel>lint-rs</RowLabel>
          <RowTrail>
            <Badge>workspace</Badge>
            <Badge variant="waiting">Off</Badge>
          </RowTrail>
          <RowBelow clamp={2} data-testid="below-lint-rs">
            Off in smart-ide: a workspace can turn a granted skill off, and smart-ide has, so its
            chats start without it until it is turned on again there.
          </RowBelow>
        </ListRow>
        <ListRow id="browser" textValue="browser">
          <RowLabel>browser</RowLabel>
          <RowTrail>
            <Badge>built-in</Badge>
          </RowTrail>
        </ListRow>
      </List>
    </div>
  );
}

/**
 * **A line under a row's words** (#3751): a skill's one-line description, cut at the end of its
 * line, and a reason that takes two lines. Each row grows to fit what is under its words and no
 * line is cut off mid-line; the arrows still walk one row at a time. The picture is the three
 * rows.
 */
export const LinesUnderTheWords: Story = {
  render: () => <LinesUnder />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Skills" });
    /** The height of what a line under holds, without its padding. */
    const wordsOf = (name: string) => {
      const below = canvas.getByTestId(`below-${name}`);
      const drawn = getComputedStyle(below);
      return below.clientHeight - parseFloat(drawn.paddingTop) - parseFloat(drawn.paddingBottom);
    };
    for (const name of ["deploy", "lint-rs"]) {
      const row = within(list).getByRole("row", { name: new RegExp(`^${name}`) });
      const belowBox = canvas.getByTestId(`below-${name}`).getBoundingClientRect();
      const label = within(row).getByText(name).getBoundingClientRect();
      // Under the words, inside the row: nothing of its lines is past the row's end.
      await expect(belowBox.top).toBeGreaterThanOrEqual(label.bottom - 1);
      await expect(belowBox.bottom).toBeLessThanOrEqual(row.getBoundingClientRect().bottom + 1);
    }
    // Cut at its clamp and never mid-line: the description is one whole line (it never wraps),
    // and the reason exactly two of them.
    const line = wordsOf("deploy");
    await expect(Math.abs(wordsOf("lint-rs") / line - 2)).toBeLessThan(0.1);
    await userEvent.tab();
    const deploy = within(list).getByRole("row", { name: /^deploy/ });
    await untilFocused(() => deploy, "Lines under: Tab lands on the chosen row");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(list).getByRole("row", { name: /^lint-rs/ }),
      "Lines under: Down moves to the next row, its line with it",
    );
    expectFocusInSight();
  },
};

/** Options as Use a skill and Search draw them: a line under the words, and a number heard. */
function OptionsWithLines() {
  return (
    <div style={{ inlineSize: "20rem", display: "grid", gap: "var(--space-3)" }}>
      <ListBox aria-label="Skills to use" selectionMode="single" defaultSelectedKeys={["review"]}>
        <ListOption id="review" textValue="review">
          <RowLabel>review</RowLabel>
          <RowDescription>Built in</RowDescription>
          <RowBelow clamp={1}>Reads a pull request and says what it would change</RowBelow>
        </ListOption>
        <ListOption id="deploy" textValue="deploy">
          <RowLabel>deploy</RowLabel>
          <RowBelow clamp={1}>Ships the app to staging</RowBelow>
        </ListOption>
      </ListBox>
      <ListBox aria-label="Matching lines" selectionMode="single" defaultSelectedKeys={[42]}>
        <ListOption id={42} textValue="42 let needle = 1;">
          <RowLead said="Line 42">42</RowLead>
          <RowLabel>
            let <mark>needle</mark> = 1;
          </RowLabel>
        </ListOption>
      </ListBox>
    </div>
  );
}

/**
 * **Every line an option draws about it is heard** (#6075): its one line, then its line under
 * its words (read whole, whatever its clamp cuts), then a mark given words to say, each once, as
 * its description; its name stays its words alone, which axe's `label-content-name-mismatch`
 * holds. The picture is the two lists, the first option under the keyboard.
 */
export const AnOptionsLinesAreHeard: Story = {
  render: () => <OptionsWithLines />,
  play: async ({ canvas, userEvent }) => {
    const skills = canvas.getByRole("listbox", { name: "Skills to use" });
    const review = within(skills).getByRole("option", { name: "review" });
    await expect(review).toHaveAccessibleDescription(
      "Built in Reads a pull request and says what it would change",
    );
    await expect(
      within(skills).getByRole("option", { name: "deploy" }),
    ).toHaveAccessibleDescription("Ships the app to staging");
    const hit = canvas.getByRole("option", { name: "let needle = 1;" });
    await expect(hit).toHaveAccessibleDescription("Line 42");
    await userEvent.tab();
    await untilFocused(() => review, "Lines heard: Tab lands on the chosen option");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(skills).getByRole("option", { name: "deploy" }),
      "Lines heard: Down moves to the next option",
    );
    await userEvent.tab();
    await untilFocused(() => hit, "Lines heard: Tab moves to the next list's chosen option");
    expectFocusInSight();
  },
};

/** A reason the Library's line cuts short, as #4711 found it, and one that fits. */
const CUT_REASON =
  "Left out: a name starting with purlis- is purlis's own, so this one is never handed to a chat here";

/** Skills left out, as the Skill studio's Library lists them, each line asking for its whole. */
function ReasonsCutShort() {
  return (
    <div style={{ inlineSize: "20rem" }}>
      <List aria-label="Skills">
        <ListRow id="purlis-review" textValue="purlis-review">
          <RowLabel>purlis-review</RowLabel>
          <RowTrail>
            <Badge>project</Badge>
            <Badge variant="waiting">Left out</Badge>
          </RowTrail>
          <RowBelow clamp={1} tipWhenCut data-testid="below-cut">
            {CUT_REASON}
          </RowBelow>
        </ListRow>
        <ListRow id="lint-rs" textValue="lint-rs">
          <RowLabel>lint-rs</RowLabel>
          <RowTrail>
            <Badge>workspace</Badge>
          </RowTrail>
          <RowBelow clamp={1} tipWhenCut data-testid="below-fits">
            Left out: off here
          </RowBelow>
        </ListRow>
      </List>
    </div>
  );
}

/**
 * **A line under the words, cut short, reads whole in the row's tooltip** (#4711): under the
 * pointer after the delay and at once under the keyboard, Esc taking it down; a line that fits
 * has none, and the row stays as it is drawn, its line one line. The picture is the cut row under
 * the keyboard, its whole reason in the tooltip.
 */
export const ALineCutShortReadsWhole: Story = {
  parameters: { screenshot: "page" },
  render: () => <ReasonsCutShort />,
  play: async ({ canvas, userEvent }) => {
    const cutRow = canvas.getByRole("row", { name: /^purlis-review/ });
    const fitsRow = canvas.getByRole("row", { name: /^lint-rs/ });
    const below = canvas.getByTestId("below-cut");
    const rowHeight = cutRow.getBoundingClientRect().height;
    const lineHeight = below.getBoundingClientRect().height;
    /** The row and its line are drawn as they were, the line one line, whatever is shown. */
    const stillOneLine = async (where: string) => {
      await expect(
        cutRow.getBoundingClientRect().height,
        `${where}: the row keeps its height`,
      ).toBe(rowHeight);
      await expect(below.getBoundingClientRect().height, `${where}: its line stays one`).toBe(
        lineHeight,
      );
      await expect(
        Math.abs(lineHeight - Number.parseFloat(getComputedStyle(below).lineHeight)),
        `${where}: the line is one line high`,
      ).toBeLessThan(Number.parseFloat(getComputedStyle(below).lineHeight) / 2);
    };
    await stillOneLine("At rest");
    // A line that fits: past the delay under the pointer, and no tooltip.
    await userEvent.hover(fitsRow);
    await new Promise((done) => setTimeout(done, 800));
    await expect(tipShown(), "A reason that fits shows no tooltip").toBeNull();
    // A line cut short: its whole words under the pointer, and Esc takes them down.
    await userEvent.hover(cutRow);
    await waitFor(() => expect(tipShown()).toHaveTextContent(CUT_REASON));
    await stillOneLine("Under the pointer");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(tipShown()).toBeNull());
    await userEvent.unhover(cutRow);
    // And under the keyboard, at once, by the row's words.
    await userEvent.tab();
    await untilFocused(() => cutRow, "Cut short: Tab lands on the first row");
    await waitFor(() => expect(tipShown()).toHaveTextContent(CUT_REASON));
    await stillOneLine("Under the keyboard");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => fitsRow, "Cut short: Down moves to the row whose line fits");
    await waitFor(() => expect(tipShown()).toBeNull());
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => cutRow, "Cut short: Up comes back to the row cut short");
    await waitFor(() => expect(tipShown()).toHaveTextContent(CUT_REASON));
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
  render: () => <Cards />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("listbox", { name: "Themes" });
    // **A card is named by its label alone and described by its line, heard once** (#6043):
    // named by its content, with its line kept out of it, so axe's label-in-name rule holds.
    const purlis = within(list).getByRole("option", { name: "purlis" });
    await expect(purlis).toHaveAccessibleDescription("Built in");
    await expect(purlis).not.toHaveAttribute("aria-labelledby");
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

/** Two theme cards as the gallery draws them, one refused: why, then where it comes from. */
function ARefusedCard() {
  return (
    <ListBox
      aria-label="Themes"
      layout="grid"
      selectionMode="single"
      selectionBehavior="toggle"
      disallowEmptySelection
      defaultSelectedKeys={["purlis"]}
      style={{ gridTemplateColumns: "repeat(2, 16rem)" }}
    >
      <ListOption id="purlis" textValue="purlis">
        <RowLabel>purlis</RowLabel>
        <RowDescription>Built in</RowDescription>
      </ListOption>
      <ListOption id="broken" textValue="broken" unavailable>
        {/* Why first, in its pictures' place over its name, as the gallery draws it. */}
        <RowDescription>surface.base is not a colour</RowDescription>
        <RowLabel>broken</RowLabel>
        <RowBelow data-testid="refused-from">Yours, on this machine · Not applied</RowBelow>
      </ListOption>
    </ListBox>
  );
}

/**
 * **A refused card says why, and where it comes from** (#6075): why it is refused, in its
 * pictures' place, then the line under its name, both heard after its name. The line under its
 * name is one more line of the card's column, never stretched to fill it. The picture is the two
 * cards, the refused one under the keyboard.
 */
export const ARefusedCardSaysWhereItComesFrom: Story = {
  render: () => <ARefusedCard />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("listbox", { name: "Themes" });
    const broken = within(list).getByRole("option", { name: "broken" });
    await expect(broken).toHaveAccessibleDescription(
      "surface.base is not a colour Yours, on this machine · Not applied",
    );
    const from = canvas.getByTestId("refused-from");
    // Its own words' height, never the card's: a line or two is under four of its ems.
    const em = parseFloat(getComputedStyle(from).fontSize);
    await expect(from.getBoundingClientRect().height).toBeLessThan(em * 4);
    await userEvent.tab();
    await untilFocused(
      () => within(list).getByRole("option", { name: "purlis" }),
      "A refused card: Tab lands on the chosen card",
    );
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => broken,
      "A refused card: Right moves to it, though it cannot be chosen",
    );
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

/** A row for each of the scale spec's 1,000 workspaces (#5501), as the palette offers them. */
const THOUSAND = Array.from(
  { length: 1_000 },
  (_, at) => `Pin workspace ws-${String(at).padStart(4, "0")}`,
);

/** The palette's rows: a field that keeps the keyboard and aims at a row of its list. */
function Aimed({ many = false }: { many?: boolean }) {
  const actions = many ? THOUSAND : ["Open a chat", "Start a task", "End this chat"];
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
      <AimedList
        id="story-actions"
        aria-label="Actions"
        // The palette's own place for its rows: 55vh, scrolling.
        className={many ? "palette-rows" : undefined}
        count={actions.length}
        aimed={at}
        option={(index) => (
          <AimedOption
            id={`story-action-${index}`}
            aimed={index === at}
            unavailable={index === 2}
            onPress={() => setSaid(`Ran ${actions[index]}`)}
          >
            <RowLabel>{actions[index]}</RowLabel>
            {index === 2 && <RowTrail>No chat is in front.</RowTrail>}
          </AimedOption>
        )}
      />
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

/**
 * **A list a field aims at, of 1,000 rows** (#5501): only the rows in sight are drawn, each saying
 * its place in the whole; ↑ and ↓ walk past the first screenful and the aimed row stays drawn and
 * in sight. The picture is the aim forty rows down.
 */
export const AimedThousand: Story = {
  render: () => <Aimed many />,
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("combobox", { name: "Run an action" });
    await userEvent.tab();
    await untilFocused(() => box, "Aimed thousand: Tab lands on the field");
    const list = canvas.getByRole("listbox", { name: "Actions" });
    await expect(within(list).getAllByRole("option").length).toBeLessThan(80);
    for (let step = 0; step < 40; step++) await userEvent.keyboard("{ArrowDown}");
    const aimed = canvas.getByRole("option", { name: "Pin workspace ws-0040" });
    await expect(aimed).toHaveAttribute("aria-selected", "true");
    await expect(aimed).toHaveAttribute("aria-posinset", "41");
    await expect(aimed).toHaveAttribute("aria-setsize", "1000");
    const seen = list.getBoundingClientRect();
    const row = aimed.getBoundingClientRect();
    // Within a pixel: the virtualizer rounds the scroller's height as it brings a row in.
    await expect(row.top).toBeGreaterThanOrEqual(seen.top - 1);
    await expect(row.bottom).toBeLessThanOrEqual(seen.bottom + 1);
    await expect(box).toHaveFocus();
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
        // The rail and the left bar, the left panel closed.
        insetInlineStart: "calc(var(--space-8) * 3)",
        insetInlineEnd: 0,
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
        {/* Words longer than the window is wide: they fill the row and are cut. */}
        <ListRow
          id="audit"
          textValue="Read the whole prototype audit, file what is missing from the list, tree and table families, and say for each one which screens draw it, which themes reach its parts and which stories hold it"
          tip={TODO_TIP}
        >
          <RowLead>
            <CircleDashed aria-hidden="true" />
          </RowLead>
          <RowLabel>
            Read the whole prototype audit, file what is missing from the list, tree and table
            families, and say for each one which screens draw it, which themes reach its parts and
            which stories hold it
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
  // Its words are long enough to fill the family's widest: 320 px, or the window less its edges.
  const widest = Number.parseFloat(getComputedStyle(tip).maxInlineSize);
  await expect(
    box.width,
    `${where}: the tooltip keeps its width (${widest} px wide at most, the window ${window.innerWidth} px)`,
  ).toBeGreaterThanOrEqual(widest - 1);
  await expect(box.left, `${where}: inside the window`).toBeGreaterThanOrEqual(0);
  await expect(box.right, `${where}: inside the window`).toBeLessThanOrEqual(window.innerWidth);
}

/**
 * **A row across the window keeps its tooltip readable** (#3328, R166): the tooltip stands by
 * the pointer or by the row's words, at their end, and under or over the row where neither side
 * has room; never a column of words against the window's edge. The pointer can go onto it, and
 * Esc takes it down until the keyboard's next visit (WCAG 1.4.13, #5038). The picture is the
 * second row under the keyboard, its words cut, its tooltip over it.
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
    // **It can be hovered** (WCAG 1.4.13, #5038): the pointer goes onto it and it stays.
    const tip = tipShown();
    if (tip === null) throw new Error("Across the window: the tooltip is not up");
    await userEvent.hover(tip);
    await new Promise((done) => setTimeout(done, TOOLTIP_CLOSE_DELAY * 3));
    await expect(tipShown(), "Across the window: the tooltip stays under the pointer").toBe(tip);
    // Esc takes it down, and leaves the page as it was.
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(tipShown()).toBeNull());
    await userEvent.unhover(notes);
    await waitFor(() => expect(tipShown()).toBeNull());
    await userEvent.tab();
    await untilFocused(() => notes, "Across the window: Tab lands on the first row");
    await expectReadable("Across the window, by the row's words");
    // Esc takes it down, and the keyboard's next visit brings it back (#5038).
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(tipShown()).toBeNull());
    await expect(notes).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => notes, "Across the window: Up comes back to the first row");
    await expectReadable("Across the window, by the row's words, after Esc");
    await userEvent.keyboard("{ArrowDown}");
    const audit = canvas.getByRole("row", { name: /Read the whole prototype audit/ });
    await untilFocused(() => audit, "Across the window: Down moves to the row whose words are cut");
    await expectReadable("Across the window, a row whose words fill it");
    expectFocusInSight();
  },
};

/** Three asks, the middle one with a box for why and a button after it, as an Inbox ask holds. */
function Holding() {
  const [reason, setReason] = useState("");
  return (
    <div style={{ inlineSize: "24rem" }}>
      <List aria-label="Asks" disallowTypeAhead>
        <ListRow id="build" textValue="Run cargo build">
          <RowLabel>Run cargo build</RowLabel>
        </ListRow>
        <ListRow id="host" textValue="Reach registry.npmjs.org">
          <RowLabel>Reach registry.npmjs.org</RowLabel>
          <RowTrail>
            <TextField
              aria-label="Why keep it blocked"
              size="sm"
              value={reason}
              onChange={setReason}
            />
            <Button size="sm">Keep blocked</Button>
          </RowTrail>
        </ListRow>
        <ListRow id="test" textValue="Run cargo test">
          <RowLabel>Run cargo test</RowLabel>
        </ListRow>
      </List>
    </div>
  );
}

/**
 * **A text box in a row keeps its keys, and typing finds no row** (#3123): → reaches the box
 * from its row; in it, ← and →, Home and End move its caret, a letter and a space are typed, and
 * ↑ and ↓ (a box of one line) go to the rows above and below; Esc gives the keyboard back to
 * its row; with typing-to-find off, a letter on a row moves nowhere. The picture is the box
 * holding the keyboard, with its words.
 */
export const ATextBoxInARow: Story = {
  render: () => <Holding />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Asks" });
    const build = within(list).getByRole("row", { name: "Run cargo build" });
    const host = within(list).getByRole("row", { name: "Reach registry.npmjs.org" });
    const box = () => within(host).getByRole("textbox", { name: "Why keep it blocked" });
    await userEvent.tab();
    await untilFocused(() => build, "A text box in a row: Tab lands on the first row");
    await userEvent.keyboard("r");
    await untilFocused(() => build, "A text box in a row: a letter finds no row");
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    await untilFocused(box, "A text box in a row: → reaches the box from its row");
    await userEvent.keyboard("not now{Home}N{ArrowRight}{End}.");
    await waitFor(() => expect(box()).toHaveValue("Nnot now."));
    await untilFocused(box, "A text box in a row: its caret keys and Space stay in it");
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => host, "A text box in a row: Esc gives the keyboard back to its row");
    await waitFor(() => expect(box()).toHaveValue("Nnot now."));
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(box, "A text box in a row: → reaches it again");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(list).getByRole("row", { name: "Run cargo test" }),
      "A text box in a row: ↓ from a box of one line goes to the row below",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowUp}{ArrowRight}");
    await untilFocused(box, "A text box in a row: the box holds the keyboard for the picture");
    expectFocusInSight();
  },
};

/**
 * **A key the input method sends** to `box`: composing, as Chromium says, or only by its key code,
 * 229, as WebKit says once the composition has ended. Its key code is put on the event itself,
 * as an engine may drop it from what a script makes the event with.
 */
function keyOfTheInputMethod(box: Element, key: string, composing: boolean) {
  const event = new KeyboardEvent("keydown", {
    key,
    isComposing: composing,
    bubbles: true,
    cancelable: true,
  });
  Object.defineProperty(event, "keyCode", { value: 229 });
  box.dispatchEvent(event);
}

/**
 * **Tab on the engine's own keyboard** (Shift+Tab with `shift`), so the browser makes the move;
 * on Storybook's page, which has no engine's keyboard, Testing Library's Tab.
 */
async function tabOnTheEngine(keys: ReturnType<typeof UserEvent.setup>, shift = false) {
  if (!(await keysOnTheEnginesKeyboard(shift ? "{Shift>}{Tab}{/Shift}" : "{Tab}")))
    await keys.tab({ shift });
}

/**
 * **One Tab leaves the list from `row` for the button after it, and Shift+Tab comes back to the
 * row**: the list is one Tab stop, whatever its rows hold (#6071), and a row whose controls are the
 * list's last is still where Shift+Tab lands, never its last control (#6033).
 */
async function tabsOutAndBack(
  row: HTMLElement,
  keys: ReturnType<typeof UserEvent.setup>,
  step: string,
) {
  const after = within(document.body).getByRole("button", { name: "After" });
  await tabOnTheEngine(keys);
  await untilFocused(() => after, `${step}: one Tab leaves the list`);
  await keys.tab({ shift: true });
  await untilFocused(() => row, `${step}: Shift+Tab comes back to the row`);
}

/**
 * **A key while a text box in a row composes is the input method's** (#5993): Esc that cancels a
 * composition (Chinese, Japanese, Korean input) leaves the keyboard in the box, and a ↓ that picks
 * a candidate leaves the list on its row, whether the browser says the key is composing or, as
 * WebKit does once the composition ends, only by its key code, 229. No engine's keyboard has an
 * input method, so the keys are the ones it sends. The picture is the box, still holding the
 * keyboard.
 */
export const ComposingInARowsBox: Story = {
  render: () => (
    <div style={{ display: "grid", gap: "var(--space-3)" }}>
      <Holding />
      <Button size="sm">After</Button>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Asks" });
    const host = within(list).getByRole("row", { name: "Reach registry.npmjs.org" });
    const box = () => within(host).getByRole("textbox", { name: "Why keep it blocked" });
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    await untilFocused(box, "Composing: → reaches the box from its row");
    await userEvent.keyboard("ni hao");
    keyOfTheInputMethod(box(), "Escape", true);
    await untilFocused(box, "Composing: Esc that cancels a composition stays in the box");
    keyOfTheInputMethod(box(), "Escape", false);
    await untilFocused(box, "Composing: Esc of key code 229 stays in the box");
    keyOfTheInputMethod(box(), "ArrowDown", false);
    await untilFocused(box, "Composing: ↓ that picks a candidate stays in the box");
    await waitFor(() => expect(box()).toHaveValue("ni hao"));
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => host, "Composing: Esc once it is done gives the keyboard to its row");
    await userEvent.keyboard("{ArrowDown}");
    const test = within(list).getByRole("row", { name: "Run cargo test" });
    await untilFocused(() => test, "Composing: ↓ on the row goes to the row below");
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => host, "Composing: ↑ goes back to the row with the box");
    await tabsOutAndBack(host, userEvent, "Composing");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(box, "Composing: the box holds the keyboard for the picture");
    // → into a box may select its words; End puts the caret after them, the picture #6025 drew.
    await userEvent.keyboard("{End}");
    await untilFocused(box, "Composing: End keeps the keyboard in the box");
    expectFocusInSight();
  },
};

const HOW_TO_FILL = [
  { value: "value", label: "A new value" },
  { value: "entry", label: "An entry I have" },
  { value: "skip", label: "Leave it empty" },
];

/** Three asks, the middle one a secret's, with a select of how to fill it, as an Inbox ask holds. */
function Choosing() {
  const [how, setHow] = useState("value");
  return (
    <div style={{ inlineSize: "24rem", display: "grid", gap: "var(--space-3)" }}>
      <List aria-label="Asks" disallowTypeAhead>
        <ListRow id="build" textValue="Run cargo build">
          <RowLabel>Run cargo build</RowLabel>
        </ListRow>
        <ListRow id="token" textValue="Fill NPM_TOKEN">
          <RowLabel>Fill NPM_TOKEN</RowLabel>
          <RowTrail>
            <Select
              aria-label="How to fill"
              size="sm"
              value={how}
              options={HOW_TO_FILL}
              onValueChange={setHow}
            />
          </RowTrail>
        </ListRow>
        <ListRow id="test" textValue="Run cargo test">
          <RowLabel>Run cargo test</RowLabel>
        </ListRow>
      </List>
      <Button size="sm">After</Button>
    </div>
  );
}

/**
 * **A select in a row keeps ↑ and ↓** (#5993): ↓ on its button opens its list, where the row
 * would have gone to the row below, and ↑ opens it too, where the row would have gone to the row
 * above, each on the option it holds; the arrows choose in it, Enter takes one, and the keyboard
 * is back on the button. The picture is the button, with what was chosen.
 *
 * **The ↓ that opens it is never the list's** (#6091, `keepsItsArrows`), and each key waits for
 * where it put the keyboard to settle before the next is pressed.
 */
export const ASelectInARow: Story = {
  render: () => <Choosing />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Asks" });
    const token = within(list).getByRole("row", { name: "Fill NPM_TOKEN" });
    const select = () => within(token).getByRole("button", { name: /How to fill/ });
    await tabInto(() => list);
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => token, "A select in a row: ↓ reaches its row");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(select, "A select in a row: → reaches its button from its row");
    await userEvent.keyboard("{ArrowDown}");
    const options = await screen.findByRole("listbox");
    await untilFocused(
      () => within(options).getByRole("option", { name: "A new value" }),
      "A select in a row: ↓ opens it on its first option",
    );
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(options).getByRole("option", { name: "An entry I have" }),
      "A select in a row: ↓ in its list goes to the next option",
    );
    await userEvent.keyboard("{Enter}");
    await untilFocused(select, "A select in a row: Enter takes an option, back on its button");
    await waitFor(() => expect(select()).toHaveTextContent("An entry I have"));
    await userEvent.keyboard("{ArrowUp}");
    const again = await screen.findByRole("listbox");
    await untilFocused(
      () => within(again).getByRole("option", { name: "An entry I have" }),
      "A select in a row: ↑ opens it on the option it holds",
    );
    await userEvent.keyboard("{Escape}");
    await untilFocused(select, "A select in a row: Esc closes it, back on its button");
    await userEvent.keyboard("{ArrowLeft}");
    await untilFocused(() => token, "A select in a row: ← goes back to its row");
    await userEvent.keyboard("{ArrowDown}");
    const test = within(list).getByRole("row", { name: "Run cargo test" });
    await untilFocused(() => test, "A select in a row: ↓ on the row goes to the row below");
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => token, "A select in a row: ↑ goes back to its row");
    await tabsOutAndBack(token, userEvent, "A select in a row");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(select, "A select in a row: its button holds the keyboard for the picture");
    expectFocusInSight();
  },
};

/** A reply long enough to wrap on to three lines of its box, with no line break in it. */
const LONG_REPLY =
  "Yes, publish it to the registry once the tests pass on both runners, and tell me when it is out.";

/** One word longer than a line of the reply box, which breaks inside it, where nothing hangs. */
const ONE_LONG_WORD =
  "sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08e3b0c44298fc1c14";

/** Three asks, the middle one with a reply box narrow enough that its words wrap. */
function Replying({ words = LONG_REPLY }: { words?: string }) {
  const [reply, setReply] = useState(words);
  return (
    <div style={{ inlineSize: "20rem", display: "grid", gap: "var(--space-3)" }}>
      <List aria-label="Asks" disallowTypeAhead>
        <ListRow id="build" textValue="Run cargo build">
          <RowLabel>Run cargo build</RowLabel>
        </ListRow>
        <ListRow id="publish" textValue="Publish 0.4.0">
          <RowLabel>Publish 0.4.0</RowLabel>
          <TextArea aria-label="Reply" rows={4} value={reply} onChange={setReply} />
        </ListRow>
        <ListRow id="test" textValue="Run cargo test">
          <RowLabel>Run cargo test</RowLabel>
        </ListRow>
      </List>
      <Button size="sm">After</Button>
    </div>
  );
}

/**
 * **↑ and ↓ in a reply that wraps go by the lines it draws** (#5993): with no line break in it,
 * ↓ from its first line moves the caret to the line its words wrap on to, and ↑ back; ↑ leaves
 * the box for the row above only from its first line, and ↓ for the row below only from its
 * last. The picture is the box holding the keyboard, its caret on a line between.
 */
export const AWrappedReplyInARow: Story = {
  render: () => <Replying />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Asks" });
    const publish = within(list).getByRole("row", { name: "Publish 0.4.0" });
    const box = () =>
      within(publish).getByRole("textbox", { name: "Reply" }) as HTMLTextAreaElement;
    const caret = () => box().selectionStart;
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    await untilFocused(box, "A wrapped reply: → reaches the box from its row");
    // The caret is put at its start by hand: Home and End go to a line's ends, never the box's.
    box().setSelectionRange(0, 0);
    // ↑ and ↓ move a box's caret a line only on the engine's own keyboard: on Storybook's page,
    // the walk ends here.
    if (!(await keysOnTheEnginesKeyboard("{ArrowDown}"))) return;
    await untilFocused(box, "A wrapped reply: ↓ from its first line stays in it");
    await waitFor(() => expect(caret()).toBeGreaterThan(0));
    await keysOnTheEnginesKeyboard("{ArrowUp}");
    await untilFocused(box, "A wrapped reply: ↑ from its second line stays in it");
    await waitFor(() => expect(caret()).toBe(0));
    await keysOnTheEnginesKeyboard("{ArrowUp}");
    await untilFocused(
      () => within(list).getByRole("row", { name: "Run cargo build" }),
      "A wrapped reply: ↑ from its first line goes to the row above",
    );
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    await untilFocused(box, "A wrapped reply: → reaches it again");
    box().setSelectionRange(LONG_REPLY.length, LONG_REPLY.length);
    await keysOnTheEnginesKeyboard("{ArrowUp}");
    await untilFocused(box, "A wrapped reply: ↑ from its last line stays in it");
    await waitFor(() => expect(caret()).toBeLessThan(LONG_REPLY.length));
    await keysOnTheEnginesKeyboard("{ArrowDown}");
    await untilFocused(box, "A wrapped reply: ↓ back on to its last line stays in it");
    await keysOnTheEnginesKeyboard("{ArrowDown}");
    const test = within(list).getByRole("row", { name: "Run cargo test" });
    await untilFocused(() => test, "A wrapped reply: ↓ from its last line goes to the row below");
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => publish, "A wrapped reply: ↑ goes back to its row");
    await tabsOutAndBack(publish, userEvent, "A wrapped reply");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(box, "A wrapped reply: the box holds the keyboard for the picture");
    box().setSelectionRange(0, 0);
    await keysOnTheEnginesKeyboard("{ArrowDown}");
    await untilFocused(box, "A wrapped reply: its caret on a line between for the picture");
    await waitFor(() => expect(caret()).toBeGreaterThan(0));
    expectFocusInSight();
  },
};

/** Three rows, each with a button of its own (or two), between two buttons outside the list. */
function Between({ two = false }: { two?: boolean }) {
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: "20rem" }}>
      <Button size="sm">Before</Button>
      <List aria-label="Runs" disallowTypeAhead>
        {["alpha", "beta", "gamma"].map((name) => (
          <ListRow key={name} id={name} textValue={name}>
            <RowLabel>{name}</RowLabel>
            <RowTrail>
              <Button size="sm">{`Open ${name}`}</Button>
              {two && <Button size="sm">{`Stop ${name}`}</Button>}
            </RowTrail>
          </ListRow>
        ))}
      </List>
      <Button size="sm">After</Button>
    </div>
  );
}

/**
 * **One Tab stop, the row the keyboard left** (#3123, #6071): one Tab from a row leaves the list,
 * past the row's own controls (the WAI-ARIA APG grid), and passing the list's last control on its
 * way (React Aria's) never makes another row the list's stop; Shift+Tab comes back to the row the
 * keyboard left. Run with the browser's own key events, whose timing a test document does not
 * have. The picture is that row under the keyboard.
 */
export const OneTabStopComingBack: Story = {
  render: () => <Between />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Runs" });
    const beta = within(list).getByRole("row", { name: "beta" });
    await userEvent.click(canvas.getByRole("button", { name: "Before" }));
    await userEvent.tab();
    await untilFocused(
      () => within(list).getByRole("row", { name: "alpha" }),
      "One Tab stop: Tab lands on the first row",
    );
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => beta, "One Tab stop: ↓ moves to the next row");
    // Tab from a row leaves the list at once, past the row's own controls (#6071).
    const after = canvas.getByRole("button", { name: "After" });
    await tabOnTheEngine(userEvent);
    await untilFocused(() => after, "One Tab stop: one Tab leaves the list");
    await userEvent.tab({ shift: true });
    await untilFocused(() => beta, "One Tab stop: Shift+Tab comes back to the row it left");
    expectFocusInSight();
  },
};

/**
 * **Shift+Tab back into a list lands on the row the keyboard left, though it holds the list's
 * last controls** (#6033, the WAI-ARIA grid's one Tab stop): from the last row's button, Tab
 * leaves the list, and Shift+Tab, which the browser lands on that button, comes back to the row;
 * → reaches the button again. The picture is the row under the keyboard.
 */
export const ShiftTabBackToARowWithControls: Story = {
  render: () => <Between />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Runs" });
    const gamma = within(list).getByRole("row", { name: "gamma" });
    const open = () => within(gamma).getByRole("button", { name: "Open gamma" });
    await userEvent.click(canvas.getByRole("button", { name: "Before" }));
    await userEvent.tab();
    await userEvent.keyboard("{End}");
    await untilFocused(() => gamma, "Back to a row: End moves to the last row");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(open, "Back to a row: → reaches its button");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "After" }),
      "Back to a row: Tab from its button leaves the list",
    );
    await userEvent.tab({ shift: true });
    await untilFocused(
      () => gamma,
      "Back to a row: Shift+Tab comes back to the row, not its button",
    );
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(open, "Back to a row: → reaches its button again");
    await userEvent.keyboard("{ArrowLeft}");
    await untilFocused(() => gamma, "Back to a row: ← goes back to the row for the picture");
    expectFocusInSight();
  },
};

/**
 * **Tab walks a row's controls, then leaves the list; Shift+Tab is the same, turned round**
 * (#6071, the WAI-ARIA APG grid): from a row's first control Tab goes on to its next, and from its
 * last Tab leaves the list, where Shift+Tab comes back to the row; from its last control Shift+Tab
 * goes back to the one before, and from its first Shift+Tab leaves the list the other way. The
 * picture is the row's last control under the keyboard.
 */
export const TabWalksARowsControlsThenLeaves: Story = {
  render: () => <Between two />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Runs" });
    const beta = within(list).getByRole("row", { name: "beta" });
    const open = () => within(beta).getByRole("button", { name: "Open beta" });
    const stop = () => within(beta).getByRole("button", { name: "Stop beta" });
    await userEvent.click(canvas.getByRole("button", { name: "Before" }));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    await untilFocused(open, "Walks a row: → reaches its first control");
    await tabOnTheEngine(userEvent);
    await untilFocused(stop, "Walks a row: Tab goes on to its next control");
    await tabOnTheEngine(userEvent);
    await untilFocused(
      () => canvas.getByRole("button", { name: "After" }),
      "Walks a row: Tab from its last control leaves the list",
    );
    await tabOnTheEngine(userEvent, true);
    await untilFocused(() => beta, "Walks a row: Shift+Tab comes back to the row");
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    await untilFocused(stop, "Walks a row: → reaches its last control");
    await tabOnTheEngine(userEvent, true);
    await untilFocused(open, "Walks a row: Shift+Tab goes back to the control before");
    await tabOnTheEngine(userEvent, true);
    await untilFocused(
      () => canvas.getByRole("button", { name: "Before" }),
      "Walks a row: Shift+Tab from its first control leaves the list the other way",
    );
    await tabOnTheEngine(userEvent);
    await untilFocused(() => beta, "Walks a row: Tab comes back to the row");
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    await untilFocused(stop, "Walks a row: its last control holds the keyboard for the picture");
    expectFocusInSight();
  },
};

/**
 * **↑ and ↓ at a soft wrap move within the box** (#6045): in a word longer than a line, which the
 * box breaks with nothing hanging, End puts the caret at the end of the first line, at the wrap,
 * and ↓ moves it on to the line below, where it once left the row; Home from the last line puts
 * it at the start of that line, at the wrap again, and ↑ moves it up. Only from the last line
 * does ↓ leave for the row below. The picture is the box holding the keyboard, its caret at the
 * wrap.
 */
export const ACaretAtASoftWrap: Story = {
  render: () => <Replying words={ONE_LONG_WORD} />,
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Asks" });
    const publish = within(list).getByRole("row", { name: "Publish 0.4.0" });
    const box = () =>
      within(publish).getByRole("textbox", { name: "Reply" }) as HTMLTextAreaElement;
    const caret = () => box().selectionStart;
    const end = ONE_LONG_WORD.length;
    await tabOnTheEngine(userEvent);
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    await untilFocused(box, "At a soft wrap: → reaches the box from its row");
    box().setSelectionRange(0, 0);
    // Home, End and the arrows move a box's caret by its lines only on the engine's keyboard.
    if (!(await keysOnTheEnginesKeyboard("{End}"))) return;
    await waitFor(() => expect(caret()).toBeGreaterThan(0));
    const wrap = caret();
    expect(wrap).toBeLessThan(end);
    await keysOnTheEnginesKeyboard("{ArrowDown}");
    await untilFocused(box, "At a soft wrap: ↓ from the end of the first line stays in the box");
    await waitFor(() => expect(caret()).toBeGreaterThan(wrap));
    box().setSelectionRange(end, end);
    await keysOnTheEnginesKeyboard("{Home}");
    await waitFor(() => expect(caret()).toBeLessThan(end));
    const start = caret();
    expect(start).toBeGreaterThan(0);
    await keysOnTheEnginesKeyboard("{ArrowUp}");
    await untilFocused(box, "At a soft wrap: ↑ from the start of the last line stays in the box");
    await waitFor(() => expect(caret()).toBeLessThan(start));
    box().setSelectionRange(end, end);
    await keysOnTheEnginesKeyboard("{ArrowDown}");
    await untilFocused(
      () => within(list).getByRole("row", { name: "Run cargo test" }),
      "At a soft wrap: ↓ from the last line goes to the row below",
    );
    await userEvent.keyboard("{ArrowUp}{ArrowRight}");
    await untilFocused(box, "At a soft wrap: → reaches the box again");
    box().setSelectionRange(0, 0);
    await keysOnTheEnginesKeyboard("{End}");
    await untilFocused(box, "At a soft wrap: the box holds the keyboard for the picture");
    await waitFor(() => expect(caret()).toBe(wrap));
    expectFocusInSight();
  },
};
