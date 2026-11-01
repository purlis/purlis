// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { StatusBar, placed, type StatusContribution } from "./StatusBar";
import { StatusCell, StatusItem } from "./StatusItem";
import { reordered } from "../../statusBarPrefs";

/**
 * **The status bar's contribution model** (N28, the operator's #2814·5): an item sits left or
 * right by priority, the person can hide any item, a press runs a command, and purlis's own
 * items and an extension's share the model.
 */

afterEach(cleanup);

const own = (
  id: string,
  side: "left" | "right",
  priority: number,
  extra: Partial<StatusContribution> = {},
): StatusContribution => ({
  id,
  name: id,
  side,
  priority,
  item: <StatusCell data-testid={id}>{id}</StatusCell>,
  ...extra,
});

describe("where an item sits", () => {
  it("is its side, highest priority first, ties in the order given", () => {
    const items = [
      own("b", "left", 10),
      own("a", "left", 90),
      own("x", "right", 5),
      own("c", "left", 10),
      own("y", "right", 50),
    ];
    expect(placed(items, "left").map((one) => one.id)).toEqual(["a", "b", "c"]);
    expect(placed(items, "right").map((one) => one.id)).toEqual(["y", "x"]);
  });

  it("leaves out what the person hid, unless the item cannot be hidden", () => {
    const items = [own("a", "left", 2), own("b", "left", 1, { hideable: false })];
    expect(placed(items, "left", ["a", "b"]).map((one) => one.id)).toEqual(["b"]);
  });

  it("follows the order the person gave it, on each end", () => {
    const items = [own("a", "left", 3), own("b", "left", 2), own("c", "left", 1)];
    expect(placed(items, "left", [], ["c", "a", "b"]).map((one) => one.id)).toEqual([
      "c",
      "a",
      "b",
    ]);
  });

  it("keeps an item the order never named beside the one it follows by priority", () => {
    const items = [own("a", "left", 3), own("new", "left", 2), own("c", "left", 1)];
    expect(placed(items, "left", [], ["c", "a"]).map((one) => one.id)).toEqual(["c", "a", "new"]);
    const first = [own("new", "left", 9), own("a", "left", 3), own("c", "left", 1)];
    expect(placed(first, "left", [], ["c", "a"]).map((one) => one.id)).toEqual(["new", "c", "a"]);
  });

  it("is not moved by an order that names items absent today, or by an item hidden", () => {
    const items = [own("a", "left", 3), own("b", "left", 2), own("c", "left", 1)];
    expect(placed(items, "left", ["b"], ["c", "gone", "b", "a"]).map((one) => one.id)).toEqual([
      "c",
      "a",
    ]);
  });

  it("draws each end in its own part of the bar", () => {
    render(
      <StatusBar
        aria-label="Status"
        items={[own("right-one", "right", 1), own("left-one", "left", 1)]}
      />,
    );
    const bar = screen.getByRole("contentinfo", { name: "Status" });
    const [start, end] = [...bar.children];
    expect(start).toHaveAttribute("data-part", "start");
    expect(within(start as HTMLElement).getByTestId("left-one")).toBeInTheDocument();
    expect(end).toHaveAttribute("data-part", "end");
    expect(within(end as HTMLElement).getByTestId("right-one")).toBeInTheDocument();
  });
});

describe("a press", () => {
  it("runs the command an item given as data names, through the bar", async () => {
    const run = vi.fn();
    render(
      <StatusBar
        aria-label="Status"
        onCommand={run}
        items={[
          {
            id: "extension:prs/open",
            name: "PRs",
            side: "left",
            priority: 1,
            label: "PRs",
            value: "2",
            command: "prs.list",
          },
          { id: "extension:ci/red", name: "CI", side: "left", priority: 0, label: "CI" },
        ]}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "PRs: 2" }));
    expect(run).toHaveBeenCalledWith("prs.list");
    // One that names no command is its words, and no button.
    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(screen.getByText("CI")).toHaveAttribute("data-part", "label");
  });

  it("is an item's own command where purlis draws it", async () => {
    const open = vi.fn();
    render(
      <StatusBar
        aria-label="Status"
        items={[
          {
            id: "purlis.notices",
            name: "Notices",
            side: "right",
            priority: 1,
            item: <StatusItem onPress={open}>Notices</StatusItem>,
          },
        ]}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Notices" }));
    expect(open).toHaveBeenCalledTimes(1);
  });
});

