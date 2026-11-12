// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Links in a pane's output** (#6215, the accepted prototype's `AtRest` and `HoverAUrl`): a real
 * xterm, the read-only terminal's, holding a harness's turn with a web address in it, with the
 * core's answers stood in for. At rest nothing marks the link (D1); under the pointer it is
 * underlined, drawn in the theme's link colour, and its tooltip says its target and what ⌘-click
 * does (D2). A link the core would not open says why in the window's "Link not opened".
 *
 * Chat links (#6217, the prototype's `HoverAChat`, `HoverAClosedChat` and
 * `ClosedChatCardFromALink`): `chat 660`, open in another workspace, and `chat 641`, finished,
 * whose ⌘-click opens its card beside the link.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { ChatCard, LinkCandidate, LinkOpened, LinkSeen } from "./bindings";
import { LinkAnswer } from "./PaneLinkSurfaces";
import { usePaneLinkChats } from "./PaneLinkChats";
import { LINK_CORE } from "./paneLinkCore";
import { onAMac } from "./mac";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

const ADDRESS = "https://github.com/purlis/purlis/pull/6148";
/** A file a harness named, and the core's reading of it (#6216). */
const FILE = "app/src/paneLinks.test.ts:18";
const FILE_IN_FULL =
  "/Users/you/projects/plane/workspaces/smart-ide/purlis/app/src/paneLinks.test.ts:18";

/** A harness's turn, as the prototype's pane holds it. */
const TURN = [
  "\u001b[1m● The resolver's tests pass.\u001b[0m",
  `  It follows ${ADDRESS} (issue #5820).`,
  "\u001b[2m  ⎿ 12 tests, 0 failed\u001b[22m",
].join("\r\n");

const COLS = 72;
const ROWS = 5;

/** A harness's turn naming a file at a line. */
const FILE_TURN = [
  "\u001b[1m● Added the test.\u001b[0m",
  `  See ${FILE} for the case.`,
  "\u001b[2m  ⎿ 1 test, 0 failed\u001b[22m",
].join("\r\n");

/** Chat 641's card, finished at 11:08 (local time, so every cell says it). */
const CARD: ChatCard = {
  number: 641,
  chat: "01K6W0RKER000000000000000B",
  title: "Signed commits #5607",
  ended: "2026-11-11T11:08:00",
  place: "smart-ide",
  started_by: "steward 4",
  screen_kept: true,
  task: {
    id: "01K9DISPATCH0000000000000A",
    asker: 0,
    name: "Signed commits #5607",
    chat: null,
    persona: "steward",
    how: "done",
    outcome: "done",
    folds: true,
    report:
      "Done. Commits are signed on every lane, and CI checks each signature before the merge queue takes a PR.",
    changed: null,
    ended: "2026-11-11T11:08:00",
    place: "smart-ide",
    branch: null,
    reopens: true,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
  },
};

/** A dispatch list naming two chats: one finished, one open in another workspace. */
const CHAT_TURN = [
  "  chat 641 · Signed commits #5607 · finished 11:08 · report sent",
  "  chat 660 · Docs site: the setup page · working in docs",
].join("\r\n");

/** What the core says a chat's number is (#6217): none for a number no chat had. */
function chatSeen(text: string): LinkSeen | null {
  if (text === "chat 641")
    return {
      kind: "chat",
      target: "chat 641",
      shown: "#641 · Signed commits #5607",
      does: { does: "opens", into: "window" },
      chat: { is: "ended", card: CARD },
    };
  if (text === "chat 660")
    return {
      kind: "chat",
      target: "chat 660",
      shown: "#660 · Docs site: the setup page",
      does: { does: "opens", into: "window" },
      chat: { is: "open", number: 660, workspace: "docs" },
    };
  return null;
}

/** A link a program marked (OSC 8, #6218): words that read as one address, marked as a link to
 *  another, as the prototype's `HoverAnOsc8Link` draws it. */
const MARKED_WORDS = "docs.purlis.dev/setup";
const MARKED_TARGET = "https://purlis-setup.example.net/s?id=7";
const MARKED_TURN = [
  "\u001b[1m● Set-up is described in the docs.\u001b[0m",
  `  Read \u001b]8;;${MARKED_TARGET}\u001b\\${MARKED_WORDS}\u001b]8;;\u001b\\ before you start.`,
  "\u001b[2m  ⎿ 1 link\u001b[22m",
].join("\r\n");

