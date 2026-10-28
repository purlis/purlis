// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The dialog family's wrapper** (#2910, spec #2897): what a person reads, presses and types in a
 * dialog, and the attributes a theme's parts are keyed on (ADR 0103). The look itself (the width,
 * the fixed top, the footer always in view) is held by the stories, in a real browser
 * (`Dialog.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useRef, useState } from "react";
import { ProjectShownProvider } from "../../projectShown";
import { Button } from "../button/Button";
import { AlertDialog, Dialog, DialogSurface } from "./Dialog";
import { Reserved } from "./Reserved";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "../menu/Menu";
import { Select } from "../select/Select";
import { selectNamed } from "../select/select.testkit";

afterEach(cleanup);

/** A dialog a button opens, as a call site writes one. */
function Opened({ main, onClosed }: { main?: () => void; onClosed?: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onPress={() => setOpen(true)}>Rename</Button>
      <Dialog
        isOpen={open}
        onClose={() => setOpen(false)}
        title="Rename workspace"
        main={{
          children: "Rename",
          onPress: () => {
            main?.();
            setOpen(false);
          },
        }}
        afterClose={onClosed}
      >
        <label>
          Name
          <input />
        </label>
      </Dialog>
    </>
  );
}

describe("a dialog", () => {
  it("is a dialog named by its title, with a header, a body and a footer", () => {
    render(
      <Dialog
        onClose={() => {}}
        title="New workspace"
        subtitle="in purlis"
        main={{ children: "Create" }}
      >
        <p>What it is for.</p>
      </Dialog>,
    );
    const dialog = screen.getByRole("dialog", { name: "New workspace" });
    expect(dialog).toHaveAttribute("data-scope", "dialog");
    expect(dialog).toHaveAttribute("data-part", "root");
    expect(dialog).toHaveAttribute("data-variant", "dialog");
    expect(dialog).toHaveAttribute("data-size", "standard");
    for (const part of ["header", "title", "subtitle", "body", "footer", "actions"])
      expect(
        dialog.querySelector(`[data-scope="dialog"][data-part="${part}"]`),
        part,
      ).not.toBeNull();
    // The footer is not inside the body that scrolls: it is always in view.
    const body = dialog.querySelector('[data-part="body"]');
    expect(body?.contains(dialog.querySelector('[data-part="footer"]'))).toBe(false);
    expect(body).toHaveTextContent("What it is for.");
    // The subtitle describes it.
    expect(dialog).toHaveAccessibleDescription("in purlis");
  });

  it("takes its width from its size and its height from what it is given, never its content", () => {
    render(
      <Dialog onClose={() => {}} title="Start a chat" size="wide" height={620}>
        <p>Two panes.</p>
      </Dialog>,
    );
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("data-size", "wide");
    expect(dialog).toHaveAttribute("data-fixed-height");
    expect(dialog.style.getPropertyValue("--dialog-height")).toBe("620px");
  });

  it("puts Cancel first and the main action last, with at most one other between", () => {
    render(
      <Dialog
        onClose={() => {}}
        title="Save changes?"
        main={{ children: "Save" }}
        alternative={{ children: "Don't save" }}
      >
        <p>Two files changed.</p>
      </Dialog>,
    );
    const actions = screen
      .getByRole("dialog")
      .querySelector('[data-part="actions"]') as HTMLElement;
    expect([...actions.querySelectorAll("button")].map((one) => one.textContent)).toEqual([
      "Cancel",
      "Don't save",
      "Save",
    ]);
    expect(screen.getByRole("button", { name: "Save" })).toHaveAttribute("data-variant", "primary");
    expect(screen.getByRole("button", { name: "Save" })).toHaveAttribute(
      "data-dialog-action",
      "main",
    );
  });

  it("starts on its first field, and Enter there is its main action", async () => {
    const renamed = vi.fn();
    render(<Opened main={renamed} />);
    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    const name = screen.getByRole("textbox", { name: "Name" });
    await waitFor(() => expect(name).toHaveFocus());
    await userEvent.type(name, "steward{Enter}");
    expect(renamed).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("closes on Escape, and hands the keyboard back to what opened it", async () => {
    const closed = vi.fn();
    render(<Opened onClosed={closed} />);
    const opener = screen.getByRole("button", { name: "Rename" });
    await userEvent.click(opener);
    await waitFor(() => expect(screen.getByRole("textbox")).toHaveFocus());
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    await waitFor(() => expect(opener).toHaveFocus());
    await waitFor(() => expect(closed).toHaveBeenCalledTimes(1));
  });

  it("closes from Cancel", async () => {
    const closed = vi.fn();
    render(
      <Dialog onClose={closed} title="About">
        <p>purlis</p>
      </Dialog>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(closed).toHaveBeenCalledTimes(1);
  });

  it("never finds an act that cannot be taken back with Return", async () => {
    const deleted = vi.fn();
    render(
      <Dialog
        onClose={() => {}}
        title="Delete vault?"
        main={{ children: "Delete", variant: "danger", onPress: deleted }}
      >
        <label>
          Type its name
          <input />
        </label>
      </Dialog>,
    );
    const field = screen.getByRole("textbox");
    await waitFor(() => expect(field).toHaveFocus());
    await userEvent.type(field, "keys{Enter}");
    expect(deleted).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(deleted).toHaveBeenCalledTimes(1);
  });

  it("never finds an answer that grants with Return, from a field or a checkbox", async () => {
    const granted = vi.fn();
    render(
      <Dialog
        onClose={() => {}}
        title="Trust this extension?"
        main={{ children: "Trust it", variant: "grants", onPress: granted }}
      >
        <label>
          A note
          <input />
        </label>
        <label>
          <input type="checkbox" />
          Remember
        </label>
      </Dialog>,
    );
    const field = screen.getByRole("textbox");
    await waitFor(() => expect(field).toHaveFocus());
    await userEvent.type(field, "keys{Enter}");
    const remember = screen.getByRole("checkbox");
    act(() => remember.focus());
    await userEvent.keyboard("{Enter}");
    expect(granted).not.toHaveBeenCalled();
    const grant = screen.getByRole("button", { name: /Trust it/ });
    expect(grant).toHaveAttribute("type", "button");
    await userEvent.click(grant);
    expect(granted).toHaveBeenCalledTimes(1);
  });

  it("never starts on an answer that grants, even asked to (#2980)", async () => {
    render(
      <Dialog
        onClose={() => {}}
        title="Open project?"
        initialFocus="main"
        main={{ children: "Open project", variant: "grants" }}
      >
        <p>It runs its own hooks.</p>
      </Dialog>,
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());
    // Reached and pressed on purpose, as any answer is.
    await userEvent.tab();
    expect(screen.getByRole("button", { name: /Open project/ })).toHaveFocus();
  });

  it("closes one layer with Escape: a menu open in it first, then the dialog", async () => {
    const closed = vi.fn();
    render(
      <Dialog onClose={closed} title="Workspace">
        <MenuTrigger>
          <Button>More</Button>
          <Menu aria-label="More">
            <MenuItem id="rename" textValue="Rename">
              <MenuWords said="Rename" />
            </MenuItem>
          </Menu>
        </MenuTrigger>
      </Dialog>,
    );
    const more = screen.getByRole("button", { name: "More" });
    await waitFor(() => expect(more).toHaveFocus());
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("menu");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(closed).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Workspace" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(closed).toHaveBeenCalledTimes(1);
  });

  // #3603: React Aria's dialog puts the keyboard on itself 500 ms after it opens when the keyboard
  // is on nothing then (a screen reader's workaround), and a menu's own hand-back waits a frame.
  // A frame drawn late, as in a busy run, let that take the keyboard the menu had just given up.
  it("gives the keyboard back to a menu's trigger as the menu goes, however late the next frame", async () => {
    // The timers a half-second wait asks for are held, to be run when the test says.
    const held: (() => void)[] = [];
    const later = globalThis.setTimeout;
    vi.spyOn(globalThis, "setTimeout").mockImplementation(((
      run: () => void,
      ms?: number,
      ...rest: unknown[]
    ) => {
      if (ms === 500) {
        held.push(run);
        return 0;
      }
      return later(run, ms, ...rest);
    }) as typeof setTimeout);
    const frames = new Map<number, FrameRequestCallback>();
    let ids = 0;
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((frame) => {
      frames.set(++ids, frame);
      return ids;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation((id) => void frames.delete(id));
    const drawFrame = () =>
      act(() => {
        const due = [...frames.values()];
        frames.clear();
        for (const frame of due) frame(performance.now());
      });
    try {
      render(
        <Dialog onClose={() => {}} title="Workspace">
          <MenuTrigger>
            <Button>More</Button>
            <Menu aria-label="More">
              <MenuItem id="rename" textValue="Rename">
                <MenuWords said="Rename" />
              </MenuItem>
            </Menu>
          </MenuTrigger>
        </Dialog>,
      );
      const more = screen.getByRole("button", { name: "More" });
      await drawFrame();
      expect(more).toHaveFocus();
      await userEvent.keyboard("{Enter}");
      await drawFrame();
      expect(screen.getByRole("menuitem", { name: "Rename" })).toHaveFocus();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("menu")).toBeNull();
      // Half a second after the dialog opened, and still no frame drawn.
      // Only React Aria's dialog asks for one: if it ever waits otherwise, this test says so.
      expect(held).toHaveLength(1);
      act(() => {
        for (const run of held.splice(0)) run();
      });
      await drawFrame();
      expect(more).toHaveFocus();
    } finally {
      vi.restoreAllMocks();
    }
  });

  it("closes one layer with Escape: a select's list open in it first, then the dialog", async () => {
    const closed = vi.fn();
    render(
      <Dialog onClose={closed} title="New workspace">
        <Select
          aria-label="Kind"
          value="code"
          options={[
            { value: "code", label: "Code" },
            { value: "notes", label: "Notes" },
          ]}
          onValueChange={() => {}}
        />
      </Dialog>,
    );
    const kind = selectNamed("Kind");
    await waitFor(() => expect(kind).toHaveFocus());
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("listbox");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    expect(closed).not.toHaveBeenCalled();
    await userEvent.keyboard("{Escape}");
    expect(closed).toHaveBeenCalledTimes(1);
  });

  it("starts where it is told", async () => {
    function Told() {
      const there = useRef<HTMLButtonElement>(null);
      return (
        <Dialog
          onClose={() => {}}
          title="Close chat?"
          initialFocus={there}
          main={{ children: "Close", ref: there }}
        >
          <input aria-label="Not here" />
        </Dialog>
      );
    }
    render(<Told />);
    await waitFor(() => expect(screen.getByRole("button", { name: "Close" })).toHaveFocus());
  });

  it("is drawn only while its project is in front", () => {
    render(
      <ProjectShownProvider value={false}>
        <Dialog onClose={() => {}} title="Delete persona steward?">
          <p>drawn by a project behind the one in front</p>
        </Dialog>
      </ProjectShownProvider>,
    );
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.body).not.toHaveTextContent("drawn by a project behind");
  });

  it("says so when it is opened over another dialog: no dialog opens another", () => {
    const said = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <>
        <Dialog onClose={() => {}} title="Extensions">
          <p>One</p>
        </Dialog>
        <Dialog onClose={() => {}} title="Approve extension">
          <p>Two</p>
        </Dialog>
      </>,
    );
    expect(said).toHaveBeenCalledWith(expect.stringContaining("no dialog opens another"));
    said.mockRestore();
  });
});

describe("an alert dialog", () => {
  it("is an alert dialog that starts on the safe answer", async () => {
    render(
      <AlertDialog
        onClose={() => {}}
        title="Delete persona steward?"
        description="Its memory goes with it."
        main={{ children: "Delete", variant: "danger" }}
      />,
    );
    const alert = screen.getByRole("alertdialog", { name: "Delete persona steward?" });
    expect(alert).toHaveAttribute("data-variant", "alert");
    expect(alert).toHaveAccessibleDescription("Its memory goes with it.");
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());
  });

  it("starts on itself, never on its main action, while its Cancel cannot be pressed", async () => {
    render(
      <AlertDialog
        onClose={() => {}}
        title="Delete persona steward?"
        cancel={{ isDisabled: true }}
        main={{ children: "Delete", variant: "danger" }}
      />,
    );
    const alert = screen.getByRole("alertdialog", { name: "Delete persona steward?" });
    await waitFor(() => expect(alert).toHaveFocus());
    expect(screen.getByRole("button", { name: "Delete" })).not.toHaveFocus();
  });

  it("takes Enter on its safe answer as that answer", async () => {
    const closed = vi.fn();
    const deleted = vi.fn();
    render(
      <AlertDialog
        onClose={closed}
        title="Delete persona steward?"
        main={{ children: "Delete", variant: "danger", onPress: deleted }}
      />,
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());
    await userEvent.keyboard("{Enter}");
    expect(closed).toHaveBeenCalledTimes(1);
    expect(deleted).not.toHaveBeenCalled();
  });
});

describe("a dialog's surface", () => {
  it("is a modal named by its label, with no header or footer of the family's", async () => {
    const closed = vi.fn();
    render(
      <DialogSurface onClose={closed} aria-label="Command palette" className="palette">
        <input aria-label="Run an action" />
      </DialogSurface>,
    );
    const surface = screen.getByRole("dialog", { name: "Command palette" });
    expect(surface).toHaveClass("palette");
    expect(surface.querySelector('[data-part="footer"]')).toBeNull();
    await waitFor(() => expect(screen.getByRole("textbox")).toHaveFocus());
    await userEvent.keyboard("{Escape}");
    expect(closed).toHaveBeenCalledTimes(1);
  });
});

describe("text that changes with a choice", () => {
  it("reserves the room of every text it can show, and says only the one it shows", () => {
    render(
      <Button>
        <Reserved texts={["Create workspace", "Creating…"]}>Creating…</Reserved>
      </Button>,
    );
    const button = screen.getByRole("button");
    expect(button).toHaveAccessibleName("Creating…");
    // Both are laid out, so the button is as wide as the wider; only one is seen, heard and
    // found by its text.
    expect(button).toHaveTextContent(/^Creating…$/);
    const kept = button.querySelector('[data-scope="reserved"][data-part="kept"]');
    expect(kept).toHaveAttribute("data-text", "Create workspace");
    expect(kept).toHaveAttribute("aria-hidden", "true");
  });

  it("reserves its room in a block too", () => {
    const { container } = render(
      <Reserved
        as="div"
        texts={["Billed to Claude.", "Billed to whatever opencode is signed in with."]}
      >
        Billed to Claude.
      </Reserved>,
    );
    expect(container.querySelector("div[data-scope='reserved']")).not.toBeNull();
  });
});

describe("a dialog opened twice", () => {
  it("opens again after it closed, with nothing said about nesting", async () => {
    const said = vi.spyOn(console, "error").mockImplementation(() => {});
    render(<Opened />);
    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    await userEvent.keyboard("{Escape}");
    await act(async () => {});
    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(said).not.toHaveBeenCalledWith(expect.stringContaining("no dialog opens another"));
    said.mockRestore();
  });
});

describe("a dialog's close", () => {
  it("calls the afterClose it was last drawn with, not the one it opened with", async () => {
    const first = vi.fn();
    const last = vi.fn();
    const { rerender, unmount } = render(
      <Dialog onClose={() => {}} afterClose={first} title="Rename workspace" />,
    );
    rerender(<Dialog onClose={() => {}} afterClose={last} title="Rename workspace" />);
    unmount();
    await act(async () => {});
    expect(first).not.toHaveBeenCalled();
    expect(last).toHaveBeenCalledOnce();
  });
});

describe("a dialog closed as its window goes", () => {
  // The window's `document` can go before a close's last step runs: a test's environment taken
  // down, a webview torn down. What the close still does uses the document the dialog was drawn
  // in, never the global one.
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("lets React Aria's return go after it held it, with the global document gone", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const { unmount } = render(
      <Dialog onClose={() => {}} returnFocus={() => false} title="Start a chat">
        <p>A chat.</p>
      </Dialog>,
    );
    unmount();
    // The close's microtask: the keyboard is not handed back, and React Aria's return is held.
    await act(async () => {});
    vi.stubGlobal("document", undefined);
    expect(() => vi.advanceTimersByTime(100)).not.toThrow();
  });

  it("hands the keyboard back and calls afterClose, with the global document gone", async () => {
    const closed = vi.fn();
    const { unmount } = render(
      <Dialog onClose={() => {}} afterClose={closed} title="Rename workspace">
        <p>A name.</p>
      </Dialog>,
    );
    unmount();
    // Gone before the close's microtask runs.
    vi.stubGlobal("document", undefined);
    vi.stubGlobal("HTMLElement", undefined);
    await Promise.resolve();
    await Promise.resolve();
    vi.unstubAllGlobals();
    expect(closed).toHaveBeenCalledOnce();
  });
});