describe("hiding an item", () => {
  it("is a check in the bar's menu, by a right click or the menu key on an item", async () => {
    const hide = vi.fn();
    render(
      <StatusBar
        aria-label="Status"
        hidden={["purlis.pin"]}
        onHide={hide}
        items={[
          own("purlis.todos", "left", 2),
          own("purlis.pin", "right", 1),
          own("purlis.project", "left", 3, { hideable: false }),
          {
            id: "purlis.notices",
            name: "Notices",
            side: "right",
            priority: 2,
            item: <StatusItem>Notices</StatusItem>,
          },
        ]}
      />,
    );
    // The hidden pin is gone from the bar and listed in the menu, unchecked.
    expect(screen.queryByTestId("purlis.pin")).toBeNull();
    fireEvent.keyDown(screen.getByRole("button", { name: "Notices" }), {
      key: "F10",
      shiftKey: true,
    });
    const menu = await screen.findByRole("menu");
    const rows = within(menu).getAllByRole("menuitemcheckbox");
    expect(rows.map((one) => one.textContent)).toEqual(["purlis.todos", "Notices", "purlis.pin"]);
    expect(rows[2]).toHaveAttribute("aria-checked", "false");
    await userEvent.click(rows[0]);
    expect(hide).toHaveBeenCalledWith("purlis.todos", true);
  });
});

