// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a list of rows that act leaves to what is in its rows** (#3123): a text box in a row
 * keeps the keys it types and moves its caret with, and a list can turn its typing-to-find off.
 * The Inbox's asks hold reply boxes and reasons, and a letter there is a word, never a search.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { Button } from "../button/Button";
import { TextArea, TextField } from "../field/Field";
import { List, ListRow, ListRowDrawn } from "./List";
import { ListFoldSection } from "./Fold";
import { Segment, SegmentedControl } from "../choice/Radio";
import { Select } from "../select/Select";
import { ComboBox } from "../select/ComboBox";
import { RowLabel } from "./Row";
import { aFramePassed } from "../../frame.testkit";

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

describe("a control in a row that takes the keyboard (#3123)", () => {
  it("makes its row the list's own, so the list never pulls the keyboard back to another row", async () => {
    render(<Asks />);
    // A screen reader's cursor presses with a click no pointer made (React Aria's "virtual"
    // modality), and what it opened takes the keyboard from a script, in a row not the first.
    act(() => screen.getByRole("button", { name: "Go to first" }).click());
    act(() => within(row("Held")).getByRole("button", { name: "Send" }).focus());
    await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
    await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
    expect(within(row("Held")).getByRole("button", { name: "Send" })).toHaveFocus();
    // And the list's one Tab stop is that row now.
    await userEvent.keyboard("{ArrowDown}");
    expect(row("Last")).toHaveFocus();
  });

  it("keeps the keyboard a script moved in from outside the list, in a row not the first (T1-39)", async () => {
    // As ⌘⇧I moves it from a chat's terminal past a high-risk first row (`asksTriage`): the
    // list has never had the keyboard, so it must not take it back to its first row.
    render(
      <>
        <textarea aria-label="Terminal" />
        <Asks />
      </>,
    );
    act(() => screen.getByRole("textbox", { name: "Terminal" }).focus());
    act(() => within(row("Held")).getByRole("button", { name: "Send" }).focus());
    await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
    await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
    expect(within(row("Held")).getByRole("button", { name: "Send" })).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(row("Last")).toHaveFocus();
  });
});

describe("a radio group in a row (#3123)", () => {
  function Fill() {
    const [how, setHow] = useState("value");
    return (
      <List aria-label="Asks" disallowTypeAhead>
        <ListRow id="secret" textValue="Fill the token">
          <RowLabel>Fill the token</RowLabel>
          <SegmentedControl aria-label="How to fill" value={how} onChange={setHow}>
            <Segment value="value">New value</Segment>
            <Segment value="entry">Use an entry I have</Segment>
          </SegmentedControl>
        </ListRow>
        <ListRow id="next" textValue="Run cargo test">
          <RowLabel>Run cargo test</RowLabel>
        </ListRow>
      </List>
    );
  }

  it("keeps its arrows: → and ↓ choose the next, ← and ↑ the one before, and the row stays", async () => {
    render(<Fill />);
    const radio = (name: string) => screen.getByRole("radio", { name });
    await userEvent.click(radio("New value"));
    await userEvent.keyboard("{ArrowRight}");
    expect(radio("Use an entry I have")).toHaveFocus();
    expect(radio("Use an entry I have")).toBeChecked();
    await userEvent.keyboard("{ArrowUp}");
    expect(radio("New value")).toHaveFocus();
    expect(radio("New value")).toBeChecked();
    await userEvent.keyboard("{ArrowDown}");
    expect(radio("Use an entry I have")).toBeChecked();
    expect(row("Fill the token").contains(document.activeElement)).toBe(true);
  });
});

describe("a key while a text box in a row composes (#5993)", () => {
  /** The Inbox's own Esc (`Inbox.tsx`'s `leave`) hears what comes up out of its lists. */
  function Composing({ onEscape, onLeave }: { onEscape: () => void; onLeave: () => void }) {
    return (
      <div onKeyDown={(event) => event.key === "Escape" && onLeave()}>
        <Asks onEscape={onEscape} />
      </div>
    );
  }

  it("gives Esc to the input method alone: the keyboard stays in the box, and nothing above hears it", async () => {
    const onEscape = vi.fn();
    const onLeave = vi.fn();
    render(<Composing onEscape={onEscape} onLeave={onLeave} />);
    await userEvent.click(reply());
    fireEvent.keyDown(reply(), { key: "Escape", isComposing: true });
    expect(reply()).toHaveFocus();
    expect(onEscape).not.toHaveBeenCalled();
    expect(onLeave).not.toHaveBeenCalled();
  });

  it("gives Esc of key code 229 to the input method alone, as WebKit sends it once composing ends", async () => {
    const onEscape = vi.fn();
    const onLeave = vi.fn();
    render(<Composing onEscape={onEscape} onLeave={onLeave} />);
    await userEvent.click(reply());
    fireEvent.keyDown(reply(), { key: "Escape", keyCode: 229 });
    expect(reply()).toHaveFocus();
    expect(onEscape).not.toHaveBeenCalled();
    expect(onLeave).not.toHaveBeenCalled();
  });

  it("keeps ↓ of key code 229 for the input method's candidates in a one-line box: the list stays on its row", async () => {
    render(<Asks />);
    await userEvent.click(reason());
    fireEvent.keyDown(reason(), { key: "ArrowDown", keyCode: 229 });
    expect(reason()).toHaveFocus();
  });
});

