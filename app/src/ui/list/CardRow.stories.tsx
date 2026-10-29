// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A card row** (#3669): the sidebar Skills view's cards in a 260 px side view, as variant C of
 * `prototype/skills-sidebar` draws them: a plain one, one with a reason and its fix, and one whose
 * Use cannot be pressed and says why. Walked by the keyboard, held to axe and compared with its
 * baseline in Chromium and WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Badge } from "../badge/Badge";
import { Button } from "../button/Button";
import { Menu, MenuItem, MenuWords } from "../menu/Menu";
import { Panel } from "../panel/Panel";
import { List } from "./List";
import { ListCardRow } from "./CardRow";

/** What a skill's description says when it says more than two lines' worth. */
const LONG =
  "Interviews the person about a plan until every branch of it is settled, one question at a time, and writes down what was decided so that the next chat can pick it up where this one stopped.";

/** A skill's menu, as the sidebar's rows have it. */
const menuOf = (name: string, said: (what: string) => void) => () => (
  <Menu aria-label={name} onAction={(key) => said(`Chose ${String(key)} for ${name}`)}>
    <MenuItem id="open" textValue="Open in Skill studio">
      <MenuWords said="Open in Skill studio" />
    </MenuItem>
    <MenuItem id="file" textValue="Open the file">
      <MenuWords said="Open the file" />
    </MenuItem>
  </Menu>
);

/**
 * **The sidebar Skills view's cards** (variant C), in a 260 px side view, on the side panel's own
 * ground (#3774), so each theme's cards are pictured on what they stand on in the window.
 */
function SkillCards() {
  const [said, setSaid] = useState("Nothing run");
  return (
    <Panel variant="side">
      <div style={{ inlineSize: "260px", display: "grid", gap: "var(--space-3)" }}>
        <List
          aria-label="Skills"
          size="sm"
          onAction={(key) => setSaid(`Opened the studio at Library › ${String(key)}`)}
        >
          <ListCardRow
            id="grilling"
            textValue="grilling"
            name="grilling"
            mark={<Badge>persona</Badge>}
            actions={
              <Button size="sm" onPress={() => setSaid("Typed /grilling")}>
                Use
              </Button>
            }
            reason={{
              says: "Changed by a chat on 28 Oct. This chat keeps the version you accepted.",
              tone: "warning",
            }}
            fix={
              <Button size="sm" onPress={() => setSaid("Reviewing the change")}>
                Review the change
              </Button>
            }
            menu={menuOf("grilling", setSaid)}
          />
          <ListCardRow
            id="deploy"
            textValue="deploy"
            name="deploy"
            mark={<Badge>project</Badge>}
            actions={
              <Button size="sm" onPress={() => setSaid("Typed /deploy")}>
                Use
              </Button>
            }
            description={LONG}
            menu={menuOf("deploy", setSaid)}
          />
          <ListCardRow
            id="lint-rs"
            textValue="lint-rs"
            name="lint-rs"
            mark={<Badge>workspace</Badge>}
            actions={
              <Button size="sm" isDisabled tooltip="No chat is in front to use it in.">
                Use
              </Button>
            }
            reason={{ says: "Off in smart-ide", tone: "quiet" }}
            fix={
              <Button size="sm" onPress={() => setSaid("Turned lint-rs on")}>
                Turn on in smart-ide
              </Button>
            }
            menu={menuOf("lint-rs", setSaid)}
          />
        </List>
        <p role="status" style={{ margin: 0 }}>
          {said}
        </p>
      </div>
    </Panel>
  );
}

const meta = {
  component: SkillCards,
} satisfies Meta<typeof SkillCards>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **The Skills view's cards**: Tab lands on the first card, Enter runs it, → reaches Use, the ⋯
 * and the fix and ← comes back, ↓ moves to the next card, Shift+F10 opens a card's menu and
 * Escape gives the keyboard back. A three-line description shows two lines and an ellipsis, and
 * its tooltip holds the whole of it. Use that cannot be pressed keeps the keyboard and says why.
 * The picture is the first card under the keyboard.
 */
export const SkillsViewCards: Story = {
  play: async ({ canvas, userEvent }) => {
    const list = canvas.getByRole("grid", { name: "Skills" });
    const grilling = within(list).getByRole("row", { name: "grilling" });
    await userEvent.tab();
    await untilFocused(() => grilling, "Cards: Tab lands on the first card");
    await userEvent.keyboard("{Escape}{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent(
        "Opened the studio at Library › grilling",
      ),
    );
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(grilling).getByRole("button", { name: "Use" }),
      "Cards: Right reaches Use",
    );
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(grilling).getByRole("button", { name: "More for grilling" }),
      "Cards: Right reaches the ⋯",
    );
    await userEvent.keyboard("{ArrowRight}");
    const fix = within(grilling).getByRole("button", { name: "Review the change" });
    await untilFocused(() => fix, "Cards: Right reaches the fix");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent("Reviewing the change"),
    );
    await userEvent.keyboard("{ArrowLeft}{ArrowLeft}{ArrowLeft}");
    await untilFocused(() => grilling, "Cards: Left comes back to the card");
    // The description cut after two lines, and the whole of it in the tooltip.
    await userEvent.keyboard("{ArrowDown}");
    const deploy = within(list).getByRole("row", { name: "deploy" });
    await untilFocused(() => deploy, "Cards: Down moves to the next card");
    const summary = deploy.querySelector<HTMLElement>('[data-part="summary"]');
    if (summary === null) throw new Error("Cards: deploy has no summary");
    const line = Number.parseFloat(getComputedStyle(summary).lineHeight);
    await expect(summary.clientHeight, "Cards: two lines of the description").toBeLessThanOrEqual(
      Math.ceil(line * 2) + 1,
    );
    await expect(summary.scrollHeight, "Cards: the rest of the description is cut").toBeGreaterThan(
      summary.clientHeight,
    );
    await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent(LONG));
    // Its menu, from the keys, and back.
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "deploy" });
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: /Open in Skill studio/ }),
      "Cards: Shift+F10 opens the card's menu on its first row",
    );
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    await untilFocused(() => deploy, "Cards: Escape gives the keyboard back to the card");
    // Use that cannot be pressed keeps the keyboard and says why.
    await userEvent.keyboard("{ArrowDown}");
    const lint = within(list).getByRole("row", { name: "lint-rs" });
    await untilFocused(() => lint, "Cards: Down moves to the card whose Use is off");
    await userEvent.keyboard("{ArrowRight}");
    const use = within(lint).getByRole("button", { name: "Use" });
    await untilFocused(() => use, "Cards: Right reaches the Use that is off");
    await expect(use).toHaveAttribute("aria-disabled", "true");
    await waitFor(() =>
      expect(screen.getByRole("tooltip")).toHaveTextContent("No chat is in front to use it in."),
    );
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Reviewing the change");
    await userEvent.keyboard("{ArrowLeft}{ArrowUp}{ArrowUp}");
    await untilFocused(() => grilling, "Cards: Up walks back to the first card");
    expectFocusInSight();
  },
};
