// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **An alert the app raises, with no press of the person's** (#7728, ADR 0104 as amended
 * 2026-11-26): the dialog family's `RaisedAlert`, held to what the amendment asks of it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { StrictMode, useState, type ReactNode } from "react";
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
import { ProjectShownProvider } from "../../projectShown";
import { KEY_HELD_MS } from "./raised";

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
    // And its release, which presses what a keydown started: none did.
    fireEvent.keyUp(answer("Keep my current rules"), { key: "Enter", code: "Enter" });
    expect(kept).not.toHaveBeenCalled();
  });

  it("does not open during an IME composition, opens once it ends, and takes the Return that ends it for no answer", async () => {
    const kept = vi.fn();
    const field = document.createElement("input");
    document.body.append(field);
    try {
      fireEvent.compositionStart(field);
      render(<Asked keep={kept} />);
      expect(screen.queryByRole("alertdialog")).toBeNull();
      fireEvent.compositionEnd(field);
      await opened();
      // WebKit sends the Return that committed the composition after its compositionend.
      fireEvent.keyDown(answer("Keep my current rules"), { key: "Enter", code: "Enter" });
      fireEvent.keyUp(answer("Keep my current rules"), { key: "Enter", code: "Enter" });
      expect(kept).not.toHaveBeenCalled();
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

  it("says each change anew, though the words are the same", async () => {
    const { rerender } = render(<Asked />);
    await opened();
    const status = screen.getByRole("status");
    rerender(<Asked shows="digest-2" />);
    const second = status.firstElementChild;
    expect(second).toHaveTextContent(RAISED_ALERT_CHANGED);
    rerender(<Asked shows="digest-3" />);
    expect(status.firstElementChild).toHaveTextContent(RAISED_ALERT_CHANGED);
    expect(status.firstElementChild).not.toBe(second);
  });

  it("says each press too soon anew", async () => {
    render(<Asked />);
    await opened();
    const status = screen.getByRole("status");
    await userEvent.click(answer("Accept"));
    const first = status.firstElementChild;
    expect(first).toHaveTextContent(RAISED_ALERT_TOO_SOON);
    await userEvent.click(answer("Accept"));
    expect(status.firstElementChild).toHaveTextContent(RAISED_ALERT_TOO_SOON);
    expect(status.firstElementChild).not.toBe(first);
  });

  it("puts the keyboard on the alert itself on a change, when its answer that grants nothing cannot be pressed", async () => {
    const Disabled = ({ shows }: { shows: string }) => (
      <RaisedAlert
        raised
        shows={shows}
        title="Policy needs your answer"
        why="Until you answer, your chats keep the policy you accepted before."
        cancel={{ children: "Keep my current rules", isDisabled: true, onPress: () => {} }}
        main={{ children: "Accept", variant: "grants", onPress: () => {} }}
      />
    );
    const { rerender } = render(<Disabled shows="digest-1" />);
    const alert = await screen.findByRole("alertdialog");
    await waitFor(() => expect(alert).toHaveFocus());
    answer("Accept").focus();
    rerender(<Disabled shows="digest-2" />);
    expect(alert).toHaveFocus();
  });

  it("lets a key held down repeat in a field inside it, and on nothing else", async () => {
    render(
      <Asked>
        <input aria-label="Note" />
      </Asked>,
    );
    await opened();
    passes(SETTLE_MS);
    const note = screen.getByRole("textbox", { name: "Note" });
    note.focus();
    expect(fireEvent.keyDown(note, { key: " ", code: "Space", repeat: true })).toBe(true);
    expect(
      fireEvent.keyDown(answer("Keep my current rules"), { key: " ", code: "Space", repeat: true }),
    ).toBe(false);
  });

  it("opens once a key with no keyup is forgotten, though the timer fires early", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      fireEvent.keyDown(document.body, { key: "a", code: "KeyA" });
      render(<Asked />);
      expect(screen.queryByRole("alertdialog")).toBeNull();
      // The timer runs a little before the clock says the key is forgotten.
      passes(KEY_HELD_MS - 5);
      act(() => void vi.advanceTimersByTime(KEY_HELD_MS));
      expect(screen.queryByRole("alertdialog")).toBeNull();
      passes(10);
      act(() => void vi.advanceTimersByTime(KEY_HELD_MS));
      expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("opens once the field an IME composition began in is taken away", async () => {
    const field = document.createElement("input");
    document.body.append(field);
    fireEvent.compositionStart(field);
    render(<Asked />);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    field.remove();
    await opened();
  });

  it("is the same one alert under React's strict mode", async () => {
    const kept = vi.fn();
    render(
      <StrictMode>
        <Asked keep={kept} />
      </StrictMode>,
    );
    const alert = await opened();
    expect(screen.getAllByRole("alertdialog")).toHaveLength(1);
    expect(alert).not.toHaveTextContent("of 2");
    passes(SETTLE_MS);
    await userEvent.click(answer("Keep my current rules"));
    expect(kept).toHaveBeenCalledTimes(1);
  });
});

describe("an alert the app raises in a project that is not in front", () => {
  it("waits, and lets one in front open meanwhile", async () => {
    render(
      <>
        <ProjectShownProvider value={false}>
          <Asked title="Behind policy" />
        </ProjectShownProvider>
        <Asked title="Front policy" />
      </>,
    );
    const front = await opened("Front policy");
    expect(screen.getAllByRole("alertdialog")).toHaveLength(1);
    expect(front).not.toHaveTextContent("of 2");
  });

  it("arms its settle guard when its project comes in front, not before", async () => {
    const accepted = vi.fn();
    const Behind = ({ shown }: { shown: boolean }) => (
      <ProjectShownProvider value={shown}>
        <Asked accept={accepted} />
      </ProjectShownProvider>
    );
    const { rerender } = render(<Behind shown={false} />);
    passes(SETTLE_MS * 3);
    rerender(<Behind shown />);
    await opened();
    await userEvent.click(answer("Accept"));
    expect(accepted).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toHaveTextContent(RAISED_ALERT_TOO_SOON);
  });

  it("leaves a key held in a field elsewhere alone", () => {
    render(
      <>
        <input aria-label="Message" />
        <ProjectShownProvider value={false}>
          <Asked />
        </ProjectShownProvider>
      </>,
    );
    const message = screen.getByRole("textbox", { name: "Message" });
    message.focus();
    expect(fireEvent.keyDown(message, { key: " ", code: "Space", repeat: true })).toBe(true);
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

describe("an alert the app raises, as a key that opens a dialog is let go", () => {
  it("waits for the dialog that key opened, and never stands under it", async () => {
    function Opener({ asked }: { asked: boolean }) {
      const [renaming, setRenaming] = useState(false);
      return (
        <>
          <Button onPress={() => setRenaming(true)}>Rename</Button>
          {renaming && (
            <Dialog onClose={() => setRenaming(false)} title="Rename workspace">
              <input aria-label="Name" />
            </Dialog>
          )}
          {asked && <Asked />}
        </>
      );
    }
    const { rerender } = render(<Opener asked={false} />);
    screen.getByRole("button", { name: "Rename" }).focus();
    await userEvent.keyboard("[Space>]");
    rerender(<Opener asked />);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await userEvent.keyboard("[/Space]");
    await screen.findByRole("dialog", { name: "Rename workspace" });
    await act(() => new Promise((done) => setTimeout(done, 20)));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await userEvent.keyboard("{Escape}");
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

  it("says no count when it is the only one, and one once a second is raised behind it", async () => {
    const { rerender } = render(<Asked title="First policy" />);
    const alert = await opened("First policy");
    expect(alert).not.toHaveTextContent("of 1");
    rerender(
      <>
        <Asked title="First policy" />
        <Asked title="Second policy" />
      </>,
    );
    await waitFor(() => expect(alert).toHaveTextContent("1 of 2"));
  });

  it("forgets one withdrawn while it waits", async () => {
    function Two({ second }: { second: boolean }) {
      const [first, setFirst] = useState(true);
      return (
        <>
          <Asked raised={first} title="First policy" keep={() => setFirst(false)} />
          <Asked raised={second} title="Second policy" />
        </>
      );
    }
    const { rerender } = render(<Two second />);
    const first = await opened("First policy");
    expect(first).toHaveTextContent("1 of 2");
    rerender(<Two second={false} />);
    await waitFor(() => expect(first).not.toHaveTextContent("of 2"));
    passes(SETTLE_MS);
    await userEvent.click(answer("Keep my current rules"));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await act(() => new Promise((done) => setTimeout(done, 20)));
    expect(screen.queryByRole("alertdialog")).toBeNull();
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
    await userEvent.tab({ shift: true });
    expect(body).toHaveFocus();
    tall.mockImplementation(() => 0);
    rerender(<Asked shows="digest-2">A short list</Asked>);
    await waitFor(() => expect(body).not.toHaveAttribute("tabindex"));
  });

  it("opens on the person typing once their key is up, starts on its answer that grants nothing, which Esc chooses, and hands the keyboard back", async () => {
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
    fireEvent.keyDown(message(), { key: "a", code: "KeyA" });
    fireEvent.change(message(), { target: { value: "a" } });
    // Not under the key still down.
    expect(screen.queryByRole("alertdialog")).toBeNull();
    fireEvent.keyUp(message(), { key: "a", code: "KeyA" });
    const alert = await opened();
    expect(answersOf(alert)).toEqual(["Keep my current rules", "Accept"]);
    passes(SETTLE_MS);
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await waitFor(() => expect(message()).toHaveFocus());
  });

  it("takes Return in a field for no answer as it settles, and never presses a grant with it after", async () => {
    const accepted = vi.fn();
    render(
      <Asked accept={accepted}>
        <input aria-label="Note" />
      </Asked>,
    );
    await opened();
    screen.getByRole("textbox", { name: "Note" }).focus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("status")).toHaveTextContent(RAISED_ALERT_TOO_SOON);
    passes(SETTLE_MS);
    await userEvent.keyboard("{Enter}");
    expect(accepted).not.toHaveBeenCalled();
  });

  it("leaves the keyboard on the window when what opened it is gone", async () => {
    const closed = vi.fn();
    function Gone({ opener, raised }: { opener: boolean; raised: boolean }) {
      return (
        <>
          {opener && <input aria-label="Message" />}
          <Asked raised={raised} afterClose={closed} />
        </>
      );
    }
    const { rerender } = render(<Gone opener raised={false} />);
    screen.getByRole("textbox", { name: "Message" }).focus();
    rerender(<Gone opener raised />);
    await opened();
    rerender(<Gone opener={false} raised />);
    rerender(<Gone opener={false} raised={false} />);
    await waitFor(() => expect(closed).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(document.activeElement?.isConnected).toBe(true);
  });
});
