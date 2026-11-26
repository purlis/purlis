// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **An alert the app raises, with no press of the person's** (#7728, ADR 0104 as amended
 * 2026-11-26): the dialog family's `RaisedAlert`, held to what the amendment asks of it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState, type ReactNode } from "react";
import { Button } from "../button/Button";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "../menu/Menu";
import { Select } from "../select/Select";
import { selectNamed } from "../select/select.testkit";
import { Dialog, DialogSurface } from "./Dialog";
import { answersOf } from "./dialog.testkit";
import {
  RAISED_ALERT_CHANGED,
  RAISED_ALERT_TOO_SOON,
  RaisedAlert,
  type RaisedAlertProps,
  type RaisedAnswer,
} from "./RaisedAlert";
import { SETTLE_MS } from "../../noticeSettle";

/** The settle guard's clock, held: each test says when time passes. */
let clock = 10_000;
const passes = (ms: number) => {
  clock += ms;
};

beforeEach(() => {
  clock = 10_000;
  vi.spyOn(performance, "now").mockImplementation(() => clock);
});

afterEach(() => {
  cleanup();
  // A window that loses focus forgets every key and composition it heard: the next test's
  // window starts with none.
  fireEvent.blur(window);
  vi.restoreAllMocks();
});

/** A raised alert as a screen writes one: Keep leads and grants nothing, Accept grants. */
function Asked({
  keep = () => {},
  accept = () => {},
  ...rest
}: Partial<Omit<RaisedAlertProps, "cancel" | "main">> & {
  keep?: RaisedAnswer["onPress"];
  accept?: RaisedAnswer["onPress"];
}) {
  return (
    <RaisedAlert
      raised
      shows="digest-1"
      title="Policy needs your answer"
      why="Until you answer, your chats keep the policy you accepted before."
      cancel={{ children: "Keep my current rules", onPress: keep }}
      main={{ children: "Accept", variant: "grants", onPress: accept }}
      {...rest}
    />
  );
}

const alertNamed = (name = "Policy needs your answer") => screen.getByRole("alertdialog", { name });
const answer = (name: string) => screen.getByRole("button", { name });

/** Waits until the alert has put the keyboard on its first answer, as it does once it opens. */
async function opened(name?: string) {
  const alert = await screen.findByRole("alertdialog", name === undefined ? {} : { name });
  await waitFor(() => expect(answer("Keep my current rules")).toHaveFocus());
  return alert;
}

describe("an alert the app raises", () => {
  it("answers no press within the settle interval after it is drawn, and one after it", async () => {
    const accepted = vi.fn();
    render(<Asked accept={accepted} />);
    await opened();
    await userEvent.click(answer("Accept"));
    expect(accepted).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toHaveTextContent(RAISED_ALERT_TOO_SOON);
    passes(SETTLE_MS);
    await userEvent.click(answer("Accept"));
    expect(accepted).toHaveBeenCalledTimes(1);
  });

  it("answers no key within the settle interval: Return, Space and Esc land on nothing", async () => {
    const kept = vi.fn();
    render(<Asked keep={kept} />);
    await opened();
    await userEvent.keyboard("{Enter}{ }{Escape}");
    expect(kept).not.toHaveBeenCalled();
    passes(SETTLE_MS);
    await userEvent.keyboard("{Enter}");
    expect(kept).toHaveBeenCalledTimes(1);
  });

  it("does not open while a key is down, and answers nothing with that key once it does", async () => {
    const kept = vi.fn();
    fireEvent.keyDown(document.body, { key: "Enter", code: "Enter" });
    render(<Asked keep={kept} />);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    fireEvent.keyUp(document.body, { key: "Enter", code: "Enter" });
    await opened();
    // The same key, still held by the system's repeat as it opened: nothing.
    passes(SETTLE_MS);
    fireEvent.keyDown(answer("Keep my current rules"), {
      key: "Enter",
      code: "Enter",
      repeat: true,
    });
    expect(kept).not.toHaveBeenCalled();
  });

  it("does not open during an IME composition, and opens once it ends", async () => {
    const field = document.createElement("input");
    document.body.append(field);
    try {
      fireEvent.compositionStart(field);
      render(<Asked />);
      expect(screen.queryByRole("alertdialog")).toBeNull();
      fireEvent.compositionEnd(field);
      await opened();
    } finally {
      field.remove();
    }
  });

  it("says a change of what it shows, draws it, arms the guard again and takes the keyboard off its main answer", async () => {
    const accepted = vi.fn();
    const { rerender } = render(<Asked accept={accepted}>Two branches loosen.</Asked>);
    await opened();
    passes(SETTLE_MS);
    answer("Accept").focus();
    expect(answer("Accept")).toHaveFocus();
    rerender(
      <Asked accept={accepted} shows="digest-2">
        Three branches loosen.
      </Asked>,
    );
    expect(alertNamed()).toHaveTextContent("Three branches loosen.");
    expect(screen.getByRole("status")).toHaveTextContent(RAISED_ALERT_CHANGED);
    expect(answer("Keep my current rules")).toHaveFocus();
    await userEvent.click(answer("Accept"));
    expect(accepted).not.toHaveBeenCalled();
    passes(SETTLE_MS);
    await userEvent.click(answer("Accept"));
    expect(accepted).toHaveBeenCalledTimes(1);
  });

  it("does not take a redraw with the same content for a change", async () => {
    const { rerender } = render(<Asked />);
    await opened();
    passes(SETTLE_MS);
    rerender(<Asked />);
    expect(screen.getByRole("status")).toHaveTextContent("");
  });
});