function Pane({ screen = TURN }: { screen?: string }) {
  // A chat link's card, as a chat's pane draws it.
  const chats = usePaneLinkChats(() => {});
  return (
    <div style={{ inlineSize: 640, paddingBlockEnd: "var(--space-8)" }}>
      {chats.surfaces}
      <ReadOnlyTerminal
        onChat={chats.onChat}
        screen={screen}
        label="A saved chat's last screen"
        cols={COLS}
        rows={ROWS}
        fontSize={13}
        links={LINK_CORE}
      />
    </div>
  );
}

const meta = {
  title: "PaneLinkSurfaces",
  component: Pane,
  // The tooltip is drawn over the page, outside the story's frame: the page is pictured.
  parameters: { screenshot: "page" },
  beforeEach: () => {
    mockIPC((cmd, args) => {
      if (cmd === "pane_link_resolve") {
        const { candidates } = args as {
          candidates: { kind: string; text: string; target?: string }[];
        };
        return candidates.map((one) =>
          one.kind === "osc8"
            ? {
                kind: "url",
                target: one.target,
                shown: one.target,
                does: { does: "asks", reason: "its text says docs.purlis.dev" },
                its_text: one.text,
                chat: null,
              }
            : one.kind === "chat"
              ? chatSeen(one.text)
              : one.kind === "file"
                ? one.text === FILE
                  ? {
                      kind: "file",
                      target: FILE_IN_FULL,
                      shown: FILE,
                      does: { does: "opens", into: "editor-tab" },
                      chat: null,
                    }
                  : null
                : {
                    kind: "url",
                    target: one.text,
                    shown: one.text,
                    does: { does: "opens", into: "browser" },
                    chat: null,
                  },
        );
      }
      if (cmd === "pane_link_open") {
        const { candidate } = args as { candidate: { text: string } };
        if (candidate.text === "chat 641")
          return { answer: "chat", go: { to: "card", card: CARD } };
        return { answer: "opened", kind: "url", target: ADDRESS };
      }
      return null;
    });
    return () => clearMocks();
  },
} satisfies Meta<typeof Pane>;
export default meta;

/** The cell at (`col`, `row`) of the screen, as the pointer would be on it. */
function cellOf(canvas: HTMLElement, col: number, row: number) {
  const screen = canvas.querySelector<HTMLElement>(".xterm-screen");
  if (screen === null) throw new Error("no terminal screen");
  const box = screen.getBoundingClientRect();
  const init = {
    bubbles: true,
    cancelable: true,
    clientX: box.left + ((col + 0.5) * box.width) / COLS,
    clientY: box.top + ((row + 0.5) * box.height) / ROWS,
  };
  return { screen, init };
}

/** **At rest**: the address is the program's text, unmarked (D1). */
export const AtRest: StoryObj<typeof meta> = {
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await waitFor(() =>
      expect(canvasElement.querySelector(".xterm-screen")?.textContent ?? "").toContain(ADDRESS),
    );
    expect(page.queryByRole("tooltip")).toBeNull();
  },
};

/** **Hovering an https address**: underlined, in the link colour, its target and what ⌘-click
 *  does beside it. */
export const HoverAUrl: StoryObj<typeof meta> = {
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await waitFor(() =>
      expect(canvasElement.querySelector(".xterm-screen")?.textContent ?? "").toContain(ADDRESS),
    );
    // On the address: row 1, past "  It follows ".
    const { screen, init } = cellOf(canvasElement, 20, 1);
    screen.dispatchEvent(new MouseEvent("mousemove", init));
    const tip = await page.findByRole("tooltip", {}, { timeout: 3000 });
    await expect(tip).toHaveTextContent(ADDRESS);
    await expect(tip).toHaveTextContent(/-click opens it in your browser/);
  },
};

/** **Hovering a file at a line** (#6216, the prototype's `HoverAFileLine`): underlined, the file
 *  from the workspace with its line (D2), and that ⌘-click opens it in an editor tab. */