describe("the keyboard and the bar's menu", () => {
  /** A bar whose hidden items it keeps itself, as the window keeps them. */
  function Kept({ items }: { items: StatusContribution[] }) {
    const [hidden, setHidden] = useState<string[]>([]);
    return (
      <StatusBar
        aria-label="Status"
        items={items}
        hidden={hidden}
        onHide={(id, hide) =>
          setHidden((was) => (hide ? [...was, id] : was.filter((one) => one !== id)))
        }
      />
    );
  }
  const notices = (): StatusContribution => ({
    id: "purlis.notices",
    name: "Notices",
    side: "right",
    priority: 30,
    hideable: false,
    item: <StatusItem>Notices</StatusItem>,
  });
  const doctor = (): StatusContribution => ({
    id: "purlis.doctor",
    name: "Doctor",
    side: "right",
    priority: 20,
    item: <StatusItem>Doctor</StatusItem>,
  });

  it("always opens from an item that stays, however many the person hides", async () => {
    render(<Kept items={[own("purlis.todos", "left", 2), doctor(), notices()]} />);
    for (const name of ["purlis.todos", "Doctor"]) {
      const item = screen.getByRole("button", { name: "Notices" });
      item.focus();
      await userEvent.keyboard("{Shift>}{F10}{/Shift}");
      const menu = await screen.findByRole("menu");
      // Notices cannot be hidden, so it is no row of the menu.
      expect(within(menu).queryByRole("menuitemcheckbox", { name: "Notices" })).toBeNull();
      await userEvent.click(within(menu).getByRole("menuitemcheckbox", { name }));
    }
    expect(screen.queryByRole("button", { name: "Doctor" })).toBeNull();
    expect(screen.queryByTestId("purlis.todos")).toBeNull();
    const left = screen.getByRole("button", { name: "Notices" });
    left.focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    expect(await screen.findByRole("menu")).toBeInTheDocument();
  });

  it("gives the keyboard back to the item it was opened from, after a check and after Escape", async () => {
    render(<Kept items={[own("purlis.todos", "left", 2), doctor(), notices()]} />);
    const from = screen.getByRole("button", { name: "Doctor" });
    from.focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    await screen.findByRole("menu");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(from).toHaveFocus();

    from.focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu");
    within(menu).getByRole("menuitemcheckbox", { name: "purlis.todos" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(screen.queryByTestId("purlis.todos")).toBeNull();
    expect(from).toHaveFocus();
  });

  it("gives it to the bar's first item where the one it was opened from was hidden", async () => {
    render(<Kept items={[doctor(), notices()]} />);
    screen.getByRole("button", { name: "Doctor" }).focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu");
    within(menu).getByRole("menuitemcheckbox", { name: "Doctor" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(screen.getByRole("button", { name: "Notices" })).toHaveFocus();
  });
});

describe("moving an item with the keyboard, from the bar's menu (#3030)", () => {
  /** A bar whose order it keeps itself, as the window keeps it. */
  function Arranged({ items }: { items: StatusContribution[] }) {
    const [order, setOrder] = useState<readonly string[]>([]);
    return (
      <StatusBar
        aria-label="Status"
        items={items}
        order={order}
        onHide={() => {}}
        onMove={(id, onto, shown) => setOrder((was) => reordered(was, shown, id, onto))}
      />
    );
  }
  const notices = (): StatusContribution => ({
    id: "purlis.notices",
    name: "Notices",
    side: "right",
    priority: 30,
    hideable: false,
    item: <StatusItem>Notices</StatusItem>,
  });
  const slots = () =>
    [...document.querySelectorAll<HTMLElement>('[data-part="slot"]')].map(
      (one) => one.dataset.item,
    );
  async function openMove(words: "Move left" | "Move right") {
    screen.getByRole("button", { name: "Notices" }).focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "Status: the items shown" });
    within(menu).getByRole("menuitem", { name: words }).focus();
    await userEvent.keyboard("{ArrowRight}");
    return screen.findByRole("menu", { name: words });
  }

  it("moves the item one place on its end, and keeps the menu up for the next move", async () => {
    render(
      <Arranged
        items={[own("a", "left", 3), own("b", "left", 2), own("c", "left", 1), notices()]}
      />,
    );
    const move = await openMove("Move left");
    const row = within(move).getByRole("menuitem", { name: "c" });
    row.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(slots()).toEqual(["a", "c", "b", "purlis.notices"]));
    expect(screen.getByRole("menu", { name: "Move left" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("c moved left of b");
    expect(within(move).getByRole("menuitem", { name: "c" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(slots()).toEqual(["c", "a", "b", "purlis.notices"]));
  });

  it("lists what the bar draws, Notices too, and no item it hid", async () => {
    render(
      <StatusBar
        aria-label="Status"
        hidden={["b"]}
        onHide={() => {}}
        onMove={() => {}}
        items={[own("a", "left", 3), own("b", "left", 2), notices()]}
      />,
    );
    const move = await openMove("Move right");
    expect(
      within(move)
        .getAllByRole("menuitem")
        .map((one) => one.textContent),
    ).toEqual(["a", "Notices"]);
  });

  it("says a row at its end's edge cannot run, and keeps it from doing anything", async () => {
    const onMove = vi.fn();
    render(
      <StatusBar
        aria-label="Status"
        onHide={() => {}}
        onMove={onMove}
        items={[own("a", "left", 3), own("b", "left", 2), notices()]}
      />,
    );
    const move = await openMove("Move left");
    const edge = within(move).getByRole("menuitem", { name: "a" });
    expect(edge).toHaveAttribute("aria-disabled", "true");
    expect(within(move).getByRole("menuitem", { name: "Notices" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(within(move).getByRole("menuitem", { name: "b" })).not.toHaveAttribute("aria-disabled");
    edge.focus();
    await userEvent.keyboard("{Enter}");
    expect(onMove).not.toHaveBeenCalled();
  });

  it("offers no move where the bar's owner keeps no order", async () => {
    render(<StatusBar aria-label="Status" onHide={() => {}} items={[own("a", "left", 1), notices()]} />);
    screen.getByRole("button", { name: "Notices" }).focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu");
    expect(within(menu).queryByRole("menuitem", { name: "Move left" })).toBeNull();
  });
});

describe("the separator part", () => {
  it("draws a line after an item that asks for one, hidden from a reader", () => {
    render(
      <StatusBar
        aria-label="Status"
        items={[own("purlis.project", "left", 2, { separated: true }), own("purlis.ws", "left", 1)]}
      />,
    );
    const line = document.querySelector('[data-scope="status-bar"][data-part="separator"]');
    expect(line).not.toBeNull();
    expect(line).toHaveAttribute("aria-hidden", "true");
    expect(line?.previousElementSibling).toHaveAttribute("data-item", "purlis.project");
  });
});