describe("a select or a combobox in a row (#5993)", () => {
  const KINDS = [
    { value: "value", label: "New value" },
    { value: "entry", label: "An entry I have" },
  ];

  function Picks() {
    const [kind, setKind] = useState("value");
    const [skill, setSkill] = useState("");
    return (
      <List aria-label="Asks" disallowTypeAhead>
        <ListRow id="first" textValue="First">
          <RowLabel>First</RowLabel>
        </ListRow>
        <ListRow id="picks" textValue="Fill the token">
          <RowLabel>Fill the token</RowLabel>
          <Select aria-label="How to fill" value={kind} options={KINDS} onValueChange={setKind} />
          <ComboBox
            aria-label="Skill"
            suggestions={["purlis:purlis-handoff", "purlis:purlis-theme"]}
            value={skill}
            onChange={setSkill}
          />
        </ListRow>
        <ListRow id="last" textValue="Last">
          <RowLabel>Last</RowLabel>
        </ListRow>
      </List>
    );
  }

  const trigger = () => screen.getByRole("button", { name: /How to fill/ });

  /** The keyboard comes to the select as a person brings it: into the list, a row down, → on. */
  async function onTheSelect() {
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    expect(trigger()).toHaveFocus();
  }

  it("opens a select on ↓, where the row would have gone to the row below", async () => {
    render(<Picks />);
    await onTheSelect();
    await userEvent.keyboard("{ArrowDown}");
    const list = await screen.findByRole("listbox");
    expect(within(list).getAllByRole("option")).toHaveLength(2);
    expect(screen.getByRole("row", { name: "Last", hidden: true })).not.toHaveFocus();
  });

  it("opens a select on ↑, where the row would have gone to the row above", async () => {
    render(<Picks />);
    await onTheSelect();
    await userEvent.keyboard("{ArrowUp}");
    await screen.findByRole("listbox");
    expect(screen.getByRole("row", { name: "First", hidden: true })).not.toHaveFocus();
  });

  // **The ↓ that opens it is its own, and never the list's** (#6091, `keepsItsArrows`).
  it("gives the keyboard back to its button once ↓ opened it and Enter took an option, and the list's row stays", async () => {
    render(<Picks />);
    await onTheSelect();
    await userEvent.keyboard("{ArrowDown}");
    const list = await screen.findByRole("listbox");
    await waitFor(() =>
      expect(within(list).getByRole("option", { name: "New value" })).toHaveFocus(),
    );
    await userEvent.keyboard("{ArrowDown}{Enter}");
    await waitFor(() => expect(trigger()).toHaveTextContent("An entry I have"));
    await aFramePassed();
    expect(trigger()).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}{ArrowDown}");
    expect(screen.getByRole("row", { name: "Last" })).toHaveFocus();
  });

  it("gives the keyboard back to its button once ↑ opened it and Esc closed it, and the list's row stays", async () => {
    render(<Picks />);
    await onTheSelect();
    await userEvent.keyboard("{ArrowUp}");
    const list = await screen.findByRole("listbox");
    await waitFor(() => expect(list).toContainElement(document.activeElement as HTMLElement));
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    await aFramePassed();
    expect(trigger()).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}{ArrowUp}");
    expect(screen.getByRole("row", { name: "First" })).toHaveFocus();
  });

  it("opens a combobox's suggestions on ↓, and the keyboard stays in its box", async () => {
    render(<Picks />);
    const box = screen.getByRole("combobox", { name: "Skill" });
    await userEvent.click(box);
    await userEvent.keyboard("{ArrowDown}");
    await screen.findByRole("listbox");
    await waitFor(() => expect(box).toHaveFocus());
  });
});

