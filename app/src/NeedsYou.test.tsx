import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { NeedsYouButton, type Quiet } from "./NeedsYou";

afterEach(cleanup);

/**
 * **The title bar's hand** (charter-app#249, #1692, #1695): the count of what waits on the
 * person, and the Inbox's way in. The list it dropped retired into the Inbox (#1700), which
 * lists from the asks registry alone.
 */
describe("the title bar's needs-you hand", () => {
  it("draws nothing when nothing needs you", () => {
    const { container } = render(<NeedsYouButton count={0} quiet={[]} onInbox={() => {}} />);

    expect(screen.queryByRole("button")).toBeNull();
    expect(container).toHaveTextContent("");
  });

  it("says how many things wait on you, in the count and in words", () => {
    render(<NeedsYouButton count={2} quiet={[]} onInbox={() => {}} />);

    const button = screen.getByRole("button", { name: "2 things wait on you" });
    expect(button).toHaveTextContent("2");
    expect(button).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("button", { name: "2 things wait on you" })).toBeInTheDocument();
    render(<NeedsYouButton count={1} quiet={[]} onInbox={() => {}} />);
    expect(screen.getByRole("button", { name: "1 thing waits on you" })).toBeInTheDocument();
  });

  it("says it counts chats where the asks registry has said nothing yet", () => {
    render(<NeedsYouButton count={2} chats quiet={[]} onInbox={() => {}} />);

    expect(screen.getByRole("button", { name: "2 chats need you" })).toBeInTheDocument();
  });

  it("opens the Inbox on a press, and pops up no list of its own", async () => {
    const onInbox = vi.fn();
    render(<NeedsYouButton count={1} quiet={[]} onInbox={onInbox} />);
    const hand = screen.getByRole("button", { name: "1 thing waits on you" });

    expect(hand).not.toHaveAttribute("aria-haspopup");
    await userEvent.click(hand);

    expect(onInbox).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("opens the Inbox from the keyboard too", async () => {
    const onInbox = vi.fn();
    render(<NeedsYouButton count={1} quiet={[]} onInbox={onInbox} />);
    screen.getByRole("button", { name: "1 thing waits on you" }).focus();

    await userEvent.keyboard("{Enter}");

    expect(onInbox).toHaveBeenCalledTimes(1);
  });

  it("hands the keyboard to the next control in the bar when the last ask goes", async () => {
    // The button goes with the last ask, and focus on an element that goes is focus on the
    // page, where the next key does nothing.
    const bar = (count: number) => (
      <header>
        <NeedsYouButton count={count} quiet={[]} onInbox={() => {}} />
        <button type="button" tabIndex={0}>
          About
        </button>
      </header>
    );
    const { rerender } = render(bar(1));
    screen.getByRole("button", { name: "1 thing waits on you" }).focus();

    rerender(bar(0));

    await waitFor(() => expect(screen.getByRole("button", { name: "About" })).toHaveFocus());
  });
});

describe("the muted hand: a chat that cannot say it is waiting (charter-app#52, #249)", () => {
  const quiet: Quiet[] = [{ name: "shell 2", project: "charter" }];

  it("shows a faint hand with no number when only a chat that cannot report is open", () => {
    render(<NeedsYouButton count={0} quiet={quiet} onInbox={() => {}} />);

    const hand = screen.getByRole("button", {
      name: "Nothing has asked for you, but shell 2 can't tell purlis it's waiting",
    });
    expect(hand).toHaveClass("muted");
    expect(hand).toHaveAttribute("title", hand.getAttribute("aria-label"));
    expect(hand).toHaveTextContent(/^$/);
    expect(hand).toHaveAttribute("tabindex", "0");
  });

  it("counts several such chats in its name", () => {
    render(
      <NeedsYouButton
        count={0}
        quiet={[...quiet, { name: "codex 4", project: "ops" }]}
        onInbox={() => {}}
      />,
    );

    expect(
      screen.getByRole("button", {
        name: "Nothing has asked for you, but 2 chats can't tell purlis they're waiting",
      }),
    ).toBeInTheDocument();
  });

  it("is the normal hand with the count once something really asks", () => {
    render(<NeedsYouButton count={1} quiet={quiet} onInbox={() => {}} />);

    const hand = screen.getByRole("button", { name: "1 thing waits on you" });
    expect(hand).not.toHaveClass("muted");
    expect(hand).toHaveTextContent("1");
  });
});

/**
 * **The count's colour is only ever on a pair the contrast suite measures.**
 *
 * `needs-you.base` is `#b85050` in charter-dark, which is 3.64:1 on `surface.base` — under AA
 * for words. So the button's own colour is `text.primary`, and the number is
 * `needs-you.text` FILLED with `needs-you.base`, which is the pair `contrast.test.ts` holds at
 * 4.5:1. jsdom computes no colour, so this reads the rules.
 */
describe("the needs-you count's colours", () => {
  const css = readFileSync(join(process.cwd(), "src/App.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );
  const rule = (selector: string) =>
    new RegExp(
      `(?:^|\\})\\s*${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`,
    ).exec(css)?.[1] ?? "";

  it("never writes the button's words in needs-you.base", () => {
    expect(rule(".needs-you-button")).toMatch(/(?:^|[;\s])color:\s*var\(--text-primary\)/);
    expect(rule(".needs-you-button")).not.toMatch(/(?:^|[;\s])color:\s*var\(--needs-you-base\)/);
  });

  it("draws the faint hand in the muted text token, and no colour of its own", () => {
    expect(rule(".needs-you-button.muted,\n.needs-you-button.muted svg")).toMatch(
      /^\s*color:\s*var\(--text-muted\);\s*$/,
    );
  });

  it("draws no coloured border on the button (charter-app#249)", () => {
    expect(rule(".needs-you-button")).toMatch(/border:\s*none/);
    expect(rule(".needs-you-button")).not.toMatch(/border[a-z-]*:[^;]*var\(--(?!border-subtle)/);
  });

  it("fills the number with needs-you.base under needs-you.text, the measured pair", () => {
    expect(rule(".needs-you-number")).toMatch(/background:\s*var\(--needs-you-base\)/);
    expect(rule(".needs-you-number")).toMatch(/(?:^|[;\s])color:\s*var\(--needs-you-text\)/);
  });
});
