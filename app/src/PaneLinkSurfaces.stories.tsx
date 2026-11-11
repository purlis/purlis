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
import { expect, waitFor, within } from "storybook/test";
import { LinkAnswer } from "./PaneLinkSurfaces";
import { LINK_CORE } from "./paneLinkCore";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { untilFocused } from "./stories.testkit";

const ADDRESS = "https://github.com/purlis/purlis/pull/6148";

/** A harness's turn, as the prototype's pane holds it. */
const TURN = [
  "\u001b[1m● The resolver's tests pass.\u001b[0m",
  `  It follows ${ADDRESS} (issue #5820).`,
  "\u001b[2m  ⎿ 12 tests, 0 failed\u001b[22m",
].join("\r\n");

const COLS = 72;
const ROWS = 5;

function Pane() {
  return (
    <div style={{ inlineSize: 640, paddingBlockEnd: "var(--space-8)" }}>
      <ReadOnlyTerminal
        screen={TURN}
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
        const { candidates } = args as { candidates: { text: string }[] };
        return candidates.map((one) => ({
          kind: "url",
          target: one.text,
          does: { does: "opens" },
        }));
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

/** **A link the core would not open**: the sentence that says why, and Close. */
export const Refused: StoryObj<typeof meta> = {
  render: () => (
    <LinkAnswer
      answering={{
        answer: {
          answer: "refused",
          reason: "purlis will not open an address that carries a name or a password",
        },
        candidate: { kind: "url", text: "https://purlis.dev/account" },
      }}
      onClose={() => {}}
      onConfirm={() => {}}
    />
  ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const close = await page.findByRole("button", { name: "Close" });
    await untilFocused(() => close, "It starts on Close");
    expect(page.getByRole("dialog", { name: "Link not opened" })).toHaveTextContent(
      "Purlis will not open an address that carries a name or a password.",
    );
  },
};
