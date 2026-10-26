// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The list family's wrapper** (#2911): what a person walks, chooses and runs, and the
 * attributes a theme's parts are keyed on (ADR 0103). The look itself is held by the stories, in
 * a real browser (`List.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { Button } from "../button/Button";
import { Menu, MenuItem, MenuWords } from "../menu/Menu";
import { TOOLTIP_DELAY } from "../tooltip/Tooltip";
import { drawnWith } from "../../cascade.testkit";
import {
  List,
  ListBox,
  ListOption,
  ListRow,
  ListRowSection,
  ListSection,
  type Selection,
} from "./List";
import { RowDescription, RowLabel, RowLead, RowTrail } from "./Row";
import { FILTERED_FROM, ListFilter, noMatchSaid, useListFilter } from "./ListFilter";
import { AimedList, AimedOption } from "./AimedList";

afterEach(cleanup);

const PERSONAS = ["steward", "web-dev", "reviewer"];

function Personas({ onAction }: { onAction?: (key: string) => void }) {
  const [chosen, setChosen] = useState<Selection>(new Set(["steward"]));
  return (
    <>
      <ListBox
        aria-label="Personas"
        size="lg"
        selectionMode="single"
        disallowEmptySelection
        selectedKeys={chosen}
        onSelectionChange={setChosen}
        onAction={onAction === undefined ? undefined : (key) => onAction(String(key))}
      >
        {PERSONAS.map((name) => (
          <ListOption key={name} id={name} textValue={name}>
            <RowLead>{name.slice(0, 2)}</RowLead>
            <RowLabel>{name}</RowLabel>
            <RowDescription>4 chats</RowDescription>
          </ListOption>
        ))}
      </ListBox>
      <p role="status">{[...(chosen as Set<string>)].join(",")}</p>
    </>
  );
}

describe("a list to choose from", () => {
  it("draws its rows as the family's parts, at the list's size", () => {
    render(<Personas />);
    const list = screen.getByRole("listbox", { name: "Personas" });
    expect(list).toHaveAttribute("data-scope", "list");
    expect(list).toHaveAttribute("data-part", "body");
    const rows = within(list).getAllByRole("option");
    expect(rows).toHaveLength(3);
    for (const row of rows) {
      expect(row).toHaveAttribute("data-scope", "list");
      expect(row).toHaveAttribute("data-part", "root");
      expect(row).toHaveAttribute("data-size", "lg");
    }
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[0]).toHaveAttribute("data-selected", "true");
    const label = within(rows[0]).getByText("steward");
    expect(label).toHaveAttribute("data-scope", "list");
    expect(label).toHaveAttribute("data-part", "label");
    expect(within(rows[0]).getByText("4 chats")).toHaveAttribute("data-part", "description");
    expect(within(rows[0]).getByText("st")).toHaveAttribute("data-part", "lead");
    // Named by its words alone, its mark left out, and described by its one line (#2911).
    expect(screen.getByRole("option", { name: "steward" })).toHaveAccessibleDescription("4 chats");
  });

  it("keeps one Tab stop, and the arrows choose and stop at the ends", async () => {
    render(<Personas />);
    await userEvent.tab();
    const rows = screen.getAllByRole("option");
    expect(rows[0]).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(rows[1]).toHaveFocus();
    expect(screen.getByRole("status")).toHaveTextContent("web-dev");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    expect(rows[2]).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(rows[0]).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(rows[2]).toHaveFocus();
    // Typing finds a row by its words.
    await userEvent.keyboard("w");
    expect(rows[1]).toHaveFocus();
    await userEvent.tab();
    expect(document.body).toHaveFocus();
  });

  it("runs what is picked from a list that opens it", async () => {
    const ran = vi.fn();
    render(<Personas onAction={ran} />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}{Enter}");
    expect(ran).toHaveBeenCalledWith("web-dev");
  });

  it("groups its options under a name, the section's header", () => {
    render(
      <ListBox aria-label="Hits">
        <ListSection title="a.rs" aria-label="a.rs, src">
          <ListOption id="1" textValue="1">
            <RowLabel>one</RowLabel>
          </ListOption>
        </ListSection>
      </ListBox>,
    );
    const group = screen.getByRole("group");
    expect(within(group).getByText("a.rs")).toHaveAttribute("data-part", "header");
    expect(within(group).getByRole("option")).toHaveTextContent("one");
  });

  it("says what it has none of in its empty line", () => {
    render(<ListBox aria-label="Nothing" items={[]} empty="No theme yet." children={() => null} />);
    expect(screen.getByRole("listbox", { name: "Nothing" })).toHaveTextContent("No theme yet.");
  });
});

function Todos({ ran }: { ran: (key: string) => void }) {
  return (
    <List aria-label="Todos" size="sm" onAction={(key) => ran(String(key))}>
      <ListRowSection title="Today">
        <ListRow id="write" textValue="Write the spec">
          <RowLabel>Write the spec</RowLabel>
          <RowTrail>
            <Button size="sm" onPress={() => ran("done:write")}>
              Done
            </Button>
          </RowTrail>
        </ListRow>
        <ListRow id="ship" textValue="Ship it">
          <RowLabel>Ship it</RowLabel>
        </ListRow>
      </ListRowSection>
    </List>
  );
}

describe("a list of rows that act", () => {
  it("runs a row by Enter, and reaches the row's own button with →", async () => {
    const ran = vi.fn();
    render(<Todos ran={ran} />);
    const list = screen.getByRole("grid", { name: "Todos" });
    expect(list).toHaveAttribute("data-scope", "list");
    // A group's name is a row of its own, which the arrows pass over.
    const rows = within(list)
      .getAllByRole("row")
      .filter((row) => row.getAttribute("data-part") === "root");
    expect(within(list).getByText("Today").closest('[data-part="header"]')).not.toBeNull();
    expect(rows[0]).toHaveAttribute("data-size", "sm");
    await userEvent.tab();
    expect(rows[0]).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(ran).toHaveBeenLastCalledWith("write");
    await userEvent.keyboard("{ArrowRight}");
    expect(within(rows[0]).getByRole("button", { name: "Done" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(ran).toHaveBeenLastCalledWith("done:write");
    await userEvent.keyboard("{ArrowDown}");
    expect(rows[1]).toHaveFocus();
  });
});

describe("a long list's filter (R164)", () => {
  function Filtered({ names }: { names: string[] }) {
    const { query, setQuery, shown, filtered } = useListFilter(names, (name) => name);
    return (
      <>
        {filtered && (
          <ListFilter
            noun={{ one: "host", many: "hosts" }}
            total={names.length}
            query={query}
            onQuery={setQuery}
          />
        )}
        <p>
          {shown.length === 0
            ? noMatchSaid({ one: "host", many: "hosts" }, query)
            : shown.join(",")}
        </p>
      </>
    );
  }

  it("is drawn from about seven items, with the total in its placeholder", async () => {
    const few = Array.from({ length: FILTERED_FROM - 1 }, (_, n) => `host-${n}.dev`);
    const { unmount } = render(<Filtered names={few} />);
    expect(screen.queryByRole("searchbox")).toBeNull();
    unmount();
    const many = Array.from({ length: 19 }, (_, n) => `host-${n}.dev`);
    render(<Filtered names={many} />);
    const box = screen.getByRole("searchbox", { name: "Filter hosts" });
    expect(box).toHaveAttribute("placeholder", "Filter 19 hosts");
    await userEvent.type(box, "HOST-1");
    expect(screen.getByText(/host-1\.dev/)).toBeInTheDocument();
    await userEvent.clear(box);
    await userEvent.type(box, "zz");
    expect(screen.getByText("No host is called “zz”.")).toBeInTheDocument();
  });
});

describe("a list a field aims at", () => {
  it("is named, says the aimed row, and runs a row on a press", async () => {
    const ran = vi.fn();
    render(
      <AimedList id="rows" aria-label="Actions">
        <AimedOption id="a" aimed onPress={() => ran("a")}>
          <RowLabel>Open a chat</RowLabel>
        </AimedOption>
        <AimedOption id="b" aimed={false} unavailable onPress={() => ran("b")}>
          <RowLabel>End it</RowLabel>
        </AimedOption>
      </AimedList>,
    );
    const list = screen.getByRole("listbox", { name: "Actions" });
    expect(list).toHaveAttribute("data-scope", "list");
    const [a, b] = within(list).getAllByRole("option");
    expect(a).toHaveAttribute("aria-selected", "true");
    expect(a).toHaveAttribute("data-aimed");
    expect(b).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(a);
    expect(ran).toHaveBeenCalledWith("a");
  });
});

describe("a row's own tooltip, menu and keys (ADR 0104, 2026-10-24)", () => {
  function Tipped() {
    return (
      <>
        <List aria-label="Memories" aria-keyshortcuts="Shift+Enter">
          <ListRow
            id="a"
            textValue="Deploys freeze on Fridays"
            tip="2026-10-10 09:12"
            menu={() => (
              <Menu aria-label="Deploys freeze on Fridays">
                <MenuItem id="open" textValue="Open">
                  <MenuWords said="Open" />
                </MenuItem>
              </Menu>
            )}
          >
            <RowLabel>Deploys freeze on Fridays</RowLabel>
          </ListRow>
          <ListRow id="b" textValue="Read the audit">
            <RowLabel>Read the audit</RowLabel>
          </ListRow>
        </List>
        <button type="button">After</button>
      </>
    );
  }
  const row = () => screen.getByRole("row", { name: "Deploys freeze on Fridays" });

  it("says the keys the list answers to", () => {
    render(<Tipped />);
    expect(screen.getByRole("grid", { name: "Memories" })).toHaveAttribute(
      "aria-keyshortcuts",
      "Shift+Enter",
    );
  });

  it("shows its tooltip at once from the keyboard, as its description, and Esc takes it down", async () => {
    render(<Tipped />);
    await userEvent.tab();
    expect(row()).toHaveFocus();
    expect(screen.getByRole("tooltip")).toHaveTextContent("2026-10-10 09:12");
    expect(row()).toHaveAccessibleDescription("2026-10-10 09:12");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("tooltip")).toBeNull();
    // Esc was the tooltip's alone: the keyboard stays on the row.
    expect(row()).toHaveFocus();
    expect(row()).not.toHaveAttribute("aria-describedby");
  });

  it("shows its tooltip under the pointer only after the window's delay", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      render(<Tipped />);
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      await user.hover(row());
      expect(screen.queryByRole("tooltip")).toBeNull();
      await act(async () => {
        vi.advanceTimersByTime(TOOLTIP_DELAY + 10);
      });
      expect(screen.getByRole("tooltip")).toHaveTextContent("2026-10-10 09:12");
      await user.unhover(row());
      expect(screen.queryByRole("tooltip")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("stands by the pointer, not the whole row, so a row across the window leaves it room (#3328)", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      render(<Tipped />);
      // A row from the rail to the window's edge: the left panel closed.
      vi.spyOn(row(), "getBoundingClientRect").mockReturnValue(
        DOMRect.fromRect({ x: 110, y: 400, width: 914, height: 24 }),
      );
      fireEvent.pointerEnter(row(), { clientX: 200, pointerType: "mouse" });
      fireEvent.pointerMove(row(), { clientX: 300, pointerType: "mouse" });
      await act(async () => {
        vi.advanceTimersByTime(TOOLTIP_DELAY + 10);
      });
      expect(screen.getByRole("tooltip")).toHaveTextContent("2026-10-10 09:12");
      const anchor = row().querySelector<HTMLElement>('[data-part="tip-anchor"]');
      // Where the pointer rested as the delay ran out, a line with no width.
      expect(anchor?.style.left).toBe("190px");
      expect(anchor?.style.width).toBe("0px");
    } finally {
      vi.useRealTimers();
    }
  });

  it("opens its menu on a right-click, and on the menu key on its first row", async () => {
    render(<Tipped />);
    fireEvent.contextMenu(row());
    expect(
      await screen.findByRole("menu", { name: "Deploys freeze on Fridays" }),
    ).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    act(() => row().focus());
    await userEvent.keyboard("{ContextMenu}");
    const menu = await screen.findByRole("menu", { name: "Deploys freeze on Fridays" });
    await waitFor(() => expect(within(menu).getByRole("menuitem", { name: /Open/ })).toHaveFocus());
  });

  it("finds a row by typing its first letters", async () => {
    render(<Tipped />);
    await userEvent.tab();
    await userEvent.keyboard("{Escape}r");
    expect(screen.getByRole("row", { name: "Read the audit" })).toHaveFocus();
  });
});

describe("a row's look as the cascade draws it", () => {
  it("keeps a chosen row's wash under the pointer, and draws the pill on one not chosen", () => {
    const row = document.createElement("div");
    row.setAttribute("data-scope", "list");
    row.setAttribute("data-part", "root");
    expect(drawnWith(row, "background-color", { hover: true })).toBe("var(--surface-hover)");
    row.setAttribute("data-selected", "true");
    expect(drawnWith(row, "background-color", { hover: true })).toBe("var(--list-selected)");
    expect(drawnWith(row, "color", { hover: true })).toBe("var(--text-primary)");
  });

  it("draws a chosen row's edge on its start in a right-to-left window too", () => {
    const css = readFileSync(join(process.cwd(), "src/ui/list/list.css"), "utf8");
    expect(css).toMatch(
      /\[data-scope="list"\]\[data-part="root"\]\[data-selected\]:dir\(rtl\) \{\s*box-shadow: inset calc\(-1 \* var\(--focus-ring-width\)\) 0 0 var\(--list-selected-edge\);/,
    );
  });
});