export const HoverAFileLine: StoryObj<typeof meta> = {
  render: () => <Pane screen={FILE_TURN} />,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await waitFor(() =>
      expect(canvasElement.querySelector(".xterm-screen")?.textContent ?? "").toContain(FILE),
    );
    // On the path: row 1, past "  See ".
    const { screen, init } = cellOf(canvasElement, 12, 1);
    screen.dispatchEvent(new MouseEvent("mousemove", init));
    const tip = await page.findByRole("tooltip", {}, { timeout: 3000 });
    // A file in the chat's workspace is said from the workspace (D2).
    await expect(tip).toHaveTextContent(FILE);
    await expect(tip).toHaveTextContent(/-click opens it in an editor tab/);
  },
};

/** The pane's screen, once it holds the turn. */
async function drawn(canvasElement: HTMLElement) {
  await waitFor(() =>
    expect(canvasElement.querySelector(".xterm-screen")?.textContent ?? "").toContain("chat 660"),
  );
}

/** **Hovering `chat 660`**, open in another workspace: "#660 · <title>", then where it is and
 *  that ⌘-click shows it there. */
export const HoverAChat: StoryObj<typeof meta> = {
  render: () => <Pane screen={CHAT_TURN} />,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await drawn(canvasElement);
    // On "chat 660": row 1, past "  ".
    const { screen, init } = cellOf(canvasElement, 4, 1);
    screen.dispatchEvent(new MouseEvent("mousemove", init));
    const tip = await page.findByRole("tooltip", {}, { timeout: 3000 });
    await expect(tip).toHaveTextContent("#660 · Docs site: the setup page");
    await expect(tip).toHaveTextContent(/Open in docs · .+-click shows it there/);
  },
};

/** **Hovering `chat 641`**, which has finished: when, and that ⌘-click shows its card. */
export const HoverAClosedChat: StoryObj<typeof meta> = {
  render: () => <Pane screen={CHAT_TURN} />,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await drawn(canvasElement);
    const { screen, init } = cellOf(canvasElement, 4, 0);
    screen.dispatchEvent(new MouseEvent("mousemove", init));
    const tip = await page.findByRole("tooltip", {}, { timeout: 3000 });
    await expect(tip).toHaveTextContent("#641 · Signed commits #5607");
    await expect(tip).toHaveTextContent(/Done 11:08 · .+-click shows its card/);
  },
};

/** **⌘-click on `chat 641`**: its card in a popover beside the link, *Read it* first and
 *  *Reopen* plain; Esc closes it. */
export const ClosedChatCardFromALink: StoryObj<typeof meta> = {
  render: () => <Pane screen={CHAT_TURN} />,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await drawn(canvasElement);
    const { screen, init } = cellOf(canvasElement, 4, 0);
    screen.dispatchEvent(new MouseEvent("mousemove", init));
    await page.findByRole("tooltip", {}, { timeout: 3000 });
    // ⌘ on a Mac and Ctrl elsewhere: both held, so the story opens it on either.
    for (const type of ["mousedown", "mouseup", "click"])
      screen.dispatchEvent(
        new MouseEvent(type, { ...init, button: 0, detail: 1, metaKey: true, ctrlKey: true }),
      );
    const card = await page.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    const read = within(card).getByRole("button", { name: "Read it" });
    // Drawn once the popover has come in.
    await waitFor(() => expect(read).toBeVisible());
    await expect(within(card).getByRole("button", { name: "Reopen" })).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => read, "Tab reaches Read it first");
    expectFocusInSight();
    await pictureNow();

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("dialog")).toBeNull());
  },
};

/**
 * **The card from Links in this pane…** (#6442): the keyboard's way to the same card. The list
 * closes on Enter, so the row is gone, and the card stands at the link, as a ⌘-click's does (the
 * prototype's D7), and not at the window's top left.
 */
export const ClosedChatCardFromTheList: StoryObj<typeof meta> = {
  render: () => <Pane screen={CHAT_TURN} />,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await drawn(canvasElement);
    const region = await page.findByRole("region", { name: "A saved chat's last screen" });
    region.focus();
    await userEvent.keyboard(
      onAMac() ? "{Meta>}{Shift>}L{/Shift}{/Meta}" : "{Control>}{Shift>}L{/Shift}{/Control}",
    );
    const list = await page.findByRole("dialog", { name: "Links in this pane" });
    const row = await within(list).findByRole("option", { name: "chat 641" });
    await userEvent.keyboard("{ArrowDown}");
    // Newest first: chat 660 on the lower row, then chat 641.
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row, "The down arrow reaches chat 641");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");

    const card = await page.findByRole("dialog", { name: "#641 · Signed commits #5607" });
    const read = within(card).getByRole("button", { name: "Read it" });
    await waitFor(() => expect(read).toBeVisible());
    // Beside the link: below its row, within the screen, not at the window's corner.
    const { screen } = cellOf(canvasElement, 4, 0);
    const box = screen.getBoundingClientRect();
    const at = card.getBoundingClientRect();
    await expect(at.top).toBeGreaterThan(box.top);
    await expect(at.left).toBeGreaterThanOrEqual(box.left - 1);
    await userEvent.tab();
    await untilFocused(() => read, "Tab reaches Read it first");
    expectFocusInSight();
    await pictureNow();

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("dialog")).toBeNull());
  },
};

