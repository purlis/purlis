// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tree family's wrapper** (#2911): what a person walks, opens and runs, and the attributes
 * a theme's parts are keyed on (ADR 0103). The look itself is held by the stories, in a real
 * browser (`Tree.stories.tsx`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { Menu, MenuItem, MenuWords } from "../menu/Menu";
import { drawnWith } from "../../cascade.testkit";
import { RowLabel, RowLead, RowTrail } from "../list/Row";
import { Tree, TreeLine, TreeRow, TreeSection, type Key } from "./Tree";

afterEach(cleanup);

/** A branch's files: `src` holds `main.ts` and `lib`, which holds `util.ts`. */
function Files({
  fixed = false,
  twist,
  onAction,
  open: first = ["src"],
}: {
  fixed?: boolean;
  twist?: "lead" | "trail";
  onAction?: (key: string) => void;
  open?: string[];
}) {
  const [open, setOpen] = useState<Set<Key>>(new Set(fixed ? ["src", "lib"] : first));
  return (
    <>
      <Tree
        aria-label="Files of main"
        size="sm"
        fixed={fixed}
        twist={twist}
        expandedKeys={open}
        onExpandedChange={setOpen}
        onAction={onAction === undefined ? undefined : (key) => onAction(String(key))}
      >
        <TreeRow
          id="src"
          textValue="src"
          content={<RowLabel>src</RowLabel>}
          menu={() => (
            <Menu aria-label="src">
              <MenuItem id="copy" textValue="Copy path">
                <MenuWords said="Copy path" />
              </MenuItem>
            </Menu>
          )}
        >
          <TreeRow id="main" textValue="main.ts" content={<RowLabel>main.ts</RowLabel>} />
          <TreeRow id="lib" textValue="lib" content={<RowLabel>lib</RowLabel>}>
            <TreeRow id="util" textValue="util.ts" content={<RowLabel>util.ts</RowLabel>} />
          </TreeRow>
        </TreeRow>
        <TreeRow
          id="readme"
          textValue="README.md"
          content={
            <>
              <RowLead>M</RowLead>
              <RowLabel>README.md</RowLabel>
              <RowTrail>2</RowTrail>
            </>
          }
        />
      </Tree>
      <p role="status">{[...open].join(",")}</p>
    </>
  );
}

const row = (name: string) => screen.getByRole("row", { name });

describe("a tree", () => {
  it("is a tree of rows, each at its level and its place among its siblings", () => {
    render(<Files />);
    const tree = screen.getByRole("treegrid", { name: "Files of main" });
    expect(tree).toHaveAttribute("data-scope", "tree");
    expect(tree).toHaveAttribute("data-part", "body");
    expect(tree).toHaveAttribute("data-twist", "lead");
    expect(row("src")).toHaveAttribute("aria-level", "1");
    expect(row("src")).toHaveAttribute("aria-expanded", "true");
    expect(row("main.ts")).toHaveAttribute("aria-level", "2");
    expect(row("main.ts")).toHaveAttribute("aria-posinset", "1");
    expect(row("lib")).toHaveAttribute("aria-setsize", "2");
    expect(row("lib")).toHaveAttribute("aria-expanded", "false");
    expect(row("README.md")).not.toHaveAttribute("aria-expanded");
    expect(row("src")).toHaveAttribute("data-scope", "tree");
    expect(row("src")).toHaveAttribute("data-part", "root");
    expect(row("src")).toHaveAttribute("data-size", "sm");
    // A closed folder's rows are not drawn.
    expect(screen.queryByRole("row", { name: "util.ts" })).toBeNull();
  });

  it("draws a row's slots in the tree's own scope", () => {
    render(<Files />);
    const readme = row("README.md");
    for (const part of ["lead", "label", "trail"])
      expect(readme.querySelector(`[data-part="${part}"]`)).toHaveAttribute("data-scope", "tree");
  });

  it("draws a twist only on a row with rows under it, named for what it does, never a Tab stop", async () => {
    render(<Files />);
    const twist = within(row("lib")).getByRole("button", { name: /Expand/ });
    expect(twist).toHaveAttribute("data-part", "twist");
    expect(twist).toHaveAttribute("tabindex", "-1");
    expect(within(row("src")).getByRole("button", { name: /Collapse/ })).toBeInTheDocument();
    expect(within(row("README.md")).queryByRole("button")).toBeNull();
    await userEvent.click(twist);
    expect(screen.getByRole("status")).toHaveTextContent("src,lib");
    expect(row("util.ts")).toHaveAttribute("aria-level", "3");
  });

  it("walks as the WAI-ARIA tree: down, into an open row, back out to the parent, closed and open", async () => {
    render(<Files />);
    await userEvent.tab();
    expect(row("src")).toHaveFocus();
    // → on an open row moves to its first child, never onto its twist.
    await userEvent.keyboard("{ArrowRight}");
    expect(row("main.ts")).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(row("lib")).toHaveFocus();
    // → on a closed one opens it, and the next → moves into it.
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("status")).toHaveTextContent("src,lib");
    expect(row("lib")).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(row("util.ts")).toHaveFocus();
    // ← on a row with nothing under it moves to its parent; on an open one closes it.
    await userEvent.keyboard("{ArrowLeft}");
    expect(row("lib")).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(screen.getByRole("status")).toHaveTextContent(/^src$/);
    await userEvent.keyboard("{ArrowLeft}");
    expect(row("src")).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(row("README.md")).toHaveFocus();
    // ← at the top, on a row with nothing under it, goes nowhere: not into the row's marks.
    await userEvent.keyboard("{ArrowLeft}");
    expect(row("README.md")).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(row("src")).toHaveFocus();
    // And the tree is one stop: Tab leaves it.
    await userEvent.tab();
    expect(screen.getByRole("treegrid")).not.toContainElement(
      document.activeElement as HTMLElement,
    );
  });

  it("finds a row by typing its first letters", async () => {
    render(<Files />);
    await userEvent.tab();
    await userEvent.keyboard("RE");
    expect(row("README.md")).toHaveFocus();
  });

  it("runs a row's action on Enter", async () => {
    const ran: string[] = [];
    render(<Files onAction={(key) => ran.push(key)} />);
    await userEvent.tab();
    await userEvent.keyboard("{End}{Enter}");
    expect(ran).toEqual(["readme"]);
  });

  it("opens a row's menu on a right-click and on Shift+F10", async () => {
    render(<Files />);
    fireEvent.contextMenu(row("src"));
    expect(await screen.findByRole("menu", { name: "src" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    act(() => row("src").focus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    expect(await screen.findByRole("menu", { name: "src" })).toBeInTheDocument();
  });

  it("draws its twists at the row's end where they trail, and keeps no step for one", () => {
    render(<Files twist="trail" />);
    expect(screen.getByRole("treegrid")).toHaveAttribute("data-twist", "trail");
    const cells = [...row("src").querySelectorAll("[data-part]")].map((one) =>
      one.getAttribute("data-part"),
    );
    expect(cells).toEqual(["label", "twist"]);
  });
});

describe("a tree whose rows never fold", () => {
  it("draws every row open, with no twist and nothing to press", () => {
    render(<Files fixed />);
    expect(screen.getByRole("treegrid")).toHaveAttribute("data-twist", "none");
    expect(row("lib")).toHaveAttribute("aria-expanded", "true");
    expect(row("util.ts")).toBeInTheDocument();
    expect(screen.getByRole("treegrid").querySelectorAll("button")).toHaveLength(0);
  });

  it("moves from an open row to its parent on ←, and keeps every row open", async () => {
    render(<Files fixed />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowRight}{ArrowDown}");
    expect(row("lib")).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(row("src")).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}{Enter}");
    expect(row("src")).toHaveFocus();
    expect(screen.getByRole("status")).toHaveTextContent("src,lib");
    expect(row("util.ts")).toBeInTheDocument();
  });
});

describe("a tree's lines and groups", () => {
  it("draws a line that is not a row to act on in its place, read by the keyboard", async () => {
    render(
      <Tree aria-label="Chats" expandedKeys={["web"]}>
        <TreeSection title="This workspace">
          <TreeRow id="web" textValue="web" content={<RowLabel>web</RowLabel>}>
            <TreeLine id="web-reading" textValue="Reading…">
              <RowLabel>Reading…</RowLabel>
            </TreeLine>
          </TreeRow>
        </TreeSection>
      </Tree>,
    );
    const line = row("Reading…");
    expect(line).toHaveAttribute("data-line");
    expect(line).toHaveAttribute("aria-level", "2");
    expect(screen.getByText("This workspace").closest('[data-part="header"]')).toHaveAttribute(
      "data-scope",
      "tree",
    );
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    expect(line).toHaveFocus();
  });
});

describe("a row's look as the cascade draws it", () => {
  const element = () => {
    const one = document.createElement("div");
    one.setAttribute("data-scope", "tree");
    one.setAttribute("data-part", "root");
    return one;
  };

  it("keeps a chosen row's wash under the pointer, and draws the pill on one not chosen", () => {
    const one = element();
    expect(drawnWith(one, "background-color", { hover: true })).toBe("var(--surface-hover)");
    one.setAttribute("data-selected", "true");
    expect(drawnWith(one, "background-color", { hover: true })).toBe("var(--list-selected)");
    expect(drawnWith(one, "color", { hover: true })).toBe("var(--text-primary)");
  });

  it("rests in the secondary tier, and a line in the quiet one", () => {
    const one = element();
    expect(drawnWith(one, "color")).toBe("var(--text-secondary)");
    one.setAttribute("data-line", "");
    expect(drawnWith(one, "color")).toBe("var(--text-quiet)");
  });
});