describe("an alert the app raises, while another layer is up", () => {
  /** A screen with a layer of its own up, and an alert raised beside it. */
  function Beside({ layer }: { layer: (close: () => void) => ReactNode }) {
    const [up, setUp] = useState(true);
    return (
      <>
        <Button onPress={() => setUp(true)}>Open</Button>
        {up && layer(() => setUp(false))}
        <Asked />
      </>
    );
  }

  it("waits for another dialog to close", async () => {
    render(
      <Beside
        layer={(close) => (
          <Dialog onClose={close} title="Rename workspace">
            <input aria-label="Name" />
          </Dialog>
        )}
      />,
    );
    await screen.findByRole("dialog", { name: "Rename workspace" });
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await userEvent.keyboard("{Escape}");
    await opened();
  });

  it("waits for the palette to close", async () => {
    render(
      <Beside
        layer={(close) => (
          <DialogSurface onClose={close} aria-label="Command palette">
            <input aria-label="Run an action" />
          </DialogSurface>
        )}
      />,
    );
    await screen.findByRole("dialog", { name: "Command palette" });
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await userEvent.keyboard("{Escape}");
    await opened();
  });

  it("waits for a menu to close", async () => {
    function WithMenu() {
      const [open, setOpen] = useState(true);
      return (
        <>
          <MenuTrigger isOpen={open} onOpenChange={setOpen}>
            <Button>More</Button>
            <Menu aria-label="More">
              <MenuItem id="rename" textValue="Rename">
                <MenuWords said="Rename" />
              </MenuItem>
            </Menu>
          </MenuTrigger>
          <Asked />
        </>
      );
    }
    render(<WithMenu />);
    await screen.findByRole("menu");
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await opened();
  });

  it("waits for a select's list to close", async () => {
    function WithSelect({ asked }: { asked: boolean }) {
      return (
        <>
          <Select
            aria-label="Kind"
            value="code"
            options={[
              { value: "code", label: "Code" },
              { value: "notes", label: "Notes" },
            ]}
            onValueChange={() => {}}
          />
          {asked && <Asked />}
        </>
      );
    }
    const { rerender } = render(<WithSelect asked={false} />);
    await userEvent.click(selectNamed("Kind"));
    await screen.findByRole("listbox");
    // Raised by the app, as the core's event raises one.
    rerender(<WithSelect asked />);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    await opened();
  });
});

