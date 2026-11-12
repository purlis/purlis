// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Links in a pane's output** (#6215, the accepted prototype's `AtRest` and `HoverAUrl`): a real
 * xterm, the read-only terminal's, holding a harness's turn with a web address in it, with the
 * core's answers stood in for. At rest nothing marks the link (D1); under the pointer it is
 * underlined, drawn in the theme's link colour, and its tooltip says its target and what ⌘-click
 * does (D2). A link the core would not open says why in the window's "Link not opened".
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { LinkCandidate, LinkOpened } from "./bindings";
import { LinkAnswer } from "./PaneLinkSurfaces";
import { LINK_CORE } from "./paneLinkCore";
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

function Pane({ screen = TURN }: { screen?: string }) {
  return (
    <div style={{ inlineSize: 640, paddingBlockEnd: "var(--space-8)" }}>
      <ReadOnlyTerminal
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
        const { candidates } = args as { candidates: { kind: string; text: string }[] };
        return candidates.map((one) =>
          one.kind === "file"
            ? one.text === FILE
              ? { kind: "file", target: FILE_IN_FULL, does: { does: "opens" } }
              : null
            : { kind: "url", target: one.text, does: { does: "opens" } },
        );
      }
      if (cmd === "pane_link_open") return { answer: "opened", kind: "url", target: ADDRESS };
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
 *  in full with its line, and that ⌘-click opens it in an editor tab. */
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
    await expect(tip).toHaveTextContent(FILE_IN_FULL);
    await expect(tip).toHaveTextContent(/-click opens it in an editor tab/);
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
          within: "smart-ide",
          folder: "/Users/you/projects/plane/workspaces/smart-ide",
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
