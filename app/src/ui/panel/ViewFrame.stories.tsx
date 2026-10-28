// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The view frame** (#3666, the Skill studio frame's variant A): the studio's pages as the
 * prototype draws them, wide (the navigation, the Library's list and the skill beside it) and
 * narrow (page-and-back, the navigation as a picker). Walked by the keyboard, held to axe, and
 * measured so that a new item or a count that changes moves no pane (#2800).
 *
 * The data is the prototype's, cut short; no skills page is built here (#3681, #3682 do that).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { Ellipsis } from "lucide-react";
import { expectFocusInSight, untilFocused, untilFocusWithin } from "../../stories.testkit";
import { Badge } from "../badge/Badge";
import { Button, IconButton } from "../button/Button";
import { List, ListRow, type Selection } from "../list/List";
import { RowDescription, RowLabel, RowTrail } from "../list/Row";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "../menu/Menu";
import { NoticeBox, NoticeMessage } from "../notice/Notice";
import { Panel } from "./Panel";
import { ViewFrame, ViewListHead, ViewPageHeader, type ViewPage } from "./ViewFrame";

type Skill = { name: string; level: string; does: string; mark?: [string, "waiting" | "accent"] };

const SKILLS: Skill[] = [
  {
    name: "deploy",
    level: "project",
    does: "Deploys a service to staging or production with the checks the team agreed on.",
  },
  {
    name: "review-pr",
    level: "project",
    does: "Reviews a pull request against REVIEW.md: correctness first, then the standards.",
  },
  {
    name: "brainstorming",
    level: "project",
    does: "Turns a loose idea into a short design before any code is written.",
    mark: ["Update available", "accent"],
  },
  {
    name: "lint-rs",
    level: "workspace smart-ide",
    does: "Runs clippy with the workspace's flags and explains each warning in one line.",
    mark: ["Off", "waiting"],
  },
  {
    name: "grilling",
    level: "persona steward",
    does: "Interviews the person about a plan until every branch of the design is settled.",
    mark: ["Needs review", "waiting"],
  },
  {
    name: "browser",
    level: "built-in",
    does: "Drives a real browser for a task that needs one, with credentials from a vault.",
  },
];

/** The studio's pages, with the counts a story can change. */
function pagesOf(needs: number): ViewPage[] {
  return [
    { id: "library", label: "Library" },
    { id: "needs", label: "Needs you", count: needs, waiting: true, countSaid: `${needs} waiting` },
    { id: "drafts", label: "Drafts", count: 1, countSaid: "1 draft" },
    { id: "sources", label: "Sources" },
    { id: "deleted", label: "Deleted" },
    { id: "harness", label: "Harness skills", startsGroup: true },
    { id: "routing", label: "Routing" },
  ];
}

/** The skill chosen in the Library, as the item beside the list draws it. */
function SkillItem({ skill }: { skill: Skill }) {
  return (
    <>
      <ViewPageHeader
        name={skill.name}
        tag={<Badge>{skill.level}</Badge>}
        mark={skill.mark && <Badge variant={skill.mark[1]}>{skill.mark[0]}</Badge>}
        description={skill.does}
        actions={
          <>
            <Button variant="primary">Use in this chat</Button>
            <Button>Edit</Button>
            <Button>Grade</Button>
          </>
        }
        more={
          <MenuTrigger>
            <IconButton aria-label={`More for ${skill.name}`} icon={<Ellipsis />} />
            <Menu aria-label={`More for ${skill.name}`}>
              <MenuItem id="rename" textValue="Rename…">
                <MenuWords said="Rename…" />
              </MenuItem>
              <MenuItem id="delete" textValue="Delete…" variant="danger">
                <MenuWords said="Delete…" />
              </MenuItem>
            </Menu>
          </MenuTrigger>
        }
      />
      {skill.mark?.[0] === "Needs review" && (
        <NoticeBox tone="trouble" placement="alone" opened={false} asks={false}>
          <NoticeMessage>A chat changed it. Look at what changed before chats use it.</NoticeMessage>
        </NoticeBox>
      )}
      <dl style={{ margin: 0, padding: "var(--space-6)" }}>
        <dt>Who gets it</dt>
        <dd>Every chat in smart-ide, including steward 5.</dd>
        <dt>Runs</dt>
        <dd>41 in the last 30 days</dd>
      </dl>
    </>
  );
}

/** The studio as a view tab draws it: the frame in the content panel, `width` px wide. */
function Studio({ width, narrow }: { width: number; narrow?: boolean }) {
  const [page, setPage] = useState("library");
  const [chosen, setChosen] = useState<string>("deploy");
  const [opened, setOpened] = useState(false);
  const [needs, setNeeds] = useState(2);
  const skill = SKILLS.find((one) => one.name === chosen);
  const library = page === "library";
  return (
    <>
      <Button size="sm" onPress={() => setNeeds((was) => was + 8)}>
        More need you
      </Button>
      <div
        data-testid="tab"
        style={{
          display: "grid",
          gridTemplateRows: "minmax(0, 1fr)",
          inlineSize: width,
          blockSize: 460,
          marginBlockStart: "var(--space-4)",
        }}
      >
        <Panel variant="content">
        <ViewFrame
          label="Skill studio pages"
          pages={pagesOf(needs)}
          page={page}
          onPage={(id) => {
            setPage(id);
            setOpened(false);
          }}
          narrow={narrow}
          opened={opened}
          onBack={() => setOpened(false)}
          list={
            library ? (
              <>
                <ViewListHead
                  title="Library"
                  lead="Every skill the chats in smart-ide · steward can use."
                />
                <List
                  aria-label="Skills"
                  selectionMode="single"
                  disallowEmptySelection
                  selectedKeys={new Set([chosen])}
                  onSelectionChange={(keys: Selection) => {
                    const [first] = keys === "all" ? [] : [...keys];
                    if (first === undefined) return;
                    setChosen(String(first));
                    setOpened(true);
                  }}
                >
                  {SKILLS.map((one) => (
                    <ListRow key={one.name} id={one.name} textValue={one.name}>
                      <RowLabel>{one.name}</RowLabel>
                      <RowDescription>{one.does}</RowDescription>
                      <RowTrail>
                        <Badge>{one.level.split(" ")[0]}</Badge>
                      </RowTrail>
                    </ListRow>
                  ))}
                </List>
              </>
            ) : (
              <ViewListHead
                title={pagesOf(needs).find((one) => one.id === page)?.label}
                lead="This page is drawn in its own prototype."
              />
            )
          }
          item={library && skill !== undefined ? <SkillItem skill={skill} /> : undefined}
        />
        </Panel>
      </div>
    </>
  );
}

const meta = { component: Studio } satisfies Meta<typeof Studio>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Each pane's box, rounded, for a story to tell that nothing moved. */
function boxes(root: HTMLElement): string {
  return [...root.querySelectorAll<HTMLElement>('[data-scope="view-frame"][data-part]')]
    .filter((one) => ["nav", "list", "item"].includes(one.dataset.part ?? ""))
    .map((one) => {
      const box = one.getBoundingClientRect();
      return `${one.dataset.part} ${Math.round(box.left)} ${Math.round(box.width)} ${Math.round(box.height)}`;
    })
    .join(", ");
}

/**
 * **Wide: three panes.** Tab reaches the navigation's one stop, then the list, then the item's
 * actions; the arrows walk the navigation and Enter chooses a page.
 */
export const Wide: Story = {
  args: { width: 1000 },
  play: async ({ canvas, userEvent }) => {
    const nav = canvas.getByRole("navigation", { name: "Skill studio pages" });
    const library = canvas.getByRole("button", { name: "Library" });
    await expect(library).toHaveAttribute("aria-current", "page");
    // Past the story's own button, the navigation's one stop: the current page.
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => library, "the current page");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => canvas.getByRole("button", { name: /^Needs you/ }), "the next page");
    await expect(library).toHaveAttribute("aria-current", "page");
    // Tab leaves the navigation in one step, into the list.
    await userEvent.tab();
    await untilFocusWithin(() => canvas.getByRole("grid", { name: "Skills" }), "the list");
    expectFocusInSight();
    await expect(nav.contains(document.activeElement)).toBe(false);
    // Then the handle between the list and the item, then the item's actions.
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("separator", { name: "Resize the list and the item beside it" }),
      "the handle between the panes",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Use in this chat" }),
      "the item's first action",
    );
    expectFocusInSight();
    // Back into the navigation: its stop is the current page again; Enter chooses another.
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    await untilFocused(() => library, "the current page again");
    await userEvent.keyboard("{End}{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("button", { name: "Routing" })).toHaveAttribute(
        "aria-current",
        "page",
      ),
    );
    await expect(canvas.queryByRole("grid", { name: "Skills" })).toBeNull();
    await userEvent.keyboard("{Home}{Enter}");
    await waitFor(() => expect(library).toHaveAttribute("aria-current", "page"));
  },
};

