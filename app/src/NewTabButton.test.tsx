// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { catalogue, type Now, type Offer } from "./actions";
import { MenuShortcuts } from "./ui/menu/Menu";
import { rowKey } from "./keys/registry";
import { NewTabButton } from "./NewTabButton";
import { CLOSES_AFTER_MS, newTabRows, OPENS_AFTER_MS } from "./newTabList";
import { noTabs } from "./tabs";

/**
 * **The strip's `+` and its list** (#5292; spec #5288 stories 73 to 81, the prototype's "The +
 * button" scenarios): a press opens an empty tab, a rest or Down shows the list, the pointer can
 * move onto it, it closes soon after the pointer leaves, and a long one has a filter. How it
 * opens each row in the window is `NewTab.window.test.tsx`.
 */

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const now = (over: Partial<Now> = {}): Now => ({
  tabs: noTabs(),
  workspaces: ["alpha"],
  focused: "alpha",
  needsYou: [],
  nameOf: String,
  ...over,
});

function drawn({ long = false } = {}) {
  const offers = catalogue(now(long ? { plane: "/p" } : {}));
  const pressed: Offer[] = [];
  const onPress = vi.fn((offer: Offer) => void pressed.push(offer));
  render(
    <MenuShortcuts.Provider value={(act) => rowKey(act, true)}>
      <p>Elsewhere</p>
      <NewTabButton
        offer={offers.find((one) => one.id === "tab.new")}
        rows={newTabRows(offers)}
        onPress={onPress}
      />
    </MenuShortcuts.Provider>,
  );
  return { plus: screen.getByRole("button", { name: "New tab" }), pressed, onPress };
}

const list = () => screen.queryByRole("menu", { name: "Open in a new tab" });
const rowNames = () =>
  within(list() as HTMLElement)
    .getAllByRole("menuitem")
    .map((row) => row.querySelector('[data-part="label"]')?.textContent ?? row.textContent);

/** Only the clocks the list's rest and grace run on, and the frames a menu takes the keyboard on. */
const fakeClocks = () =>
  vi.useFakeTimers({
    toFake: ["setTimeout", "clearTimeout", "requestAnimationFrame", "cancelAnimationFrame"],
  });
const pass = (ms: number) => act(() => void vi.advanceTimersByTime(ms));

describe("pressing + (#5292; story 73)", () => {
  it("opens an empty tab, and no list", async () => {
    const { plus, pressed } = drawn();

    await userEvent.click(plus);

    expect(pressed.map((offer) => offer.id)).toEqual(["tab.new"]);
    expect(list()).toBeNull();
  });

  it("says it has a list, and that it is closed", () => {
    const { plus } = drawn();

    expect(plus.getAttribute("aria-haspopup")).toBe("menu");
    expect(plus.getAttribute("aria-expanded")).toBe("false");
  });
});

describe("hovering + (#5292; stories 74 to 76)", () => {
  it("shows the list shortly after the pointer arrives, not at once", () => {
    fakeClocks();
    const { plus } = drawn();

    fireEvent.pointerEnter(plus);
    pass(OPENS_AFTER_MS - 1);
    expect(list()).toBeNull();
    pass(1);

    expect(list()).not.toBeNull();
    expect(plus.getAttribute("aria-expanded")).toBe("true");
  });

  it("opens nothing for a pointer that only passes over it", () => {
    fakeClocks();
    const { plus } = drawn();

    fireEvent.pointerEnter(plus);
    pass(OPENS_AFTER_MS / 2);
    fireEvent.pointerLeave(plus);
    pass(OPENS_AFTER_MS * 2);

    expect(list()).toBeNull();
  });

  it("takes no keyboard: whoever was typing goes on typing", () => {
    fakeClocks();
    const { plus } = drawn();
    const elsewhere = screen.getByText("Elsewhere");
    elsewhere.tabIndex = -1;
    elsewhere.focus();

    fireEvent.pointerEnter(plus);
    pass(OPENS_AFTER_MS);

    expect(list()).not.toBeNull();
    expect(document.activeElement).toBe(elsewhere);
  });

  it("stays open as the pointer moves onto the list, and closes soon after it leaves both", () => {
    fakeClocks();
    const { plus } = drawn();
    fireEvent.pointerEnter(plus);
    pass(OPENS_AFTER_MS);
    const menu = list() as HTMLElement;
    const surface = menu.closest('[data-scope="popover"][data-part="root"]') ?? menu;

    fireEvent.pointerLeave(plus);
    pass(CLOSES_AFTER_MS / 2);
    fireEvent.pointerEnter(surface);
    pass(CLOSES_AFTER_MS * 2);
    expect(list()).not.toBeNull();

    fireEvent.pointerLeave(surface);
    pass(CLOSES_AFTER_MS - 1);
    expect(list()).not.toBeNull();
    pass(1);
    expect(list()).toBeNull();
  });

  it("goes when + is pressed, and the press is the empty tab", async () => {
    fakeClocks();
    const { plus, pressed } = drawn();
    fireEvent.pointerEnter(plus);
    pass(OPENS_AFTER_MS);
    expect(list()).not.toBeNull();

    fireEvent.click(plus);

    expect(pressed.map((offer) => offer.id)).toEqual(["tab.new"]);
    expect(list()).toBeNull();
    // Still over it: a press is not a reason to open it again.
    pass(OPENS_AFTER_MS * 2);
    expect(list()).toBeNull();
  });
});