/** The window's answer to a link ⌘-clicked, up until it is closed (Esc, or an answer). */
function Answer({ answer, candidate }: { answer: LinkOpened; candidate: LinkCandidate }) {
  const [up, setUp] = useState(true);
  if (!up) return <p>Closed.</p>;
  return (
    <LinkAnswer
      answering={{ answer, candidate }}
      onClose={() => setUp(false)}
      onConfirm={() => setUp(false)}
    />
  );
}

/** **A link the core would not open**: the sentence that says why, and Close. */
export const Refused: StoryObj<typeof meta> = {
  render: () => (
    <Answer
      answer={{
        answer: "refused",
        reason: "purlis will not open an address that carries a name or a password",
      }}
      candidate={{ kind: "url", text: "https://purlis.dev/account" }}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const close = await page.findByRole("button", { name: "Close" });
    await untilFocused(() => close, "It starts on Close");
    expectFocusInSight();
    // purlis's name is lowercase, even where it starts the sentence (docs/ui-copy.md).
    await expect(page.getByRole("dialog", { name: "Link not opened" })).toHaveTextContent(
      "purlis will not open an address that carries a name or a password.",
    );
    await pictureNow();

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("dialog")).toBeNull());
  },
};

/**
 * **A link the core asks about first** (the OSC 8 kind, #6218, in general words): its text
 * and where it really goes, Cancel first and Open link beside it.
 */
export const Asks: StoryObj<typeof meta> = {
  render: () => (
    <Answer
      answer={{
        answer: "ask",
        reason: "its text names another address than the one it goes to",
        shown: "docs.purlis.dev/setup",
        target: "https://purlis-setup.example.net/s",
        token: "a-one-use-token",
        about: { about: "link" },
      }}
      candidate={{ kind: "url", text: "docs.purlis.dev/setup" }}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("dialog", { name: "Open this link?" });
    await expect(dialog).toHaveTextContent("docs.purlis.dev/setup");
    await expect(dialog).toHaveTextContent("https://purlis-setup.example.net/s");
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "It starts on Cancel, never on Open link");
    expectFocusInSight();
    await pictureNow();

    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Open link" }),
      "Tab reaches Open link",
    );
    expectFocusInSight();

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("dialog")).toBeNull());
  },
};

/**
 * **A file outside the workspace** (#6216, the prototype's S7 `FileOutsideTheWorkspace`, D9): the
 * file and the workspace, *Don't open*, *Copy path* and *Open file*. It starts on *Open file*,
 * since opening only shows the file in an editor tab, and Return opens it.
 */
export const FileOutsideTheWorkspace: StoryObj<typeof meta> = {
  render: () => (
    <Answer
      answer={{
        answer: "ask",
        reason: "outside smart-ide",
        shown: "/etc/hosts:3",
        target: "/etc/hosts:3",
        token: "a-one-use-token",
        about: {
          about: "file-outside",
          home: {
            is: "workspace",
            name: "smart-ide",
            folder: "/Users/you/projects/plane/workspaces/smart-ide",
          },
        },
      }}
      candidate={{ kind: "file", text: "/etc/hosts:3" }}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("dialog", {
      name: "Open a file outside this workspace?",
    });
    await expect(dialog).toHaveTextContent(
      "This chat's output names a file that is not in smart-ide.",
    );
    await expect(dialog).toHaveTextContent("/etc/hosts:3");
    await expect(dialog).toHaveTextContent("It opens in an editor tab. Nothing is run.");
    const open = within(dialog).getByRole("button", { name: "Open file" });
    await untilFocused(() => open, "It starts on Open file (D9)");
    expectFocusInSight();
    await pictureNow();

    await userEvent.tab({ shift: true });
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Copy path" }),
      "Shift+Tab reaches Copy path",
    );
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Don't open" }),
      "and then Don't open",
    );
    expectFocusInSight();

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("dialog")).toBeNull());
  },
};

