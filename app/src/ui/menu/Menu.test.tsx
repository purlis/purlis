// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The menu family's wrapper** (#2909): what a person opens, walks, chooses and closes, and the
 * attributes a theme's parts are keyed on (ADR 0103). The look itself is held by the stories,
 * in a real browser (`Menu.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useRef, useState } from "react";
import { Button } from "../button/Button";
import { ProjectShownProvider } from "../../projectShown";
import {
  AnchoredMenu,
  Menu,
  MenuItem,
  MenuSection,
  MenuSeparator,
  MenuShortcuts,
  MenuTrigger,
  MenuWords,
  Submenu,
} from "./Menu";
import { ContextMenu } from "./ContextMenu";

afterEach(cleanup);

function Dropdown({
  onAction = () => {},
  onClosed,
}: {
  onAction?: (key: string) => void;
  onClosed?: () => void;
}) {
  return (
    <>
      <p>Outside</p>
      <MenuTrigger onClosed={onClosed}>
        <Button>Things</Button>
        <Menu aria-label="Things" onAction={(key) => onAction(String(key))}>
          <MenuItem id="rename" textValue="Rename…" keys="F2">
            <MenuWords said="Rename…" />
          </MenuItem>
          <MenuItem id="pin" textValue="Pin">
            <MenuWords said="Pin" />
          </MenuItem>
          <MenuSeparator />
          <MenuItem id="end" textValue="End" variant="danger" className="ends-it">
            <MenuWords said="End" under="Ends the program it runs. There is no undo." />
          </MenuItem>
        </Menu>
      </MenuTrigger>
    </>
  );
}

describe("a menu opened by its trigger", () => {
  it("opens on a press, says what it is about, and chooses a row", async () => {
    const chose = vi.fn();
    render(<Dropdown onAction={chose} />);
    await userEvent.click(screen.getByRole("button", { name: "Things" }));
    const menu = screen.getByRole("menu", { name: "Things" });
    expect(menu).toHaveAttribute("data-scope", "popover");
    expect(menu).toHaveAttribute("data-part", "list");
    await userEvent.click(screen.getByRole("menuitem", { name: /Pin/ }));
    expect(chose).toHaveBeenCalledWith("pin");
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("is walked by the arrows, which stop at the ends, and chosen by Enter", async () => {
    const chose = vi.fn();
    render(<Dropdown onAction={chose} />);
    screen.getByRole("button", { name: "Things" }).focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement).toHaveTextContent("Rename…"));
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown}");
    expect(document.activeElement).toHaveTextContent("End");
    await userEvent.keyboard("{ArrowUp}{Enter}");
    expect(chose).toHaveBeenCalledWith("pin");
  });

  it("finds a row by the first letter of its words", async () => {
    const chose = vi.fn();
    render(<Dropdown onAction={chose} />);
    screen.getByRole("button", { name: "Things" }).focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement).toHaveTextContent("Rename…"));
    await userEvent.keyboard("e");
    expect(document.activeElement).toHaveTextContent("End");
  });

  it("is named by its own name, not its trigger's", async () => {
    render(
      <MenuTrigger>
        <Button>More</Button>
        <Menu aria-label="Default profile">
          <MenuItem textValue="Reset">Reset</MenuItem>
        </Menu>
      </MenuTrigger>,
    );
    await userEvent.click(screen.getByRole("button", { name: "More" }));
    expect(screen.getByRole("menu", { name: "Default profile" })).toBeInTheDocument();
  });

  it("closes on Escape and hands the keyboard back to its trigger", async () => {
    render(<Dropdown />);
    const trigger = screen.getByRole("button", { name: "Things" });
    trigger.focus();
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("menu");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });

  it("closes on a click outside", async () => {
    render(<Dropdown />);
    await userEvent.click(screen.getByRole("button", { name: "Things" }));
    await screen.findByRole("menu");
    await userEvent.click(document.body);
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  });

  it("closes on a press outside on a control, which keeps the keyboard", async () => {
    render(
      <>
        <Button>Elsewhere</Button>
        <Dropdown />
      </>,
    );
    screen.getByRole("button", { name: "Things" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.getByRole("menuitem", { name: /Rename/ })).toHaveFocus());
    const elsewhere = screen.getByRole("button", { name: "Elsewhere" });
    await userEvent.click(elsewhere);
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    // After the family's hand-back and React Aria's, a frame later: the press's control has it.
    await new Promise((frame) => requestAnimationFrame(frame));
    expect(elsewhere).toHaveFocus();
  });

  it("tells its owner it closed, and the keyboard stays where its owner put it", async () => {
    const closed = vi.fn(() =>
      screen.getByRole("button", { name: "Elsewhere", hidden: true }).focus(),
    );
    render(
      <>
        <Button>Elsewhere</Button>
        <Dropdown onClosed={closed} />
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Things" }));
    await userEvent.keyboard("{Escape}");
    expect(closed).toHaveBeenCalledTimes(1);
    // React Aria's own hand-back, a frame later, finds the keyboard already given, and so does
    // the family's, a microtask after the menu goes: the owner's place stands.
    await new Promise((frame) => requestAnimationFrame(frame));
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
    await new Promise((frame) => requestAnimationFrame(frame));
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Elsewhere" }));
  });

  it("draws each row as the family's root, in its variant, with its parts", async () => {
    render(<Dropdown />);
    await userEvent.click(screen.getByRole("button", { name: "Things" }));
    const end = screen.getByRole("menuitem", { name: /End/ });
    expect(end).toHaveAttribute("data-scope", "menu");
    expect(end).toHaveAttribute("data-part", "root");
    expect(end).toHaveAttribute("data-variant", "danger");
    expect(end).toHaveClass("ends-it");
    expect(end.querySelector('[data-part="description"]')).toHaveTextContent(
      "Ends the program it runs. There is no undo.",
    );
    const rename = screen.getByRole("menuitem", { name: /Rename/ });
    expect(rename).toHaveAttribute("data-variant", "default");
    expect(rename.querySelector('[data-part="shortcut"]')).toHaveTextContent("F2");
    const surface = screen.getByRole("menu").closest('[data-part="root"][data-scope="popover"]');
    expect(surface).toHaveAttribute("data-variant", "menu");
    expect(screen.getByRole("separator")).toHaveAttribute("data-part", "rule");
  });
});

describe("a menu held open by its owner", () => {
  function Held() {
    const [open, setOpen] = useState<boolean | "first">(false);
    const trigger = useRef<HTMLButtonElement>(null);
    return (
      <>
        <p>Outside</p>
        <Button
          ref={trigger}
          aria-haspopup="menu"
          aria-expanded={open !== false}
          onPress={(event) =>
            setOpen((up) => (up ? false : event.pointerType === "keyboard" ? "first" : true))
          }
        >
          More
        </Button>
        <AnchoredMenu
          triggerRef={trigger}
          isOpen={open !== false}
          autoFocus={open === false ? true : open}
          onOpenChange={setOpen}
        >
          <Menu aria-label="More" align="end">
            <MenuItem id="one" textValue="One">
              One
            </MenuItem>
          </Menu>
        </AnchoredMenu>
      </>
    );
  }

  it("opens and closes on its owner's press, and a second press leaves it closed", async () => {
    render(<Held />);
    const more = screen.getByRole("button", { name: "More" });
    await userEvent.click(more);
    expect(screen.getByRole("menu", { name: "More" })).toBeInTheDocument();
    await userEvent.click(more);
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await new Promise((frame) => setTimeout(frame, 20));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("closes on a press outside, which reaches what it was on, and on Escape", async () => {
    render(<Held />);
    await userEvent.click(screen.getByRole("button", { name: "More" }));
    // Not modal: the rest of the window is still there to a screen reader.
    expect(screen.getByText("Outside")).toBeVisible();
    await userEvent.click(screen.getByText("Outside"));
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    screen.getByRole("button", { name: "More" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(document.activeElement).toHaveTextContent("One"));
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  });

  it("stands under its trigger, lined up with the edge it is asked for", async () => {
    render(<Held />);
    await userEvent.click(screen.getByRole("button", { name: "More" }));
    const surface = screen.getByRole("menu").closest('[data-part="root"]');
    expect(surface).toHaveAttribute("data-placement", "bottom");
  });
});

describe("a menu that takes no keyboard", () => {
  // A preview the pointer's rest opened (`takesNoKeyboard`): a press like a screen reader's on
  // one of its rows leaves the keyboard where it was (#3074 takes the keyboard to a pressed row
  // only in a menu that takes it).
  function Preview() {
    const trigger = useRef<HTMLButtonElement>(null);
    return (
      <>
        <button ref={trigger} type="button">
          Tasks
        </button>
        <AnchoredMenu triggerRef={trigger} isOpen onOpenChange={() => {}} autoFocus={false}>
          <Menu aria-label="Tasks" takesNoKeyboard>
            <MenuItem id="one" textValue="One" shouldCloseOnSelect={false}>
              One
            </MenuItem>
          </Menu>
        </AnchoredMenu>
      </>
    );
  }

  it("leaves the keyboard where it was when a row is pressed", async () => {
    render(<Preview />);
    const tasks = screen.getByRole("button", { name: "Tasks" });
    tasks.focus();
    fireEvent.click(within(screen.getByRole("menu", { name: "Tasks" })).getByRole("menuitem"));
    await act(() => new Promise((done) => setTimeout(done, 50)));
    expect(document.activeElement).toBe(tasks);
  });
});

describe("a row's shortcut", () => {
  it("is the key registry's for its act, where the registry has one", async () => {
    render(
      <MenuShortcuts.Provider value={(act) => (act === "tab.rename" ? "⌘R" : undefined)}>
        <MenuTrigger>
          <Button>Things</Button>
          <Menu aria-label="Things">
            <MenuItem id="rename" textValue="Rename…" shortcut="tab.rename" keys="F2">
              Rename…
            </MenuItem>
            <MenuItem id="pin" textValue="Pin" shortcut="tab.pin" keys="P">
              Pin
            </MenuItem>
            <MenuItem id="close" textValue="Close" shortcut="tab.close">
              Close
            </MenuItem>
          </Menu>
        </MenuTrigger>
      </MenuShortcuts.Provider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Things" }));
    const keyOf = (name: RegExp) =>
      screen.getByRole("menuitem", { name }).querySelector('[data-part="shortcut"]')?.textContent ??
      null;
    expect(keyOf(/Rename/)).toBe("⌘R");
    // Not in the registry: the key its caller knows.
    expect(keyOf(/Pin/)).toBe("P");
    expect(keyOf(/Close/)).toBeNull();
  });
});

describe("a menu's groups, toggles and submenus", () => {
  function Grouped({ onAction = () => {} }: { onAction?: (key: string) => void }) {
    const [pinned, setPinned] = useState<Set<string | number>>(new Set());
    return (
      <MenuTrigger>
        <Button>Tab</Button>
        <Menu aria-label="Tab steward 4" onAction={(key) => onAction(String(key))}>
          <MenuSection
            title="Pins"
            selectionMode="multiple"
            selectedKeys={pinned}
            onSelectionChange={(keys) => setPinned(new Set(keys as Set<string>))}
          >
            <MenuItem id="pinned" textValue="Pinned">
              Pinned
            </MenuItem>
          </MenuSection>
          <MenuSection>
            <Submenu
              row={<MenuItem textValue="Open">Open</MenuItem>}
              aria-label="Open"
              onAction={(key) => onAction(String(key))}
            >
              <MenuItem id="activity" textValue="Activity">
                Activity
              </MenuItem>
            </Submenu>
          </MenuSection>
        </Menu>
      </MenuTrigger>
    );
  }

  it("names a group by its header, and ticks a toggle that is on", async () => {
    render(<Grouped />);
    await userEvent.click(screen.getByRole("button", { name: "Tab" }));
    expect(screen.getByRole("group", { name: "Pins" })).toBeInTheDocument();
    const pinned = screen.getByRole("menuitemcheckbox", { name: "Pinned" });
    expect(pinned).toHaveAttribute("aria-checked", "false");
    // A toggle keeps its menu up, so the next one is a press away.
    await userEvent.click(pinned);
    expect(screen.getByRole("menuitemcheckbox", { name: "Pinned" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(
      pinned.querySelector('[data-part="indicator"][data-indicator="check"] svg'),
    ).not.toBeNull();
  });

  it("opens a submenu by Right and closes it by Left", async () => {
    const chose = vi.fn();
    render(<Grouped onAction={chose} />);
    screen.getByRole("button", { name: "Tab" }).focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement).toHaveTextContent("Pinned"));
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement).toHaveTextContent("Open");
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(screen.getAllByRole("menu")).toHaveLength(2));
    await waitFor(() => expect(document.activeElement).toHaveTextContent("Activity"));
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(screen.getAllByRole("menu")).toHaveLength(1));
  });

  // #3074: a press that is not the pointer's (a screen reader's, `element.click()`) hands the
  // keyboard on only after the page's transitions, a frame or more late; a submenu row pressed
  // before then stays open, and its row runs on the first click.
  it("keeps a submenu its row opened before the menu took the keyboard, and runs its row", async () => {
    const chose = vi.fn();
    render(<Grouped onAction={chose} />);
    fireEvent.click(screen.getByRole("button", { name: "Tab" }));
    // At once: before the frame the menu's hand-on waits for.
    const menu = screen.getByRole("menu", { name: "Tab steward 4" });
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Open" }));
    const sub = screen.getByRole("menu", { name: "Open" });
    // Past the frames the menu's own hand-on waits for.
    await act(() => new Promise((done) => setTimeout(done, 100)));
    expect(sub).toBeInTheDocument();
    // The keyboard is in the submenu, where React Aria hands it after a press like this.
    expect(sub).toContainElement(document.activeElement as HTMLElement);
    await userEvent.click(within(sub).getByRole("menuitem", { name: "Activity" }));
    expect(chose).toHaveBeenCalledWith("activity");
  });

  // **#3105**: React Aria hands a menu a screen reader opened to its first row, and two calls
  // race for it, each waiting for the page's transitions. The rows are built after the first
  // render, so the list is asked for first, takes the keyboard, and the row's own call then finds
  // the keyboard has moved and leaves it. The APG menu pattern asks for the first row.
  const pastTheHandOn = () => act(() => new Promise((done) => setTimeout(done, 100)));

  it("lands a screen reader's open of a root menu on its first row", async () => {
    render(<Grouped />);
    fireEvent.click(screen.getByRole("button", { name: "Tab" }));
    await pastTheHandOn();
    expect(document.activeElement).toBe(screen.getByRole("menuitemcheckbox", { name: "Pinned" }));
  });

  it("lands a screen reader's open of a submenu on its first row", async () => {
    render(<Grouped />);
    fireEvent.click(screen.getByRole("button", { name: "Tab" }));
    await pastTheHandOn();
    fireEvent.click(screen.getByRole("menuitem", { name: "Open" }));
    await pastTheHandOn();
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Activity" }));
  });

  it("leaves a pointer's open of a root menu and of a submenu where they were", async () => {
    render(<Grouped />);
    await userEvent.click(screen.getByRole("button", { name: "Tab" }));
    await pastTheHandOn();
    const menu = screen.getByRole("menu", { name: "Tab steward 4" });
    // On the list, and no row: a pointer has no need of a row chosen for it.
    expect(document.activeElement).toBe(menu);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Open" }));
    await pastTheHandOn();
    // The row it pressed keeps the keyboard, and the submenu is up beside it.
    expect(screen.getByRole("menu", { name: "Open" })).toBeInTheDocument();
    expect(document.activeElement).toBe(within(menu).getByRole("menuitem", { name: "Open" }));
  });

  it("leaves a keyboard's open on the row it asked for", async () => {
    render(<Grouped />);
    screen.getByRole("button", { name: "Tab" }).focus();
    await userEvent.keyboard("{ArrowUp}");
    await pastTheHandOn();
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Open" }));
  });
});

describe("a menu held by a project", () => {
  it("is drawn only while its project is in front", async () => {
    const { rerender } = render(
      <ProjectShownProvider value>
        <Dropdown />
      </ProjectShownProvider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Things" }));
    expect(screen.getByRole("menu", { hidden: true })).toBeInTheDocument();
    rerender(
      <ProjectShownProvider value={false}>
        <Dropdown />
      </ProjectShownProvider>,
    );
    expect(screen.queryByRole("menu", { hidden: true })).toBeNull();
  });
});

describe("a context menu", () => {
  function OnARow({
    onAsked,
    onOpenChange,
  }: {
    onAsked?: (event: React.MouseEvent<HTMLElement>) => void;
    onOpenChange?: (open: boolean) => void;
  }) {
    const [kept, setKept] = useState(0);
    return (
      <ContextMenu
        onAsked={onAsked}
        onOpenChange={onOpenChange}
        menu={() => (
          <Menu aria-label="Row steward 4" onAction={() => setKept((n) => n + 1)}>
            <MenuItem id="rename" textValue="Rename…">
              Rename…
            </MenuItem>
            <MenuItem id="end" textValue="End" variant="danger">
              End
            </MenuItem>
          </Menu>
        )}
      >
        <button type="button" data-kept={kept} onClick={() => setKept((n) => n + 10)}>
          steward 4
        </button>
      </ContextMenu>
    );
  }

  it("opens at the pointer on a right-click, and the element keeps its own press", async () => {
    render(<OnARow />);
    const row = screen.getByRole("button", { name: "steward 4" });
    fireEvent.contextMenu(row, { clientX: 40, clientY: 60 });
    expect(await screen.findByRole("menu", { name: "Row steward 4" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await userEvent.click(row);
    expect(row).toHaveAttribute("data-kept", "10");
  });

  it("opens from Shift+F10 and the menu key on its first row", async () => {
    render(<OnARow />);
    const row = screen.getByRole("button", { name: "steward 4" });
    row.focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    await waitFor(() => expect(document.activeElement).toHaveTextContent("Rename…"));
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(document.activeElement).toBe(row));
    fireEvent.keyDown(row, { key: "ContextMenu" });
    expect(await screen.findByRole("menu")).toBeInTheDocument();
  });

  it("stays up for a scroll that left its row where it was, and closes for one that moved it", async () => {
    // #3797: the keyboard's arrival scrolls a list to the row, and that scroll's event comes a
    // frame later, after a Shift+F10 pressed at once has opened the menu.
    render(
      <div data-testid="scroller" style={{ overflow: "auto" }}>
        <OnARow />
      </div>,
    );
    const row = screen.getByRole("button", { name: "steward 4" });
    row.focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    await screen.findByRole("menu", { name: "Row steward 4" });

    fireEvent.scroll(screen.getByTestId("scroller"));
    await new Promise((soon) => setTimeout(soon, 20));
    expect(screen.getByRole("menu", { name: "Row steward 4" })).toBeInTheDocument();

    vi.spyOn(row, "getBoundingClientRect").mockReturnValue(new DOMRect(0, -58, 100, 20));
    fireEvent.scroll(screen.getByTestId("scroller"));
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  });

  it("runs no row on the release of the press that opened it (a Mac's Ctrl+click)", async () => {
    render(<OnARow />);
    const row = screen.getByRole("button", { name: "steward 4" });
    fireEvent.pointerDown(row, { button: 0, ctrlKey: true, pointerType: "mouse" });
    fireEvent.contextMenu(row, { button: 0, ctrlKey: true });
    const menu = await screen.findByRole("menu", { name: "Row steward 4" });
    const first = within(menu).getByRole("menuitem", { name: "Rename…" });
    // The left button comes up over the first row, which the menu was placed under.
    fireEvent.pointerUp(first, { button: 0, pointerType: "mouse" });
    fireEvent.mouseUp(first, { button: 0 });
    await new Promise((soon) => setTimeout(soon, 20));
    expect(screen.getByRole("menu", { name: "Row steward 4" })).toBeInTheDocument();
    expect(row).toHaveAttribute("data-kept", "0");
    // A press after it is a press.
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Rename…" }));
    expect(row).toHaveAttribute("data-kept", "1");
  });

  it("opens nothing for a right-click a program kept", () => {
    render(<OnARow onAsked={(event) => event.preventDefault()} />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "steward 4" }));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("stands back for a menu inside it", async () => {
    const outer = vi.fn();
    render(
      <ContextMenu
        onOpenChange={outer}
        menu={() => (
          <Menu aria-label="Panes">
            <MenuItem textValue="New tab">New tab</MenuItem>
          </Menu>
        )}
      >
        <div>
          <OnARow />
        </div>
      </ContextMenu>,
    );
    fireEvent.contextMenu(screen.getByRole("button", { name: "steward 4" }));
    expect(await screen.findByRole("menu", { name: "Row steward 4" })).toBeInTheDocument();
    expect(screen.queryByRole("menu", { name: "Panes" })).toBeNull();
    expect(outer).not.toHaveBeenCalled();
  });

  it("closes on a right-click elsewhere", async () => {
    const opened = vi.fn();
    render(<OnARow onOpenChange={opened} />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "steward 4" }));
    await screen.findByRole("menu");
    act(() => {
      document.body.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true }));
    });
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(opened).toHaveBeenLastCalledWith(false);
  });
});