/** **Nothing moves**: another skill chosen, or a count from 2 to 10, changes no pane's box. */
export const NothingMoves: Story = {
  args: { width: 1000 },
  play: async ({ canvas, userEvent }) => {
    const tab = canvas.getByTestId("tab");
    const before = boxes(tab);
    await userEvent.click(canvas.getByRole("row", { name: /^grilling/ }));
    await waitFor(() =>
      expect(canvas.getByRole("heading", { name: /^grilling/ })).toBeInTheDocument(),
    );
    await expect(boxes(tab)).toBe(before);
    await userEvent.click(canvas.getByRole("button", { name: "More need you" }));
    await waitFor(() =>
      expect(canvas.getByRole("button", { name: /^Needs you/ })).toHaveTextContent("10"),
    );
    await expect(boxes(tab)).toBe(before);
  },
};

/**
 * **Narrow: page-and-back.** The navigation is a picker, the list comes first, a skill opened
 * from it is drawn alone with a way back, and back lands on the list.
 */
export const Narrow: Story = {
  args: { width: 420, narrow: true },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await expect(canvas.queryByRole("navigation")).toBeNull();
    const picker = canvas.getByRole("button", { name: /Skill studio pages/ });
    await expect(picker).toHaveTextContent("Library");
    // Nothing scrolls sideways.
    const root = canvasElement.querySelector<HTMLElement>('[data-part="root"][data-narrow]');
    if (root === null) throw new Error("the frame is not narrow");
    await expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1);
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => picker, "the page picker");
    await userEvent.tab();
    await untilFocusWithin(() => canvas.getByRole("grid", { name: "Skills" }), "the list");
    await userEvent.keyboard("{ArrowDown}{Enter}");
    const back = await waitFor(() => canvas.getByRole("button", { name: "Back to Library" }));
    await untilFocused(() => back, "the way back");
    expectFocusInSight();
    await expect(canvas.queryByRole("grid", { name: "Skills" })).toBeNull();
    await userEvent.keyboard("{Enter}");
    await untilFocused(() => canvas.getByRole("region", { name: "Library" }), "the list again");
    await expect(canvas.getByRole("grid", { name: "Skills" })).toBeInTheDocument();
  },
};