/**
 * **Hovering a link a program marked** (OSC 8, #6218, the prototype's `HoverAnOsc8Link`): its
 * tooltip says where it really goes first, "Goes to …", then that its words say another host
 * and ⌘-click asks first.
 */
export const HoverAnOsc8Link: StoryObj<typeof meta> = {
  render: () => <Pane screen={MARKED_TURN} />,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    await waitFor(() =>
      expect(canvasElement.querySelector(".xterm-screen")?.textContent ?? "").toContain(
        MARKED_WORDS,
      ),
    );
    // On the words: row 1, past "  Read ".
    const { screen, init } = cellOf(canvasElement, 12, 1);
    screen.dispatchEvent(new MouseEvent("mousemove", init));
    const tip = await page.findByRole("tooltip", {}, { timeout: 3000 });
    await expect(tip).toHaveTextContent(`Goes to ${MARKED_TARGET}`);
    await expect(tip).toHaveTextContent(/Its text says docs\.purlis\.dev · .+-click asks first/);
  },
};

/**
 * **The ask for a marked link whose words name another address** (#6218, the prototype's S8
 * `Osc8Mismatch`, D10): its words and where it really goes, the real host in bold. It starts on
 * *Don't open*, Return never opens it, and *Open in browser* is plain.
 */
export const Osc8Mismatch: StoryObj<typeof meta> = {
  render: () => (
    <Answer
      answer={{
        answer: "ask",
        reason: "its text says docs.purlis.dev",
        shown: MARKED_WORDS,
        target: MARKED_TARGET,
        token: "a-one-use-token",
        about: { about: "misleading", kind: "url", host: "purlis-setup.example.net" },
      }}
      candidate={{ kind: "osc8", text: MARKED_WORDS, target: MARKED_TARGET }}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("dialog", {
      name: "This link goes somewhere its text doesn't say",
    });
    await expect(dialog).toHaveTextContent(MARKED_WORDS);
    await expect(dialog).toHaveTextContent(MARKED_TARGET);
    await expect(within(dialog).getByText("purlis-setup.example.net").tagName).toBe("STRONG");
    await expect(dialog).toHaveTextContent(
      "Open it only if you expected purlis-setup.example.net.",
    );
    const dont = within(dialog).getByRole("button", { name: "Don't open" });
    await untilFocused(() => dont, "It starts on Don't open (D10)");
    expectFocusInSight();
    await pictureNow();

    // Tab walks the answers in order, and back to Don't open.
    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Copy address" }),
      "Tab reaches Copy address",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Open in browser" }),
      "and then Open in browser",
    );
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    await untilFocused(() => dont, "Shift+Tab goes back to Don't open");
    expectFocusInSight();

    // Return on Don't open closes it, and never opens the link.
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(page.queryByRole("dialog")).toBeNull());
  },
};

/** **The same ask for a file** (#6218): a marked link whose words name another file. */
export const Osc8MismatchAFile: StoryObj<typeof meta> = {
  render: () => (
    <Answer
      answer={{
        answer: "ask",
        reason: "its text says src/main.rs",
        shown: "src/main.rs",
        target: "/Users/you/projects/plane/workspaces/smart-ide/purlis/src/secrets.rs",
        token: "a-one-use-token",
        about: { about: "misleading", kind: "file", host: null },
      }}
      candidate={{
        kind: "osc8",
        text: "src/main.rs",
        target: "file:///Users/you/projects/plane/workspaces/smart-ide/purlis/src/secrets.rs",
      }}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const dialog = await page.findByRole("dialog", {
      name: "This link goes somewhere its text doesn't say",
    });
    await expect(dialog).toHaveTextContent("a link to a different file");
    const dont = within(dialog).getByRole("button", { name: "Don't open" });
    await untilFocused(() => dont, "It starts on Don't open");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Copy path" }),
      "Tab reaches Copy path",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => within(dialog).getByRole("button", { name: "Open file" }),
      "and then Open file",
    );
    expectFocusInSight();

    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("dialog")).toBeNull());
  },
};
