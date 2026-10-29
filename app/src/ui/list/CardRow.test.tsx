// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A card row** (#3669): what a person reads, walks and runs on the sidebar Skills view's
 * cards, and the attributes a theme's parts are keyed on. The look is held by the stories
 * (`CardRow.stories.tsx`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { Button } from "../button/Button";
import { Menu, MenuItem, MenuWords } from "../menu/Menu";
import { drawnWith } from "../../cascade.testkit";
import { List } from "./List";
import { ListCardRow } from "./CardRow";

afterEach(cleanup);

const LONG =
  "Interviews the person about a plan until every branch of it is settled, one question at a time, and writes down what was decided so the next chat can pick it up.";

function Skills() {
  const [said, setSaid] = useState("Nothing yet");
  return (
    <>
      <List aria-label="Skills" size="sm" onAction={(key) => setSaid(`Opened ${String(key)}`)}>
        <ListCardRow
          id="grilling"
          textValue="grilling"
          name="grilling"
          mark={<span>persona</span>}
          actions={
            <Button size="sm" onPress={() => setSaid("Used grilling")}>
              Use
            </Button>
          }
          reason={{
            says: "Changed by a chat on 28 Oct. This chat keeps the version you accepted.",
            tone: "warning",
          }}
          fix={
            <Button size="sm" onPress={() => setSaid("Reviewing grilling")}>
              Review the change
            </Button>
          }
          menu={() => (
            <Menu aria-label="grilling" onAction={(key) => setSaid(`Chose ${String(key)}`)}>
              <MenuItem id="open" textValue="Open in Skill studio">
                <MenuWords said="Open in Skill studio" />
              </MenuItem>
            </Menu>
          )}
        />
        <ListCardRow id="deploy" textValue="deploy" name="deploy" description={LONG} />
        <ListCardRow
          id="gpu-profile"
          textValue="gpu-profile"
          name="gpu-profile"
          reason={{ says: "Claude Code can't take it", tone: "quiet" }}
        />
      </List>
      <p role="status">{said}</p>
    </>
  );
}

const row = (name: string) => screen.getByRole("row", { name });

describe("a card row", () => {
  it("is the list family's root in its card variant, its lines its parts", () => {
    render(<Skills />);
    const grilling = row("grilling");
    expect(grilling).toHaveAttribute("data-scope", "list");
    expect(grilling).toHaveAttribute("data-part", "root");
    expect(grilling).toHaveAttribute("data-variant", "card");
    const reason = within(grilling).getByText(/Changed by a chat/);
    expect(reason).toHaveAttribute("data-part", "summary");
    expect(reason).toHaveAttribute("data-tone", "warning");
    const fix = within(grilling).getByRole("button", { name: "Review the change" });
    expect(fix.closest('[data-part="actions"]')).not.toBeNull();
    expect(within(row("deploy")).getByText(LONG)).not.toHaveAttribute("data-tone");
    expect(within(row("gpu-profile")).getByText("Claude Code can't take it")).toHaveAttribute(
      "data-tone",
      "quiet",
    );
  });

  it("says the whole of its summary in its tooltip", async () => {
    render(<Skills />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    expect(row("deploy")).toHaveFocus();
    expect(screen.getByRole("tooltip")).toHaveTextContent(LONG);
  });

  it("runs on Enter, reaches Use, its ⋯ and its fix with →, and comes back with ←", async () => {
    render(<Skills />);
    await userEvent.tab();
    expect(row("grilling")).toHaveFocus();
    await userEvent.keyboard("{Escape}{Enter}");
    expect(screen.getByRole("status")).toHaveTextContent("Opened grilling");
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Use" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "More for grilling" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    const fix = screen.getByRole("button", { name: "Review the change" });
    expect(fix).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("status")).toHaveTextContent("Reviewing grilling");
    await userEvent.keyboard("{ArrowLeft}{ArrowLeft}{ArrowLeft}");
    expect(row("grilling")).toHaveFocus();
  });

  it("opens the row's own menu from its ⋯, and from Shift+F10", async () => {
    render(<Skills />);
    await userEvent.click(screen.getByRole("button", { name: "More for grilling" }));
    expect(await screen.findByRole("menu", { name: "grilling" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    act(() => row("grilling").focus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    expect(await screen.findByRole("menu", { name: "grilling" })).toBeInTheDocument();
  });

  it("draws no ⋯ where it has no menu", () => {
    render(<Skills />);
    expect(within(row("deploy")).queryByRole("button")).toBeNull();
  });

  it("changes only a fill or a ring between its states, never its room", () => {
    const card = document.createElement("div");
    card.setAttribute("data-scope", "list");
    card.setAttribute("data-part", "root");
    card.setAttribute("data-variant", "card");
    card.setAttribute("data-size", "sm");
    const room = (state: { hover?: boolean; focusVisible?: boolean } = {}) =>
      ["padding-block", "padding-inline", "min-block-size", "row-gap"].map((one) =>
        drawnWith(card, one, state),
      );
    const atRest = room();
    expect(drawnWith(card, "background-color")).toBe("var(--surface-raised)");
    expect(drawnWith(card, "background-color", { hover: true })).toBe("var(--surface-hover)");
    card.setAttribute("data-selected", "true");
    expect(drawnWith(card, "background-color", { hover: true })).toBe("var(--list-selected)");
    expect(room({ hover: true })).toEqual(atRest);
    expect(room({ focusVisible: true })).toEqual(atRest);
  });
});
