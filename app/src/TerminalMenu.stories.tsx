// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The terminal's menu with its link rows** (#6219, the accepted prototype's
 * `RightClickOverALink` and `MenuFromTheKeyboard`, S3 and S4): the real `TerminalMenu`, its rows
 * as `terminalRows.ts` declares them, over a stand-in for the terminal. Over a link, *Open link*
 * and *Copy link* come first, each with its target under it (D3); an address the core refuses
 * offers *Copy link* alone; opened from the keyboard there is no pointer, so *Links in this
 * pane…* takes their place (D4). A menu draws outside its frame, so each picture is the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { catalogued, type Offer } from "./actions";
import type { LinkUnder } from "./paneLinks";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";
import { TerminalMenu } from "./TerminalMenu";
import { linkRows, terminalRows } from "./terminalRows";

const ADDRESS = "https://github.com/purlis/purlis/pull/6148";

const OPENS: LinkUnder = {
  candidate: { kind: "url", text: ADDRESS },
  seen: { kind: "url", target: ADDRESS, does: { does: "opens" } },
};

const REFUSED: LinkUnder = {
  candidate: { kind: "url", text: "ftp://files.example/build.log" },
  seen: {
    kind: "url",
    target: "ftp://files.example/build.log",
    does: { does: "refused", reason: "purlis opens only http and https addresses" },
  },
};

function offer(id: string, title: string): Offer {
  return { id, title, available: true, reason: "", does: { verb: "nothing" } };
}

/** The pane's own rows, as `LayoutPanes` hands the catalogue's to a pane. */
const PANE_ROWS = catalogued([
  offer("pane.split.right", "Split right"),
  offer("pane.split.down", "Split down"),
]);

/** A terminal's menu over `link`, or over none; what a row did is said under it. */
function Over({ link }: { link: LinkUnder | undefined }) {
  const [said, setSaid] = useState("Nothing chosen");
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: 560 }}>
      <TerminalMenu
        rows={terminalRows({ selected: false, mac: true })}
        links={linkRows({ link, mac: true, lists: true })}
        onRun={(id) => setSaid(`Ran ${id}`)}
        pane={{ offers: PANE_ROWS, onPress: (row) => setSaid(`Ran ${row.id}`) }}
        onAsked={() => {}}
        onOpenChange={() => {}}
        onGiveBack={() => {}}
        onPassKey={() => {}}
      >
        <div role="group" aria-label="Terminal of chat 641" tabIndex={0} data-testid="terminal">
          <pre
            style={{ margin: 0 }}
          >{`● The resolver's tests pass.\n  It follows ${link?.candidate.text ?? ADDRESS}.`}</pre>
        </div>
      </TerminalMenu>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

const meta = {
  title: "TerminalMenu",
  component: Over,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Over>;
export default meta;
type Story = StoryObj<typeof meta>;

/** A right-click on the terminal, as the engine raises it. */
function rightClick(on: HTMLElement) {
  const box = on.getBoundingClientRect();
  on.dispatchEvent(
    new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      button: 2,
      clientX: box.left + 40,
      clientY: box.top + 24,
    }),
  );
}

/** **A right-click over a link**: Open link and Copy link first, each with its target under it. */
export const RightClickOverALink: Story = {
  args: { link: OPENS },
  play: async ({ canvas, userEvent }) => {
    rightClick(canvas.getByTestId("terminal"));
    const menu = await screen.findByRole("menu", { name: "Terminal" });
    const open = within(menu).getByRole("menuitem", { name: "Open link" });
    await expect(open).toHaveAccessibleDescription(ADDRESS);
    // The menu fades in as it appears: visible once it has.
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: "Copy link" })).toBeVisible(),
    );
    await pictureNow();

    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => open, "RightClickOverALink, step 1");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(
      () => within(menu).getByRole("menuitem", { name: "Copy link" }),
      "RightClickOverALink, step 2",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await expect(canvas.getByRole("status")).toHaveTextContent("Ran terminal.copyLink");
  },
};

/** **A right-click over an address the core refuses**: Copy link alone, there being nothing to
 *  open. */
export const RightClickOverARefusedLink: Story = {
  args: { link: REFUSED },
  play: async ({ canvas, userEvent }) => {
    rightClick(canvas.getByTestId("terminal"));
    const menu = await screen.findByRole("menu", { name: "Terminal" });
    await expect(within(menu).queryByRole("menuitem", { name: "Open link" })).toBeNull();
    await expect(
      within(menu).getByRole("menuitem", { name: "Copy link" }),
    ).toHaveAccessibleDescription(REFUSED.seen.target);
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  },
};

/** **The menu from the keyboard** (Shift+F10): no pointer, so no link under it. *Links in this
 *  pane…* is first, said with its key. */
export const MenuFromTheKeyboard: Story = {
  args: { link: undefined },
  play: async ({ canvas, userEvent }) => {
    canvas.getByTestId("terminal").focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "Terminal" });
    const links = within(menu).getByRole("menuitem", { name: "Links in this pane…" });
    await expect(links).toHaveAttribute("aria-keyshortcuts", "Shift+Meta+L");
    await expect(within(menu).queryByRole("menuitem", { name: "Open link" })).toBeNull();
    await untilFocused(() => links, "MenuFromTheKeyboard, step 1");
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  },
};
