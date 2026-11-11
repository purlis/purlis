// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/pane-links, #6211; spec #6208 and #5820). Never merges
 * to main. Every surface of links in a pane, on one stand-in chat pane: open
 * `PROTOTYPE/Pane links` in Storybook. The map of surfaces and actions is
 * `prototype-pane-links/WAYFINDER.md`; the self-grilled choices are its D-lines.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fireEvent, screen, waitFor, within } from "storybook/test";
import { ClosedChatCard, PaneLinks } from "./prototype-pane-links/PaneLinks";
import { CLOSED_CHAT } from "./prototype-pane-links/links";
import { TabAndHead } from "./prototype-pane-links/TabAndHead";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

const meta = {
  title: "PROTOTYPE/Pane links",
  component: PaneLinks,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof PaneLinks>;
export default meta;
type Story = StoryObj<typeof meta>;

/** The pane at rest: no link is marked until the pointer is on it (D1). */
export const AtRest: Story = { args: { start: { at: "rest" } } };

/** Hovering `chat 660`, open in another workspace: underlined, its tooltip says where it is. */
export const HoverAChat: Story = { args: { start: { at: "hover", link: "chat660" } } };

/** Hovering `chat 641`, which has finished: the tooltip says so, and ⌘-click shows its card. */
export const HoverAClosedChat: Story = { args: { start: { at: "hover", link: "chat641" } } };

/** Hovering a `path:line:col`: resolved against the chat's folder, said in full. */
export const HoverAFileLine: Story = { args: { start: { at: "hover", link: "resolve" } } };

/** Hovering an https address. */
export const HoverAUrl: Story = { args: { start: { at: "hover", link: "pr" } } };

/** Hovering an OSC 8 link whose text is another address: the tooltip leads with the real one. */
export const HoverAnOsc8Link: Story = { args: { start: { at: "hover", link: "setup" } } };

/** **Right-click over a link**: Open link and Copy link first, each with its target under it. */
export const RightClickOverALink: Story = {
  args: { start: { at: "rest" } },
  play: async ({ canvasElement }) => {
    const link = canvasElement.querySelector<HTMLElement>('[data-link="resolve"]');
    if (link === null) throw new Error("no link");
    const box = link.getBoundingClientRect();
    fireEvent.contextMenu(link, { clientX: box.left + 8, clientY: box.top + 6, button: 2 });
    const menu = await screen.findByRole("menu", { name: "Terminal" });
    await expect(within(menu).getByRole("menuitem", { name: /Open link/ })).toBeVisible();
    await expect(within(menu).getByRole("menuitem", { name: /Copy link/ })).toBeVisible();
  },
};

/**
 * **The menu from the keyboard** (Shift+F10 in the terminal): no pointer, so no link under it.
 * The first row is Links in this pane… instead (D4).
 */
export const MenuFromTheKeyboard: Story = {
  args: { start: { at: "rest" } },
  play: async ({ canvas, userEvent }) => {
    const terminal = canvas.getByLabelText(/Terminal of chat/);
    terminal.focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu", { name: "Terminal" });
    await expect(within(menu).getByRole("menuitem", { name: /Links in this pane/ })).toBeVisible();
    await expect(within(menu).queryByRole("menuitem", { name: /Open link/ })).toBeNull();
  },
};

/** **Links in this pane…** (⇧⌘L): the links on screen, newest first; typing narrows them. */
export const LinksInThisPane: Story = {
  args: { start: { at: "rest" } },
  play: async ({ canvas, userEvent }) => {
    canvas.getByLabelText(/Terminal of chat/).focus();
    await userEvent.keyboard("{Meta>}{Shift>}L{/Shift}{/Meta}");
    const dialog = await screen.findByRole("dialog", { name: "Links in this pane" });
    const rows = within(dialog).getAllByRole("option");
    // Newest first: the lowest link on screen leads.
    await expect(rows[0]).toHaveTextContent("/etc/hosts:3");
    await expect(rows.at(-1)).toHaveTextContent("chat 641");
    expectFocusInSight();
  },
};

/** **⌘-click on a closed chat** opens its card beside the link (#5820). */
export const ClosedChatCardFromALink: Story = {
  args: { start: { at: "rest" } },
  play: async ({ canvasElement }) => {
    const link = canvasElement.querySelector<HTMLElement>('[data-link="chat641"]');
    if (link === null) throw new Error("no link");
    fireEvent.click(link, { metaKey: true });
    const card = await screen.findByRole("dialog", { name: /#641 · Signed commits/ });
    await expect(within(card).getByRole("button", { name: "Read it" })).toBeVisible();
    await expect(within(card).getByRole("button", { name: "Reopen" })).toBeVisible();
  },
};

/** The card in each outcome, as the switcher's finished row opens it too (#5820). */
export const ClosedChatCardOutcomes: Story = {
  render: () => (
    <div style={{ display: "grid", gap: "var(--space-4)" }}>
      {[
        { ...CLOSED_CHAT },
        {
          ...CLOSED_CHAT,
          number: 633,
          title: "Docker pool on a full disk",
          outcome: "blocked" as const,
          finished: "09:41",
          report:
            "Blocked: the Docker VM's disk is full (ENOSPC). It needs the person to prune images; nothing else was changed.",
        },
        {
          ...CLOSED_CHAT,
          number: 629,
          title: "talk",
          outcome: "closed" as const,
          finished: "08:15",
          startedBy: "you",
          report: "",
        },
      ].map((chat) => (
        <div key={chat.number} className="pl-switcher" data-scope="popover" data-part="surface">
          <ClosedChatCard chat={chat} onRead={() => undefined} onReopen={() => undefined} />
        </div>
      ))}
    </div>
  ),
};

/** **The ask before a file outside the workspace opens.** */
export const FileOutsideTheWorkspace: Story = {
  args: { start: { at: "ask-outside" } },
  play: async () => {
    const dialog = await screen.findByRole("dialog", { name: /outside this workspace/ });
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Open file" }),
      "the ask starts on Open file: it opens only an editor tab",
    );
    expectFocusInSight();
  },
};

/** **The ask for an OSC 8 link whose text is another address**: both shown, the host bold. */
export const Osc8Mismatch: Story = {
  args: { start: { at: "ask-osc8" } },
  play: async ({ userEvent }) => {
    const dialog = await screen.findByRole("dialog", { name: /somewhere its text/ });
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Don't open" }),
      "the ask starts on Don't open",
    );
    await expect(dialog).toHaveTextContent("docs.purlis.dev/setup");
    await expect(dialog).toHaveTextContent("purlis-setup.example.net");
    expectFocusInSight();
    // Esc is Don't open.
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  },
};

/** **#641 on the tab's tooltip and in the chat's head** (#5820). */
export const NumberOnTabAndHead: Story = {
  render: () => <TabAndHead />,
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByTestId("head-number")).toHaveTextContent("#641");
    await userEvent.hover(canvas.getByRole("tab", { name: /Signed commits #5607/ }));
    const tip = await screen.findByRole("tooltip", {}, { timeout: 3000 });
    await expect(tip).toHaveTextContent("Chat #641");
  },
};
