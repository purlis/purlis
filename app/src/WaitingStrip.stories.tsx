// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The waiting strip** (#4563): each state the reviewed prototype drew (#4561), and the strip
 * arriving over a real terminal, fitted as a chat's pane fits it, so its reflow is seen in
 * Chromium and WebKit: the terminal gives up the strip's line at its top, its input line stays
 * at its foot, and the keyboard stays where the person was typing.
 */
import { useEffect, useRef, useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import type { Shown } from "./bindings";
import { WaitingStrip } from "./WaitingStrip";
import { STRIP } from "./waitingOn";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { inForce, terminalFont, xtermLook, xtermTheme } from "./theme/theme";

const CHAT = 4;

function ask(source: Shown["source"], more: Partial<Shown> = {}): Shown {
  return {
    session: CHAT,
    ask: `${source}:${CHAT}`,
    says: source === "question" ? "Waiting on your reply" : "Run npm publish --tag next",
    options: [],
    source,
    chain: ["steward 4"],
    answer: { via: "in-its-pane" },
    ...more,
  };
}

const LONG =
  "The sign-in token for the staging pool expires tonight. The runner image pins it at build " +
  "time, so rotating it means rebuilding the image for every pool, which takes about forty " +
  "minutes each and blocks the release branch while it runs. I can rotate it now and hold the " +
  "release until the images are done, or leave it until after the release and run on the old " +
  "token for the two hours left. Which do you want?";

/**
 * The strip at the top of a pane, as big as one. A chat's pane is a size container whose size is
 * the layout's (`paneFrame.css`), so a pane given no height here would be none, and clip it.
 */
function InAPane({ waiting }: { waiting: readonly Shown[] }) {
  return (
    <div className="pane-frame pane-headed" style={{ inlineSize: 720, blockSize: 360 }}>
      <div className="pane-top">
        <WaitingStrip session={CHAT} waiting={waiting} />
      </div>
    </div>
  );
}

const meta = {
  component: InAPane,
} satisfies Meta<typeof InAPane>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A real ask, in its own words: answered in the terminal below, where it always was. */
export const ARealAsk: Story = {
  args: { waiting: [ask("permission")] },
  play: async ({ canvasElement }) => {
    const strip = within(canvasElement).getByRole("group", { name: STRIP.waitingOn });
    await expect(within(strip).getByText("Run npm publish --tag next")).toBeVisible();
    // Nothing in it to press: Tab passes it by.
    await expect(within(strip).queryAllByRole("button")).toHaveLength(0);
  },
};

/** A long message, opened to the whole of it and folded again from the keyboard. */
export const ALongMessageOpened: Story = {
  args: { waiting: [ask("question", { last_message: LONG })] },
  play: async ({ canvasElement }) => {
    const strip = within(canvasElement);
    await userEvent.tab();
    await untilFocused(() => strip.getByRole("button", { name: STRIP.showAll }), "Show all");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await untilFocused(() => strip.getByRole("button", { name: STRIP.showLess }), "Show less");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await untilFocused(() => strip.getByRole("button", { name: STRIP.showAll }), "folded");
    await userEvent.keyboard("{Enter}");
    await untilFocused(() => strip.getByRole("button", { name: STRIP.showLess }), "open again");
    expectFocusInSight();
  },
};

/** Prose: the message the chat's turn ended on, and how many came before it. */
export const ItsLastMessage: Story = {
  args: {
    waiting: [
      ask("question", {
        last_message:
          "The branch is ready and every check passes. Should I open the pull request now?",
        earlier_messages: [
          "Dispatched developer 7, designer 3 and architect 2.",
          "Two tests failed on the old wording. I changed the tests, not the code.",
          "developer 7 reported: the row words are in.",
        ],
      }),
    ],
  },
};

/** Walking back through the earlier messages from the keyboard, one line each, and back. */
export const AnEarlierMessage: Story = {
  args: ItsLastMessage.args,
  play: async ({ canvasElement }) => {
    const strip = within(canvasElement);
    const height = canvasElement.querySelector(".waiting-strip")?.getBoundingClientRect().height;
    await userEvent.tab();
    await untilFocused(() => strip.getByRole("button", { name: STRIP.earlier(3) }), "earlier");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await untilFocused(() => strip.getByRole("button", { name: STRIP.previous }), "previous");
    expectFocusInSight();
    await expect(strip.getByText(STRIP.earlierAt(1, 3))).toBeVisible();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(strip.getByText(STRIP.earlierAt(3, 3))).toBeVisible());
    await expect(strip.getByText(/Dispatched developer 7/)).toBeVisible();
    // Walking them never changes the strip's height, so the terminal is never refitted for it.
    await expect(
      canvasElement.querySelector(".waiting-strip")?.getBoundingClientRect().height,
    ).toBe(height);
    await userEvent.tab();
    await untilFocused(() => strip.getByRole("button", { name: STRIP.next }), "next");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => strip.getByRole("button", { name: STRIP.backToLast }), "back");
    await userEvent.keyboard("{Enter}");
    await untilFocused(() => strip.getByRole("button", { name: STRIP.earlier(3) }), "the last");
    expectFocusInSight();
  },
};