describe("a press from the keyboard (#5292)", () => {
  it("leaves the next rest free to open the list", async () => {
    const { plus } = drawn();
    plus.focus();
    await userEvent.keyboard("{Enter}");
    fakeClocks();

    fireEvent.pointerEnter(plus);
    pass(OPENS_AFTER_MS);

    expect(list()).not.toBeNull();
  });
});

describe("the list from the keyboard (#5292; story 77)", () => {
  it("opens on Down, on its first row, and Esc gives the keyboard back to +", async () => {
    const { plus } = drawn();
    plus.focus();

    await userEvent.keyboard("{ArrowDown}");

    await waitFor(() => expect(list()).not.toBeNull());
    await waitFor(() => expect(document.activeElement?.textContent).toContain("Empty tab"));
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement?.textContent).toContain("New chat");

    await userEvent.keyboard("{Escape}");

    await waitFor(() => expect(list()).toBeNull());
    expect(document.activeElement).toBe(plus);
  });

  it("chooses a row with Enter, in a new tab", async () => {
    const { plus, pressed } = drawn();
    plus.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement?.textContent).toContain("Empty tab"));

    await userEvent.keyboard("{ArrowDown}{Enter}");

    await waitFor(() => expect(list()).toBeNull());
    expect(pressed.map((offer) => offer.does)).toEqual([{ verb: "chat.new", ownTab: true }]);
  });
});

describe("the list's rows (#5292; stories 78 and 81)", () => {
  it("holds Empty tab, New chat and New shell first, then the views, each with its key", async () => {
    const { plus } = drawn();
    plus.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(list()).not.toBeNull());

    expect(rowNames().slice(0, 3)).toEqual(["Empty tab", "New chat", "New shell"]);
    const rows = within(list() as HTMLElement).getAllByRole("menuitem");
    const keyOf = (row: HTMLElement) => row.querySelector('[data-part="shortcut"]')?.textContent;
    expect(rows.slice(0, 3).map(keyOf)).toEqual(["⌘T", "⌘N", "⌘⇧T"]);
    expect(within(list() as HTMLElement).getAllByRole("separator")).toHaveLength(1);
  });

  it("opens a new shell chosen there in a tab of its own", async () => {
    const { plus, pressed } = drawn();
    plus.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(list()).not.toBeNull());

    await userEvent.click(
      within(list() as HTMLElement).getByRole("menuitem", { name: /New shell/ }),
    );

    expect(pressed.map((offer) => offer.does)).toEqual([{ verb: "newShell", ownTab: true }]);
  });

  it("has no filter while it is short", async () => {
    const { plus } = drawn();
    plus.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(list()).not.toBeNull());

    expect(screen.queryByRole("searchbox", { name: "Filter" })).toBeNull();
    expect(screen.queryByRole("textbox", { name: "Filter" })).toBeNull();
  });
});

describe("a long list's filter (#5292; story 80)", () => {
  const filterField = () => screen.getByRole("textbox", { name: "Filter" });

  it("takes the keyboard as the list opens from the keyboard, and narrows the rows", async () => {
    const { plus } = drawn({ long: true });
    plus.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement).toBe(filterField()));

    await userEvent.keyboard("shell");

    expect(rowNames()).toEqual(["New shell"]);
    expect(within(list() as HTMLElement).queryAllByRole("separator")).toHaveLength(0);
  });

  it("says so when nothing matches", async () => {
    const { plus } = drawn({ long: true });
    plus.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement).toBe(filterField()));

    await userEvent.keyboard("nothing like it");

    // React Aria draws the empty state in a row of its own, which holds no catalogue row.
    expect(list()?.querySelectorAll('[data-scope="menu"][data-part="root"]')).toHaveLength(0);
    expect(list()?.textContent).toBe("Nothing matches what you typed.");
  });

  it("chooses the row the arrows reach from the field with Enter", async () => {
    const { plus, pressed } = drawn({ long: true });
    plus.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement).toBe(filterField()));

    await userEvent.keyboard("dispatch{ArrowDown}");
    const aimed = within(list() as HTMLElement).getByRole("menuitem", { name: /Dispatches/ });
    await waitFor(() => expect(filterField()).toHaveAttribute("aria-activedescendant", aimed.id));
    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(list()).toBeNull());
    expect(pressed.map((offer) => offer.id)).toEqual(["dispatches.show"]);
  });

  it("starts empty again each time it opens", async () => {
    const { plus } = drawn({ long: true });
    plus.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement).toBe(filterField()));
    await userEvent.keyboard("shell{Escape}");
    await waitFor(() => expect(list()).toBeNull());

    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement).toBe(filterField()));

    expect((filterField() as HTMLInputElement).value).toBe("");
  });
});