describe("alerts the app raises, several at once", () => {
  it("shows them one after the other, never stacked, each saying where it stands", async () => {
    function Two() {
      const [first, setFirst] = useState(true);
      return (
        <>
          <Asked
            raised={first}
            title="First policy"
            keep={() => setFirst(false)}
            data-testid="first"
          />
          <Asked title="Second policy" data-testid="second" />
        </>
      );
    }
    render(<Two />);
    const first = await opened("First policy");
    expect(screen.getAllByRole("alertdialog")).toHaveLength(1);
    expect(first).toHaveTextContent("1 of 2");
    passes(SETTLE_MS);
    await userEvent.click(answer("Keep my current rules"));
    const second = await screen.findByRole("alertdialog", { name: "Second policy" });
    expect(screen.getAllByRole("alertdialog")).toHaveLength(1);
    expect(second).toHaveTextContent("2 of 2");
  });

  it("says no count when it is the only one", async () => {
    render(<Asked />);
    const alert = await opened();
    expect(alert).not.toHaveTextContent("of 1");
  });
});

describe("what a raised alert gives a screen reader and the keyboard", () => {
  it("is busy while a press is in flight, and takes no second press meanwhile", async () => {
    let land: () => void = () => {};
    const accepted = vi.fn(() => new Promise<void>((done) => (land = done)));
    render(<Asked accept={accepted} />);
    const alert = await opened();
    passes(SETTLE_MS);
    expect(alert).not.toHaveAttribute("aria-busy", "true");
    await userEvent.click(answer("Accept"));
    expect(alert).toHaveAttribute("aria-busy", "true");
    await userEvent.click(answer("Accept"));
    expect(accepted).toHaveBeenCalledTimes(1);
    await act(async () => land());
    expect(alert).not.toHaveAttribute("aria-busy", "true");
  });

  it("is described by its why-line and its count", async () => {
    render(
      <>
        <Asked title="First policy" />
        <Asked title="Second policy" />
      </>,
    );
    const alert = await opened("First policy");
    expect(alert).toHaveAccessibleDescription(
      "1 of 2 Until you answer, your chats keep the policy you accepted before.",
    );
  });

  it("lets the keyboard reach a body that scrolls, and only one that does", async () => {
    const tall = vi
      .spyOn(HTMLElement.prototype, "scrollHeight", "get")
      .mockImplementation(function (this: HTMLElement) {
        return this.dataset.part === "body" ? 800 : 0;
      });
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function (
      this: HTMLElement,
    ) {
      return this.dataset.part === "body" ? 200 : 0;
    });
    const { rerender } = render(<Asked>A long list</Asked>);
    const alert = await opened();
    const body = alert.querySelector<HTMLElement>('[data-scope="dialog"][data-part="body"]');
    expect(body).toHaveAttribute("tabindex", "0");
    expect(body).toHaveAccessibleName("Policy needs your answer");
    tall.mockImplementation(() => 0);
    rerender(<Asked shows="digest-2">A short list</Asked>);
    await waitFor(() => expect(body).not.toHaveAttribute("tabindex"));
  });

  it("starts on its answer that grants nothing, which Esc chooses, and hands the keyboard back", async () => {
    const message = () => screen.getByRole("textbox", { name: "Message" });
    // The person is typing when it is raised.
    function Typing() {
      const [raised, setRaised] = useState(false);
      return (
        <>
          <input aria-label="Message" onChange={() => setRaised(true)} />
          <Asked raised={raised} keep={() => setRaised(false)} />
        </>
      );
    }
    render(<Typing />);
    message().focus();
    fireEvent.change(message(), { target: { value: "a" } });
    const alert = await opened();
    expect(answersOf(alert)).toEqual(["Keep my current rules", "Accept"]);
    passes(SETTLE_MS);
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await waitFor(() => expect(message()).toHaveFocus());
  });

  it("never presses a main answer that grants with Return", async () => {
    const accepted = vi.fn();
    render(
      <Asked accept={accepted}>
        <input aria-label="Note" />
      </Asked>,
    );
    await opened();
    passes(SETTLE_MS);
    screen.getByRole("textbox", { name: "Note" }).focus();
    await userEvent.keyboard("{Enter}");
    expect(accepted).not.toHaveBeenCalled();
  });
});