/** The app heard no message (its harness gave none, or purlis started since): said once. */
export const NoMessageKept: Story = {
  args: { waiting: [ask("question")] },
};

/**
 * **The strip arrives over a real terminal** while the person types in it: the terminal is
 * fitted as a chat's pane fits it (`FitAddon`, on its host's `ResizeObserver`), the strip takes
 * its line at the top, the terminal gives up rows there, its input line stays at its foot, and
 * what the person types goes on reaching the terminal.
 */
function OverATerminal() {
  const host = useRef<HTMLDivElement>(null);
  const [waiting, setWaiting] = useState<readonly Shown[]>([]);
  useEffect(() => {
    const where = host.current;
    if (where === null) return;
    const pane = new Terminal({
      fontFamily: terminalFont(),
      theme: xtermTheme(inForce()),
      ...xtermLook(inForce()),
    });
    const fitting = new FitAddon();
    pane.loadAddon(fitting);
    pane.open(where);
    fitting.fit();
    where.dataset.rowsBefore = String(pane.rows);
    for (let line = 0; line < 60; line += 1) pane.writeln(`● step ${line}: read and checked`);
    pane.write("❯ pnpm te");
    let typed = "";
    pane.onData((data) => {
      typed += data;
      where.dataset.typed = typed;
      pane.write(data);
    });
    pane.focus();
    const watching = new ResizeObserver(() => {
      fitting.fit();
      where.dataset.rows = String(pane.rows);
      // The line the person types on is the terminal's last, before the strip and after it.
      where.dataset.inputAtFoot = String(pane.buffer.active.cursorY === pane.rows - 1);
    });
    watching.observe(where);
    // The question arrives once the person is typing: after the terminal's first fit.
    where.dataset.ready = "";
    const arrive = () =>
      setWaiting([
        ask("question", {
          last_message: "The recorded fixture disagrees with the new row word. Re-record it?",
        }),
      ]);
    where.addEventListener("arrive", arrive);
    return () => {
      where.removeEventListener("arrive", arrive);
      watching.disconnect();
      pane.dispose();
    };
  }, []);
  return (
    <div
      className="pane-frame pane-headed"
      style={{ inlineSize: 720, blockSize: 360, display: "flex", flexDirection: "column" }}
    >
      <div className="pane-top">
        <WaitingStrip session={CHAT} waiting={waiting} />
      </div>
      <div ref={host} data-testid="terminal" style={{ flex: 1, minBlockSize: 0 }} />
    </div>
  );
}

export const OverARealTerminal: StoryObj<typeof OverATerminal> = {
  render: () => <OverATerminal />,
  play: async ({ canvasElement }) => {
    const host = within(canvasElement).getByTestId("terminal");
    await waitFor(() => expect(host.dataset.rowsBefore).toBeDefined());
    const typing = host.querySelector<HTMLTextAreaElement>(".xterm-helper-textarea");
    await untilFocused(() => typing, "the terminal has the keyboard");
    const frame = canvasElement.querySelector(".pane-frame") as HTMLElement;
    // Measured against the pane, not the page: the story's page may scroll as the frame
    // changes, and what is claimed is where the terminal stands in its pane (#4708's run).
    const paneBefore = frame.getBoundingClientRect().height;
    const footGap = () =>
      frame.getBoundingClientRect().bottom - host.getBoundingClientRect().bottom;
    await expect(footGap()).toBe(0);

    host.dispatchEvent(new Event("arrive"));
    const strip = await within(canvasElement).findByRole("group", { name: STRIP.waitingOn });
    await expect(strip).toBeVisible();

    // The terminal refits to the room the strip leaves: fewer rows, its foot where it was.
    await waitFor(() =>
      expect(Number(host.dataset.rows)).toBeLessThan(Number(host.dataset.rowsBefore)),
    );
    await expect(frame.getBoundingClientRect().height).toBe(paneBefore);
    await expect(footGap()).toBe(0);
    await expect(host.dataset.inputAtFoot).toBe("true");
    // The strip took nothing from the person: the keyboard is where it was, and typing goes on.
    await untilFocused(() => typing, "the keyboard stays in the terminal");
    await userEvent.keyboard("st");
    await waitFor(() => expect(host.dataset.typed).toBe("st"));
  },
};
