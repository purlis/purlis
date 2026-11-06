// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The strip's `+` and its list** (#5292): a press opens an empty tab; Down shows the list of
 * what opens in a new tab, what starts first and the views under a rule, each with its key; a
 * long list has a filter. Each picture is the page's, since the list draws outside its frame, and
 * each is the keyboard's: no picture waits on a rest or a tooltip. The rest's timing is
 * `NewTabButton.test.tsx`'s.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, screen, waitFor, within } from "storybook/test";
import type { Offer } from "./actions";
import { rowKey } from "./keys/registry";
import { NewTabButton } from "./NewTabButton";
import type { NewTabRow } from "./newTabList";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { MenuShortcuts } from "./ui/menu/Menu";

const offer = (id: string, title: string, does: Offer["does"]): Offer => ({
  id,
  title,
  available: true,
  reason: "",
  does,
});

const NEW_TAB = offer("tab.new", "New tab", { verb: "tab.new" });

const row = (one: Offer, group: NewTabRow["group"], said = one.title): NewTabRow => ({
  offer: one,
  said,
  group,
});

/** The list before a project is open: eight rows, no filter. */
const SHORT: NewTabRow[] = [
  row(NEW_TAB, "start", "Empty tab"),
  row(offer("chat.new", "New chat", { verb: "chat.new" }), "start"),
  row(offer("shell.new", "New shell", { verb: "newShell" }), "start"),
  row(offer("browser.new", "New browser", { verb: "newBrowser" }), "view"),
  row(offer("extensions.show", "Extensions…", { verb: "showExtensions" }), "view"),
  row(
    offer("shortcuts.show", "Keyboard shortcuts", {
      verb: "openView",
      view: { from: null, view: "shortcuts", key: "" },
      title: "Keyboard shortcuts",
    }),
    "view",
  ),
  row(offer("settings.show", "Settings…", { verb: "openSettingsTab" }), "view"),
  row(
    offer("agent-identity.show", "Agent identity", {
      verb: "openView",
      view: { from: null, view: "agent-identity", key: "" },
      title: "Agent identity",
    }),
    "view",
  ),
];

const view = (id: string, said: string): NewTabRow =>
  row(
    offer(id, `Open ${said.toLowerCase()}`, {
      verb: "openView",
      view: { from: null, view: id, key: "" },
      title: said,
    }),
    "view",
    said,
  );

/** The list with a project open: past eight rows, so it has a filter. */
const LONG: NewTabRow[] = [
  ...SHORT,
  view("memory.shared", "Shared memory"),
  view("dispatches.show", "Dispatches"),
  view("browsers.show", "Browsers"),
  view("skills.show", "Skill studio"),
];

/** The `+` at the end of a strip, with room under it for its list. Keys as a Mac's say them,
 *  so the pictures are the same on every runner. */
function Strip(props: Parameters<typeof NewTabButton>[0]) {
  return (
    <MenuShortcuts.Provider value={(act) => rowKey(act, true)}>
      <div style={{ blockSize: 480, display: "flex", alignItems: "flex-start" }}>
        <NewTabButton {...props} />
      </div>
    </MenuShortcuts.Provider>
  );
}

const meta = {
  component: Strip,
  args: { offer: NEW_TAB, rows: SHORT, onPress: fn() },
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Strip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** How long a list may take to go: WebKit draws the next frame late in a busy run. */
const CLOSES = { timeout: 3_000 };

const plus = (canvas: { getByRole: typeof screen.getByRole }) =>
  canvas.getByRole("button", { name: "New tab" });
const theList = () => screen.getByRole("menu", { name: "Open in a new tab" });

/** **A press is an empty tab**, by Enter as by the pointer, and opens no list. No picture: it
 *  is the `+` alone, which the strip's own stories draw. */
export const PressIsAnEmptyTab: Story = {
  parameters: { screenshot: false },
  play: async ({ canvas, args, userEvent }) => {
    await userEvent.tab();
    await untilFocused(() => plus(canvas), "Tab onto +");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onPress).toHaveBeenCalledWith(NEW_TAB);
    await expect(screen.queryByRole("menu")).toBeNull();
  },
};

/**
 * **Down shows the list** on its first row: Empty tab, New chat and New shell, a rule, then the
 * views, each with its key. Esc gives the keyboard back to `+`; Down again and the arrows walk to
 * New shell, which the picture shows.
 */
export const ItsList: Story = {
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await untilFocused(() => plus(canvas), "Its list: Tab onto +");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(theList()).getByRole("menuitem", { name: /Empty tab/ }),
      "Its list: Down opens it on its first row",
    );
    await expect(within(theList()).getAllByRole("separator")).toHaveLength(1);
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await untilFocused(() => plus(canvas), "Its list: Esc gives the keyboard back to +");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(theList()).getByRole("menuitem", { name: /Empty tab/ }),
      "Its list: Down opens it again",
    );
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    await untilFocused(
      () => within(theList()).getByRole("menuitem", { name: /New shell/ }),
      "Its list: the arrows reach New shell",
    );
    expectFocusInSight();
  },
};

/** **Enter chooses a row**, and the list closes: New chat, in a tab of its own. */
export const ChoosesARow: Story = {
  parameters: { screenshot: false },
  play: async ({ canvas, args, userEvent }) => {
    await userEvent.tab();
    await untilFocused(() => plus(canvas), "Chooses: Tab onto +");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(theList()).getByRole("menuitem", { name: /Empty tab/ }),
      "Chooses: Down opens it",
    );
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull(), CLOSES);
    await expect(args.onPress).toHaveBeenCalledWith(
      expect.objectContaining({ id: "chat.new", does: { verb: "chat.new", ownTab: true } }),
    );
  },
};

/**
 * **A long list has a filter**, which takes the keyboard as the list opens and narrows the rows;
 * the picture shows it narrowed, with the arrows on the row they reached.
 */
export const Filtered: Story = {
  args: { rows: LONG },
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await untilFocused(() => plus(canvas), "Filtered: Tab onto +");
    await userEvent.keyboard("{ArrowDown}");
    const filter = await screen.findByRole("textbox", { name: "Filter" });
    await untilFocused(() => filter, "Filtered: the filter takes the keyboard");
    await userEvent.keyboard("new");
    await waitFor(() =>
      expect(
        within(theList())
          .getAllByRole("menuitem")
          .map((one) => one.querySelector('[data-part="label"]')?.textContent),
      ).toEqual(["New chat", "New shell", "New browser"]),
    );
    await userEvent.keyboard("{ArrowDown}");
    expectFocusInSight();
  },
};

/** **Nothing matches**: the list says so, and the field keeps the keyboard. */
export const NothingMatches: Story = {
  args: { rows: LONG },
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    await untilFocused(() => plus(canvas), "Nothing: Tab onto +");
    await userEvent.keyboard("{ArrowDown}");
    const filter = await screen.findByRole("textbox", { name: "Filter" });
    await untilFocused(() => filter, "Nothing: the filter takes the keyboard");
    await userEvent.keyboard("zzz");
    await waitFor(() => expect(theList()).toHaveTextContent("Nothing matches what you typed."));
    expectFocusInSight();
  },
};