describe("a box of lines that wrap, in a row (#5993)", () => {
  /** Ten characters to a line, twenty pixels to it: the test's document draws no lines, so each
   *  run of text in the box's measure is laid out here as a browser would wrap it. */
  const PER_LINE = 10;
  function wrapped(this: Element): DOMRectList {
    const start = this.previousSibling?.textContent?.length ?? 0;
    const length = this.textContent?.length ?? 0;
    const rects: DOMRect[] = [];
    if (length > 0)
      for (
        let line = Math.floor(start / PER_LINE);
        line <= Math.floor((start + length - 1) / PER_LINE);
        line += 1
      )
        rects.push(new DOMRect(0, line * 20, 10, 20));
    return Object.assign(rects, {
      item: (at: number) => rects[at] ?? null,
    }) as unknown as DOMRectList;
  }

  afterEach(() => vi.restoreAllMocks());

  /** Two lines that wrap, and no line break: "aaaa bbbb " and "cccc dddd". */
  const TWO_LINES = "aaaa bbbb cccc dddd";

  async function typedAndAt(at: number) {
    vi.spyOn(HTMLSpanElement.prototype, "getClientRects").mockImplementation(wrapped);
    render(<Asks />);
    await userEvent.click(reply());
    await userEvent.keyboard(TWO_LINES);
    reply().setSelectionRange(at, at);
  }

  it("keeps ↑ on its second line, which wraps from the first", async () => {
    await typedAndAt(13);
    await userEvent.keyboard("{ArrowUp}");
    expect(reply()).toHaveFocus();
  });

  it("keeps ↓ on its first line, which wraps on to the second", async () => {
    await typedAndAt(3);
    await userEvent.keyboard("{ArrowDown}");
    expect(reply()).toHaveFocus();
  });

  it("gives ↑ to the list from its first line", async () => {
    await typedAndAt(3);
    await userEvent.keyboard("{ArrowUp}");
    expect(row("First")).toHaveFocus();
  });

  it("gives ↓ to the list from its last line", async () => {
    await typedAndAt(13);
    await userEvent.keyboard("{ArrowDown}");
    expect(row("Last")).toHaveFocus();
  });

  /** One word too long for a line, broken in it where nothing hangs: "aaaaaaaaaa" and "bbbbbbbbb". */
  const ONE_WORD = "aaaaaaaaaabbbbbbbbb";

  /**
   * The caret at a soft wrap (#6045): drawn at the end of the first line or the start of the
   * second, as the box chooses, and the box never says which.
   */
  async function wordAtTheWrap() {
    vi.spyOn(HTMLSpanElement.prototype, "getClientRects").mockImplementation(wrapped);
    render(<Asks />);
    await userEvent.click(reply());
    await userEvent.keyboard(ONE_WORD);
    reply().setSelectionRange(PER_LINE, PER_LINE);
  }

  it("keeps ↓ at a soft wrap, where the caret may be drawn at the end of the line above (#6045)", async () => {
    await wordAtTheWrap();
    await userEvent.keyboard("{ArrowDown}");
    expect(reply()).toHaveFocus();
  });

  it("keeps ↑ at a soft wrap, where the caret may be drawn at the start of the line below (#6045)", async () => {
    await wordAtTheWrap();
    await userEvent.keyboard("{ArrowUp}");
    expect(reply()).toHaveFocus();
  });
});

