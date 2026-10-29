// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What folds in a list** (#3670): a group whose name folds its rows, and a run folded to one
 * quiet line, as a person walks and presses them. The look is held by the stories
 * (`Fold.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { Button } from "../button/Button";
import { List, ListRow } from "./List";
import { ListFoldSection, ListRun } from "./Fold";
import { RowLabel } from "./Row";

afterEach(cleanup);

const BUILT_IN = ["browser", "handoff", "secrets"];

function Groups({
  onAction = () => undefined,
  standsOut,
  onFoldedChange,
}: {
  onAction?: (key: string) => void;
  standsOut?: (name: string) => boolean;
  onFoldedChange?: (folded: boolean) => void;
}) {
  const [outFolded, setOutFolded] = useState(true);
  return (
    <List aria-label="Skills" size="sm" onAction={(key) => onAction(String(key))}>
      <ListFoldSection id="needs" title="Needs you" count={1} tone="waiting">
        <ListRow id="grilling" textValue="grilling">
          <RowLabel>grilling</RowLabel>
        </ListRow>
      </ListFoldSection>
      <ListFoldSection id="in" title="In this chat" count={4}>
        <ListRow id="deploy" textValue="deploy">
          <RowLabel>deploy</RowLabel>
        </ListRow>
        <ListRun
          id="built-in"
          items={BUILT_IN}
          standsOut={standsOut}
          says={(count) => `${count} built-in skills`}
          closing={{
            says: "They ship with purlis.",
            children: (
              <Button size="sm" onPress={() => onAction("copy")}>
                Make my own copy…
              </Button>
            ),
          }}
        >
          {(name) => (
            <ListRow key={name} id={name} textValue={name}>
              <RowLabel>{name}</RowLabel>
            </ListRow>
          )}
        </ListRun>
      </ListFoldSection>
      <ListFoldSection
        id="out"
        title="Left out"
        count={1}
        folded={outFolded}
        onFoldedChange={(folded) => {
          setOutFolded(folded);
          onFoldedChange?.(folded);
        }}
      >
        <ListRow id="lint-rs" textValue="lint-rs">
          <RowLabel>lint-rs</RowLabel>
        </ListRow>
      </ListFoldSection>
    </List>
  );
}

/** The cell a fold's row holds, which says whether it is open. */
const cellOf = (one: HTMLElement) => within(one).getByRole("gridcell");

const row = (name: string) => screen.getByRole("row", { name });
const noRow = (name: string) => screen.queryByRole("row", { name });

describe("a section that folds", () => {
  it("is a row the keyboard reaches, says whether it is open, and carries its count and tone", async () => {
    render(<Groups />);
    await userEvent.tab();
    const needs = row("Needs you, 1");
    expect(needs).toHaveFocus();
    expect(cellOf(needs)).toHaveAttribute("aria-expanded", "true");
    expect(needs).toHaveAttribute("data-part", "header");
    expect(needs).toHaveAttribute("data-tone", "waiting");
    expect(within(needs).getByText("1")).toHaveAttribute("data-part", "count");
    // The group is named by its name alone.
    expect(screen.getAllByRole("rowgroup").map((one) => one.getAttribute("aria-label"))).toEqual([
      "Needs you",
      "In this chat",
      "Left out",
    ]);
    // Its twist is the tree's, a mark and no button.
    const twist = needs.querySelector('[data-scope="tree"][data-part="twist"]');
    expect(twist).not.toBeNull();
    expect(twist?.tagName).toBe("SPAN");
  });

  it("folds and unfolds on Enter, Space, ← and →, and runs nothing of the list's", async () => {
    const onAction = vi.fn();
    render(<Groups onAction={onAction} />);
    await userEvent.tab();
    await userEvent.keyboard("{Enter}");
    expect(cellOf(row("Needs you, 1"))).toHaveAttribute("aria-expanded", "false");
    expect(noRow("grilling")).toBeNull();
    await userEvent.keyboard(" ");
    expect(row("grilling")).toBeInTheDocument();
    await userEvent.keyboard("{ArrowLeft}");
    expect(cellOf(row("Needs you, 1"))).toHaveAttribute("aria-expanded", "false");
    expect(row("Needs you, 1")).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(cellOf(row("Needs you, 1"))).toHaveAttribute("aria-expanded", "true");
    expect(row("Needs you, 1")).toHaveFocus();
    expect(onAction).not.toHaveBeenCalled();
    // A row of its own still runs the list's action.
    await userEvent.keyboard("{ArrowDown}{Enter}");
    expect(onAction).toHaveBeenCalledWith("grilling");
  });

  it("lets a Space go on finding a row by its words", async () => {
    render(<Groups />);
    await userEvent.tab();
    await userEvent.keyboard("i");
    expect(row("In this chat, 4")).toHaveFocus();
    await userEvent.keyboard(" ");
    expect(cellOf(row("In this chat, 4"))).toHaveAttribute("aria-expanded", "true");
  });

  it("draws a name that is data as it is written", () => {
    render(
      <List aria-label="Files">
        <ListFoldSection id="day" title="2026-10-29" named>
          <ListRow id="a" textValue="a">
            <RowLabel>a</RowLabel>
          </ListRow>
        </ListFoldSection>
      </List>,
    );
    expect(row("2026-10-29")).toHaveAttribute("data-named");
  });

  it("starts folded, and tells its owner, who holds it", async () => {
    const onFoldedChange = vi.fn();
    render(<Groups onFoldedChange={onFoldedChange} />);
    expect(cellOf(row("Left out, 1"))).toHaveAttribute("aria-expanded", "false");
    expect(noRow("lint-rs")).toBeNull();
    await userEvent.click(row("Left out, 1"));
    expect(onFoldedChange).toHaveBeenCalledWith(false);
    expect(row("lint-rs")).toBeInTheDocument();
  });
});

describe("a run folded to one quiet line", () => {
  it("draws its rows as one line, and unfolds them in place with its closing line", async () => {
    render(<Groups />);
    const line = row("3 built-in skills");
    expect(line).toHaveAttribute("data-part", "run");
    expect(cellOf(line)).toHaveAttribute("aria-expanded", "false");
    expect(noRow("browser")).toBeNull();
    await userEvent.click(line);
    const rows = screen.getAllByRole("row").map((one) => one.getAttribute("aria-label"));
    expect(rows.slice(rows.indexOf("deploy"))).toEqual([
      "deploy",
      "3 built-in skills",
      "browser",
      "handoff",
      "secrets",
      "They ship with purlis.",
      "Left out, 1",
    ]);
    expect(cellOf(row("3 built-in skills"))).toHaveAttribute("aria-expanded", "true");
  });

  it("reaches its closing line's button with →, which runs only that button", async () => {
    const onAction = vi.fn();
    render(<Groups onAction={onAction} />);
    await userEvent.click(row("3 built-in skills"));
    row("They ship with purlis.").focus();
    await userEvent.keyboard("{Enter}");
    expect(onAction).not.toHaveBeenCalled();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Make my own copy…" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(onAction).toHaveBeenCalledWith("copy");
  });

  it("draws a row that must show on its own, outside the fold", () => {
    render(<Groups standsOut={(name) => name === "secrets"} />);
    expect(row("secrets")).toBeInTheDocument();
    expect(cellOf(row("2 built-in skills"))).toHaveAttribute("aria-expanded", "false");
    expect(noRow("browser")).toBeNull();
  });

  it("draws no line when every row stands out", () => {
    render(<Groups standsOut={() => true} />);
    expect(screen.queryByRole("row", { name: /built-in skills/ })).toBeNull();
    expect(row("browser")).toBeInTheDocument();
  });
});

describe("the fold's look", () => {
  it("uses the tree's twist and draws no chevron of its own", () => {
    const fold = readFileSync(join(process.cwd(), "src/ui/list/Fold.tsx"), "utf8");
    expect(fold).not.toMatch(/Chevron|▸|▾/);
    expect(fold).toMatch(/import \{ TwistMark \} from "\.\.\/tree\/Tree"/);
  });
});
