// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a list of rows that act leaves to what is in its rows** (#3123): a text box in a row
 * keeps the keys it types and moves its caret with, and a list can turn its typing-to-find off.
 * The Inbox's asks hold reply boxes and reasons, and a letter there is a word, never a search.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { Button } from "../button/Button";
import { TextArea, TextField } from "../field/Field";
import { List, ListRow } from "./List";
import { ListFoldSection } from "./Fold";
import { RowLabel } from "./Row";

afterEach(cleanup);

/** Three rows: the middle one holds a one-line box, a box of lines and a button after them. */
function Asks({
  onAction = () => {},
  onEscape,
  onBoxKey = () => {},
}: {
  onAction?: (key: string) => void;
  onEscape?: () => void;
  onBoxKey?: (key: string) => void;
}) {
  const [reason, setReason] = useState("");
  const [reply, setReply] = useState("");
  return (
    <>
      <List
        aria-label="Asks"
        onAction={(key) => onAction(String(key))}
        onEscape={onEscape}
        disallowTypeAhead
      >
        <ListRow id="first" textValue="First">
          <RowLabel>First</RowLabel>
          <Button size="sm">Go to first</Button>
        </ListRow>
        <ListRow id="held" textValue="Held">
          <RowLabel>Held</RowLabel>
          <TextField aria-label="Reason" value={reason} onChange={setReason} />
          <TextArea
            aria-label="Reply"
            value={reply}
            onChange={setReply}
            onKeyDown={(event) => onBoxKey(event.key)}
          />
          <Button size="sm">Send</Button>
        </ListRow>
        <ListRow id="last" textValue="Last">
          <RowLabel>Last</RowLabel>
        </ListRow>
      </List>
      <button type="button">After</button>
    </>
  );
}

const row = (name: string) => screen.getByRole("row", { name });
const reason = () => screen.getByRole("textbox", { name: "Reason" }) as HTMLInputElement;
const reply = () => screen.getByRole("textbox", { name: "Reply" }) as HTMLTextAreaElement;

describe("a text box in a row (#3123)", () => {
  it("is reached by → from its row, as the row's buttons are", async () => {
    render(<Asks />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    expect(row("Held")).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(reason()).toHaveFocus();
  });

  it("keeps ← and → for its caret: the row's other controls are not walked to", async () => {
    render(<Asks />);
    await userEvent.click(reason());
    await userEvent.keyboard("abc{ArrowLeft}{ArrowLeft}X");
    expect(reason()).toHaveFocus();
    expect(reason().value).toBe("aXbc");
    await userEvent.keyboard("{ArrowRight}");
    expect(reason()).toHaveFocus();
  });

  it("keeps Home and End for its caret, and the list stays on its row", async () => {
    render(<Asks />);
    await userEvent.click(reason());
    await userEvent.keyboard("bc{Home}a{End}d");
    expect(reason()).toHaveFocus();
    expect(reason().value).toBe("abcd");
  });

  it("types a space, and its own Enter is its own: the row runs nothing", async () => {
    const onAction = vi.fn();
    const onBoxKey = vi.fn();
    render(<Asks onAction={onAction} onBoxKey={onBoxKey} />);
    await userEvent.click(reply());
    await userEvent.keyboard("yes please{Enter}");
    expect(reply().value).toContain("yes please");
    expect(onBoxKey).toHaveBeenCalledWith("Enter");
    expect(onAction).not.toHaveBeenCalled();
  });

  it("types a letter as a letter: no row is found by it", async () => {
    render(<Asks />);
    await userEvent.click(reason());
    await userEvent.keyboard("l");
    expect(reason()).toHaveFocus();
    expect(reason().value).toBe("l");
  });

  it("gives ↑ and ↓ to the list from a one-line box", async () => {
    render(<Asks />);
    await userEvent.click(reason());
    await userEvent.keyboard("{ArrowDown}");
    expect(row("Last")).toHaveFocus();
  });

  it("keeps ↑ and ↓ inside a box's lines, and gives them to the list from its first and last", async () => {
    render(<Asks />);
    await userEvent.click(reply());
    await userEvent.keyboard("one{Shift>}{Enter}{/Shift}two");
    expect(reply().value).toBe("one\ntwo");
    await userEvent.keyboard("{ArrowUp}");
    expect(reply()).toHaveFocus();
    // The browser moves the caret a line up; the test's document draws no lines, so it is put
    // there by hand.
    reply().setSelectionRange(1, 1);
    await userEvent.keyboard("{ArrowDown}");
    expect(reply()).toHaveFocus();
    reply().setSelectionRange(1, 1);
    await userEvent.keyboard("{ArrowUp}");
    expect(row("First")).toHaveFocus();
  });

  it("gives the keyboard back to its row on Esc, keeping what was typed; Esc on the row is the list's", async () => {
    const onEscape = vi.fn();
    render(<Asks onEscape={onEscape} />);
    await userEvent.click(reason());
    await userEvent.keyboard("kept{Escape}");
    expect(row("Held")).toHaveFocus();
    expect(onEscape).not.toHaveBeenCalled();
    expect(reason().value).toBe("kept");
    await userEvent.keyboard("{Escape}");
    expect(onEscape).toHaveBeenCalledOnce();
  });

  it("moves on to its row's next control with Tab, and back to the one before with Shift+Tab", async () => {
    render(<Asks />);
    await userEvent.click(reason());
    await userEvent.tab();
    expect(reply()).toHaveFocus();
    await userEvent.tab();
    expect(within(row("Held")).getByRole("button", { name: "Send" })).toHaveFocus();
    await userEvent.click(reply());
    await userEvent.tab({ shift: true });
    expect(reason()).toHaveFocus();
  });
});

describe("a list with typing-to-find off (#3123)", () => {
  it("moves nowhere on a letter", async () => {
    render(<Asks />);
    await userEvent.tab();
    expect(row("First")).toHaveFocus();
    await userEvent.keyboard("l");
    expect(row("First")).toHaveFocus();
  });

  it("folds on Space right after a letter, which found nothing", async () => {
    render(
      <List aria-label="Runs" disallowTypeAhead>
        <ListFoldSection id="runs" title="Two runs" count={2} defaultFolded>
          <ListRow id="a" textValue="Alpha">
            <RowLabel>Alpha</RowLabel>
          </ListRow>
          <ListRow id="b" textValue="Beta">
            <RowLabel>Beta</RowLabel>
          </ListRow>
        </ListFoldSection>
      </List>,
    );
    await userEvent.tab();
    const fold = screen.getByRole("row", { name: "Two runs, 2" });
    expect(fold).toHaveFocus();
    await userEvent.keyboard("t ");
    expect(fold).toHaveAttribute("data-expanded");
  });
});
