// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { Shown } from "./bindings";
import { WaitingStrip } from "./WaitingStrip";
import { STRIP } from "./waitingOn";

/**
 * **The waiting strip** (#4563): what a chat waiting on the person waits on, at the top of its
 * pane, from the asks registry the Inbox reads. What is asserted is what the person sees.
 */

afterEach(cleanup);

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

const strip = () => screen.getByRole("group", { name: STRIP.waitingOn });

describe("the waiting strip", () => {
  it("shows a real ask in its own words, and where to answer it", () => {
    render(<WaitingStrip session={CHAT} waiting={[ask("permission")]} />);
    expect(within(strip()).getByText("Run npm publish --tag next")).toBeVisible();
    expect(within(strip()).getByText(STRIP.waitingOn, { exact: false })).toBeVisible();
    expect(within(strip()).getByText(STRIP.answerBelow, { exact: false })).toBeVisible();
  });

  it("keeps a real ask's words whatever a later turn ended saying", () => {
    render(
      <WaitingStrip
        session={CHAT}
        waiting={[ask("question", { last_message: "Three tasks are at work." }), ask("permission")]}
      />,
    );
    expect(within(strip()).getByText("Run npm publish --tag next")).toBeVisible();
    expect(screen.queryByText("Three tasks are at work.")).toBeNull();
  });

  it("shows a prose wait's last message, and walks back through the few before it", async () => {
    const user = userEvent.setup();
    render(
      <WaitingStrip
        session={CHAT}
        waiting={[
          ask("question", {
            last_message: "Should I open the pull request now?",
            earlier_messages: ["First.", "Second.", "Third."],
          }),
        ]}
      />,
    );
    expect(within(strip()).getByText(STRIP.lastMessage)).toBeVisible();
    expect(within(strip()).getByText("Should I open the pull request now?")).toBeVisible();

    await user.click(screen.getByRole("button", { name: STRIP.earlier(3) }));
    expect(within(strip()).getByText(STRIP.earlierAt(1, 3))).toBeVisible();
    expect(within(strip()).getByText("Third.")).toBeVisible();
    // The keyboard stays in the strip, on the next step back.
    expect(screen.getByRole("button", { name: STRIP.previous })).toHaveFocus();

    await user.click(screen.getByRole("button", { name: STRIP.previous }));
    await user.click(screen.getByRole("button", { name: STRIP.previous }));
    expect(within(strip()).getByText("First.")).toBeVisible();
    // The first has none before it; the button stays a Tab stop, so the keyboard is not dropped.
    expect(screen.getByRole("button", { name: STRIP.previous })).toHaveAttribute(
      "aria-disabled",
      "true",
    );

    await user.click(screen.getByRole("button", { name: STRIP.next }));
    expect(within(strip()).getByText("Second.")).toBeVisible();

    await user.click(screen.getByRole("button", { name: STRIP.backToLast }));
    expect(within(strip()).getByText("Should I open the pull request now?")).toBeVisible();
    expect(screen.getByRole("button", { name: STRIP.earlier(3) })).toHaveFocus();
  });

  it("says plainly when the app heard no message", () => {
    render(<WaitingStrip session={CHAT} waiting={[ask("question")]} />);
    expect(within(strip()).getByText(STRIP.noMessage)).toBeVisible();
    expect(screen.queryByRole("button", { name: /earlier message/ })).toBeNull();
  });

  it("expands a long message to the whole of it, and folds it again", async () => {
    const user = userEvent.setup();
    const long = `${"The runner image pins the token at build time. ".repeat(4)}Which do you want?`;
    render(<WaitingStrip session={CHAT} waiting={[ask("question", { last_message: long })]} />);
    const all = screen.getByRole("button", { name: STRIP.showAll });
    expect(all).toHaveAttribute("aria-expanded", "false");
    await user.click(all);
    expect(screen.getByRole("button", { name: STRIP.showLess })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(strip()).toHaveAttribute("data-open");
    await user.click(screen.getByRole("button", { name: STRIP.showLess }));
    expect(strip()).not.toHaveAttribute("data-open");
  });

  it("offers no Show all on a message the line holds", () => {
    render(
      <WaitingStrip session={CHAT} waiting={[ask("question", { last_message: "main or dev?" })]} />,
    );
    expect(screen.queryByRole("button", { name: STRIP.showAll })).toBeNull();
  });

  it("is drawn for its own chat alone, and goes once nothing waits", () => {
    const { rerender, container } = render(
      <WaitingStrip session={CHAT} waiting={[ask("permission", { session: CHAT + 1 })]} />,
    );
    expect(container).toBeEmptyDOMElement();
    rerender(<WaitingStrip session={CHAT} waiting={[ask("permission")]} />);
    expect(strip()).toBeVisible();
    rerender(<WaitingStrip session={CHAT} waiting={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("never takes the keyboard when it comes", () => {
    const { rerender } = render(
      <>
        <textarea aria-label="Terminal input" />
        <WaitingStrip session={CHAT} waiting={[]} />
      </>,
    );
    const typing = screen.getByRole("textbox", { name: "Terminal input" });
    typing.focus();
    rerender(
      <>
        <textarea aria-label="Terminal input" />
        <WaitingStrip
          session={CHAT}
          waiting={[
            ask("question", {
              last_message: "A question long enough to be offered whole. ".repeat(3),
              earlier_messages: ["Before."],
            }),
          ]}
        />
      </>,
    );
    expect(strip()).toBeVisible();
    expect(typing).toHaveFocus();
  });

  it("goes back to the last message when a new message arrives", async () => {
    const user = userEvent.setup();
    const first = ask("question", { last_message: "One?", earlier_messages: ["Before."] });
    const { rerender } = render(<WaitingStrip session={CHAT} waiting={[first]} />);
    await user.click(screen.getByRole("button", { name: STRIP.earlier(1) }));
    expect(within(strip()).getByText("Before.")).toBeVisible();
    rerender(
      <WaitingStrip
        session={CHAT}
        waiting={[{ ...first, last_message: "Two?", earlier_messages: ["Before.", "One?"] }]}
      />,
    );
    expect(within(strip()).getByText("Two?")).toBeVisible();
    expect(within(strip()).getByText(STRIP.lastMessage)).toBeVisible();
  });
});