describe("Shift+Tab back into a list (#6033)", () => {
  /** Two rows, the last holding a box and a button, the list's last controls; a button after. */
  function Below({ drawn = false }: { drawn?: boolean }) {
    const [reason, setReason] = useState("");
    const held = (
      <>
        <TextField aria-label="Reason" value={reason} onChange={setReason} />
        <Button size="sm">Send</Button>
      </>
    );
    return (
      <>
        <List aria-label="Asks" disallowTypeAhead>
          <ListRow id="first" textValue="First">
            <RowLabel>First</RowLabel>
          </ListRow>
          {drawn ? (
            <ListRowDrawn id="held" textValue="Held" aria-labelledby="held-name">
              <span id="held-name">Held</span>
              {held}
            </ListRowDrawn>
          ) : (
            <ListRow id="held" textValue="Held">
              <RowLabel>Held</RowLabel>
              {held}
            </ListRow>
          )}
        </List>
        <button type="button">After</button>
      </>
    );
  }

  const after = () => screen.getByRole("button", { name: "After" });
  const send = () => screen.getByRole("button", { name: "Send" });

  /** Tab on until the keyboard has left the list for the button after it. */
  async function tabOut() {
    for (let press = 0; press < 4 && after() !== document.activeElement; press += 1)
      await userEvent.tab();
    expect(after()).toHaveFocus();
  }

  it.each([
    ["a row of the family's", false],
    ["a row another family draws", true],
  ])("lands on the row the keyboard left, not on its last control, in %s", async (_, drawn) => {
    render(<Below drawn={drawn} />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    expect(row("Held")).toHaveFocus();
    await tabOut();
    await userEvent.tab({ shift: true });
    expect(row("Held")).toHaveFocus();
  });

  it("lands on the row when the keyboard left from one of the row's controls", async () => {
    render(<Below />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}{ArrowRight}");
    expect(reason()).toHaveFocus();
    await tabOut();
    await userEvent.tab({ shift: true });
    expect(row("Held")).toHaveFocus();
  });

  it("lands on the last row of a list the keyboard has not been in", async () => {
    render(<Below />);
    await userEvent.click(after());
    await userEvent.tab({ shift: true });
    expect(row("Held")).toHaveFocus();
  });

  it("reaches the row's first control again with →", async () => {
    render(<Below />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    await tabOut();
    await userEvent.tab({ shift: true });
    await userEvent.keyboard("{ArrowRight}");
    expect(reason()).toHaveFocus();
  });

  it("leaves a control the pointer presses in that row where it is", async () => {
    render(<Below />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    await tabOut();
    await userEvent.click(send());
    expect(send()).toHaveFocus();
  });

  it("leaves a control a script focuses in that row where it is", async () => {
    render(<Below />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    await tabOut();
    act(() => send().focus());
    await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
    expect(send()).toHaveFocus();
  });
});

describe("Tab in a list (#6071)", () => {
  /** Three rows, each with two buttons of its own, between two buttons outside the list. */
  function Between() {
    return (
      <>
        <button type="button">Before</button>
        <List aria-label="Runs" disallowTypeAhead>
          {["alpha", "beta", "gamma"].map((name) => (
            <ListRow key={name} id={name} textValue={name}>
              <RowLabel>{name}</RowLabel>
              <Button size="sm">{`Open ${name}`}</Button>
              <Button size="sm">{`Stop ${name}`}</Button>
            </ListRow>
          ))}
        </List>
        <button type="button">After</button>
      </>
    );
  }

  const button = (name: string) => screen.getByRole("button", { name });

  /** The keyboard on the row `beta`, come to by the keyboard from the button before the list. */
  async function onBeta() {
    render(<Between />);
    await userEvent.click(button("Before"));
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    expect(row("beta")).toHaveFocus();
  }

  it("leaves the list with one Tab from a row, past every control in it", async () => {
    await onBeta();
    await userEvent.tab();
    expect(button("After")).toHaveFocus();
  });

  it("leaves the list with one Shift+Tab from a row, back to what comes before it", async () => {
    await onBeta();
    await userEvent.tab({ shift: true });
    expect(button("Before")).toHaveFocus();
  });

  it("moves on to a row's next control with Tab from one of its controls, and leaves from its last", async () => {
    await onBeta();
    await userEvent.keyboard("{ArrowRight}");
    expect(button("Open beta")).toHaveFocus();
    await userEvent.tab();
    expect(button("Stop beta")).toHaveFocus();
    await userEvent.tab();
    expect(button("After")).toHaveFocus();
  });

  it("moves back to a row's control before with Shift+Tab, and leaves backwards from its first", async () => {
    await onBeta();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    expect(button("Stop beta")).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(button("Open beta")).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(button("Before")).toHaveFocus();
  });

  it("leaves the list with Tab from a row's control when its next control is not drawn", async () => {
    // The test's document lays nothing out, nor says what is drawn: here a control is drawn
    // while it is not `hidden`.
    Object.defineProperty(HTMLElement.prototype, "checkVisibility", {
      configurable: true,
      value(this: HTMLElement) {
        return !this.hidden;
      },
    });
    try {
      await onBeta();
      button("Stop beta").hidden = true;
      await userEvent.keyboard("{ArrowRight}");
      expect(button("Open beta")).toHaveFocus();
      await userEvent.tab();
      expect(button("After")).toHaveFocus();
    } finally {
      delete (HTMLElement.prototype as Partial<HTMLElement>).checkVisibility;
    }
  });

  it("comes back to the row the keyboard left with Shift+Tab, after a Tab out from its last control", async () => {
    await onBeta();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    await userEvent.tab();
    expect(button("After")).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(row("beta")).toHaveFocus();
  });
});
