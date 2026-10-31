// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tooltip family** (#2908): how a hint opens, stays and goes, as a person meets it, and
 * WCAG 1.4.13 (content on hover or focus): **dismissible** by Esc without moving the pointer or
 * the focus, **hoverable** so the pointer can move onto it, and **persistent** until the pointer
 * or the focus leaves or Esc. The look is held by the stories, in a real browser
 * (`Tooltip.stories.tsx`).
 */
import { cloneElement, useEffect, useRef, useState, type ReactElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { ProjectShownProvider } from "../../projectShown";
import { Button, IconButton, ToggleButton } from "../button/Button";
import { Dialog } from "../dialog/Dialog";
import { PopoverDialog, PopoverTrigger } from "../popover/Popover";
import { StatusCell, StatusItem } from "../status/StatusItem";
import {
  Tooltip,
  TOOLTIP_CLOSE_DELAY,
  TOOLTIP_DELAY,
  TOOLTIP_OFFSET,
  TOOLTIP_PLACEMENT,
} from "./Tooltip";
import { shownTip, tipOf } from "./tooltip.testkit";

/** Long enough for any tooltip that is closing to have closed, and the warm-up to have cooled. */
const SETTLED = TOOLTIP_DELAY + TOOLTIP_CLOSE_DELAY + 1000;

let user: ReturnType<typeof userEvent.setup>;

beforeEach(() => {
  // Time moves on its own as well, as Testing Library's waits need.
  vi.useFakeTimers({ shouldAdvanceTime: true });
  user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
});

afterEach(async () => {
  // Every tooltip closed and the warm-up over, so no test opens one at once for the last's sake.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(SETTLED);
  });
  cleanup();
  vi.useRealTimers();
});

/** The pointer has moved over the window before it comes onto a target, as a real one has. */
const pointerIn = () => user.hover(document.body);

const wait = (ms: number) =>
  act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });

function Path() {
  return (
    <>
      <Tooltip tip="/Users/me/work/purlis">
        <span>purlis</span>
      </Tooltip>
      <span>elsewhere</span>
    </>
  );
}

describe("a tooltip under the pointer", () => {
  it("opens after the one delay, beside its target at its end, 10 px from it (R166)", async () => {
    render(<Path />);
    await pointerIn();
    await user.hover(screen.getByText("purlis"));
    await wait(TOOLTIP_DELAY - 100);
    expect(shownTip()).toBeNull();
    await wait(200);
    expect(shownTip()).toHaveTextContent("/Users/me/work/purlis");
    expect(TOOLTIP_PLACEMENT).toBe("end");
    expect(TOOLTIP_OFFSET).toBe(10);
  });

  it("is hoverable: the pointer moves onto it and it stays (WCAG 1.4.13)", async () => {
    render(<Path />);
    await pointerIn();
    await user.hover(screen.getByText("purlis"));
    await wait(TOOLTIP_DELAY + 50);
    const tip = shownTip();
    expect(tip).not.toBeNull();
    await user.unhover(screen.getByText("purlis"));
    if (tip === null) throw new Error("no tooltip");
    await user.hover(tip);
    await wait(TOOLTIP_CLOSE_DELAY * 10);
    expect(shownTip()).toHaveTextContent("/Users/me/work/purlis");
  });

  it("is hoverable in the title bar too, and a press on it moves the window as the bar does", async () => {
    render(
      <div data-tauri-drag-region="deep">
        <Tooltip tip="Stop every agent in this project">
          <button type="button">Kill</button>
        </Tooltip>
        <span>gap</span>
      </div>,
    );
    await pointerIn();
    const target = screen.getByRole("button", { name: "Kill" });
    await user.hover(target);
    await wait(TOOLTIP_DELAY + 50);
    const tip = shownTip();
    if (tip === null) throw new Error("no tooltip");
    // It takes the pointer (`data-pointer`), and is a drag region of its own: it is drawn
    // outside the bar, so the bar's attribute does not reach it.
    expect(tip).toHaveAttribute("data-pointer");
    expect(tip).toHaveAttribute("data-tauri-drag-region", "deep");
    await user.unhover(target);
    await user.hover(tip);
    await wait(TOOLTIP_CLOSE_DELAY * 10);
    expect(shownTip()).toHaveTextContent("Stop every agent in this project");
  });

  it("takes no pointer when the keyboard opened it, in the title bar or anywhere", async () => {
    render(
      <div data-tauri-drag-region="deep">
        <Tooltip tip="Stop every agent in this project">
          <button type="button">Kill</button>
        </Tooltip>
      </div>,
    );
    await user.tab();
    const tip = shownTip();
    if (tip === null) throw new Error("no tooltip");
    expect(tip).not.toHaveAttribute("data-pointer");
  });

  it("is persistent: it stays while the pointer rests, and goes once it leaves both", async () => {
    render(<Path />);
    await pointerIn();
    const target = screen.getByText("purlis");
    await user.hover(target);
    await wait(TOOLTIP_DELAY + 50);
    await wait(60_000);
    expect(shownTip()).not.toBeNull();
    await user.unhover(target);
    await user.hover(screen.getByText("elsewhere"));
    await wait(TOOLTIP_CLOSE_DELAY + 50);
    expect(shownTip()).toBeNull();
  });

  it("is dismissible: Esc takes it down, and the pointer has not moved", async () => {
    render(<Path />);
    await user.hover(screen.getByText("purlis"));
    await wait(TOOLTIP_DELAY + 50);
    expect(shownTip()).not.toBeNull();
    await user.keyboard("{Escape}");
    expect(shownTip()).toBeNull();
  });

  it("opens the next one at once while the first is warm", async () => {
    render(
      <>
        <Tooltip tip="First hint">
          <span>one</span>
        </Tooltip>
        <Tooltip tip="Second hint">
          <span>two</span>
        </Tooltip>
      </>,
    );
    await pointerIn();
    await user.hover(screen.getByText("one"));
    await wait(TOOLTIP_DELAY + 50);
    await user.unhover(screen.getByText("one"));
    await user.hover(screen.getByText("two"));
    expect(shownTip()).toHaveTextContent("Second hint");
  });
});

describe("a tooltip from the keyboard", () => {
  it("makes words with a hint a Tab stop, opens at once on focus and describes them", async () => {
    render(<Path />);
    const target = screen.getByText("purlis");
    expect(target).toHaveAttribute("tabindex", "0");
    expect(screen.getByText("elsewhere")).not.toHaveAttribute("tabindex");
    await user.tab();
    expect(target).toHaveFocus();
    const tip = shownTip();
    expect(tip).toHaveTextContent("/Users/me/work/purlis");
    expect(target).toHaveAttribute("aria-describedby", tip?.id);
  });

  it("is dismissed by Esc, the focus staying where it was (WCAG 1.4.13)", async () => {
    render(<Path />);
    const target = screen.getByText("purlis");
    await user.tab();
    expect(shownTip()).not.toBeNull();
    await user.keyboard("{Escape}");
    expect(shownTip()).toBeNull();
    expect(target).toHaveFocus();
  });

  it("takes the first Esc in a dialog for itself; the second closes the dialog (WCAG 1.4.13)", async () => {
    function Asking() {
      const [open, setOpen] = useState(true);
      return open ? (
        <Dialog onClose={() => setOpen(false)} title="Delete persona steward?" cancel={false}>
          <Button tooltip="Its memory is archived, never removed">Delete</Button>
        </Dialog>
      ) : (
        <p>closed</p>
      );
    }
    render(<Asking />);
    const remove = await screen.findByRole("button", { name: "Delete" });
    // From the keyboard: the dialog put its focus on Delete as it opened.
    act(() => remove.blur());
    await user.tab();
    expect(remove).toHaveFocus();
    expect(shownTip()).not.toBeNull();
    await user.keyboard("{Escape}");
    // The tooltip went, and nothing else: the dialog is up, with the focus where it was.
    expect(shownTip()).toBeNull();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(remove).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(await screen.findByText("closed")).toBeInTheDocument();
  });

  it("keeps an Esc that dismisses it from the terminal that has the focus", async () => {
    const heard = vi.fn();
    function Terminal() {
      const ref = useRef<HTMLTextAreaElement>(null);
      useEffect(() => {
        // As xterm hears its keys: on its own element, outside React.
        const area = ref.current;
        const key = (event: KeyboardEvent) => heard(event.key);
        area?.addEventListener("keydown", key);
        return () => area?.removeEventListener("keydown", key);
      }, []);
      return <textarea ref={ref} aria-label="Terminal input" />;
    }
    render(
      <>
        <Path />
        <Terminal />
      </>,
    );
    await pointerIn();
    await user.hover(screen.getByText("purlis"));
    await wait(TOOLTIP_DELAY + 50);
    expect(shownTip()).not.toBeNull();
    act(() => screen.getByRole("textbox", { name: "Terminal input" }).focus());
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Terminal input" }), { key: "Escape" });
    expect(shownTip()).toBeNull();
    expect(heard).not.toHaveBeenCalled();
    // With no tooltip up, the terminal's Esc is its own.
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Terminal input" }), { key: "Escape" });
    expect(heard).toHaveBeenCalledWith("Escape");
  });

  it("keeps the first Esc from a window listener added before it opened (the palette's)", async () => {
    const heard = vi.fn();
    const palette = (event: KeyboardEvent) => heard(event.key);
    window.addEventListener("keydown", palette, true);
    try {
      render(<Path />);
      await pointerIn();
      await user.hover(screen.getByText("purlis"));
      await wait(TOOLTIP_DELAY + 50);
      expect(shownTip()).not.toBeNull();
      fireEvent.keyDown(document.body, { key: "Escape" });
      expect(shownTip()).toBeNull();
      expect(heard).not.toHaveBeenCalled();
      fireEvent.keyDown(document.body, { key: "Escape" });
      expect(heard).toHaveBeenCalledWith("Escape");
    } finally {
      window.removeEventListener("keydown", palette, true);
    }
  });

  it("stays while the focus does, and goes with it", async () => {
    render(
      <>
        <Path />
        <Button>Next</Button>
      </>,
    );
    await user.tab();
    await wait(60_000);
    expect(shownTip()).not.toBeNull();
    await user.tab();
    expect(screen.getByRole("button", { name: "Next" })).toHaveFocus();
    expect(shownTip()).toBeNull();
  });

  it("keeps an element's own place in the Tab order", () => {
    render(
      <Tooltip tip="Chats here start at the project root">
        <button role="tab" aria-selected="false" tabIndex={-1}>
          Project
        </button>
      </Tooltip>,
    );
    expect(screen.getByRole("tab")).toHaveAttribute("tabindex", "-1");
  });

  it("leaves a mark inside a control, and one a screen reader does not hear, to the pointer", async () => {
    render(
      <>
        <button type="button">
          main.rs
          <Tooltip tip="Changed in this branch">
            <span>M</span>
          </Tooltip>
        </button>
        <Tooltip tip="steward">
          <span aria-hidden="true">S</span>
        </Tooltip>
      </>,
    );
    expect(screen.getByText("M")).not.toHaveAttribute("tabindex");
    expect(screen.getByText("S")).not.toHaveAttribute("tabindex");
    await pointerIn();
    await user.hover(screen.getByText("M"));
    await wait(TOOLTIP_DELAY + 50);
    expect(shownTip()).toHaveTextContent("Changed in this branch");
  });

  it("hands what a parent gives it on to its element, as a Radix trigger's child", async () => {
    const opened = vi.fn();
    const ref = { current: null as HTMLElement | null };
    function Trigger({ children }: { children: ReactElement }) {
      return cloneElement(children, {
        onContextMenu: opened,
        "data-state": "closed",
        ref,
      } as object);
    }
    render(
      <Trigger>
        <Tooltip tip="Branch main">
          <span>main</span>
        </Tooltip>
      </Trigger>,
    );
    const words = screen.getByText("main");
    expect(words).toHaveAttribute("data-state", "closed");
    expect(ref.current).toBe(words);
    fireEvent.contextMenu(words);
    expect(opened).toHaveBeenCalledTimes(1);
    expect(tipOf(words)).toBe("Branch main");
  });

  it("draws no tooltip and takes no keyboard for an empty hint", () => {
    render(
      <Tooltip tip="">
        <span>plain</span>
      </Tooltip>,
    );
    const words = screen.getByText("plain");
    expect(words).not.toHaveAttribute("tabindex");
    expect(tipOf(words)).toBeNull();
  });
});

describe("a status item's tooltip", () => {
  it("is a pill's hint, shown by the keyboard, and a cell's, which makes it a Tab stop", () => {
    render(
      <>
        <StatusItem
          variant="pill"
          aria-label="Update"
          tooltip="Purlis 1.2 is ready"
          onPress={() => {}}
        />
        <StatusCell tooltip="From the forge's facts file" count="3">
          PRs
        </StatusCell>
      </>,
    );
    expect(tipOf(screen.getByRole("button", { name: "Update" }))).toBe("Purlis 1.2 is ready");
    const cell = screen.getByText("PRs").closest('[data-scope="status-item"][data-part="root"]');
    expect(cell).toHaveAttribute("tabindex", "0");
    expect(tipOf(cell)).toBe("From the forge's facts file");
    expect(document.querySelector("[title]")).toBeNull();
  });

  it("keeps a disabled item with a hint in reach, saying why, and pressing nothing", async () => {
    const pressed = vi.fn();
    render(
      <StatusItem isDisabled tooltip="This status line has no Inbox behind it." onPress={pressed}>
        Notices
      </StatusItem>,
    );
    const item = screen.getByRole("button", { name: "Notices" });
    expect(item).toHaveAttribute("aria-disabled", "true");
    expect(item).not.toBeDisabled();
    expect(item).toHaveAttribute("tabindex", "0");
    expect(item).toHaveAccessibleDescription("This status line has no Inbox behind it.");
    await user.click(item);
    item.focus();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");
    expect(pressed).not.toHaveBeenCalled();
  });
});

describe("the family's parts (ADR 0103)", () => {
  it("draws its surface as the tooltip's root, in its one look, never a native title", () => {
    render(<Path />);
    const target = screen.getByText("purlis");
    expect(tipOf(target)).toBe("/Users/me/work/purlis");
    expect(target).not.toHaveAttribute("title");
    fireEvent.keyDown(document.body, { key: "Tab" });
    act(() => target.focus());
    const tip = shownTip();
    expect(tip).toHaveAttribute("data-scope", "tooltip");
    expect(tip).toHaveAttribute("data-part", "root");
    // One look: no variant to key on.
    expect(tip).not.toHaveAttribute("data-variant");
  });
});

describe("a tooltip of a project behind another", () => {
  it("never opens: nothing of a project that is not in front is drawn over the window", async () => {
    render(
      <ProjectShownProvider value={false}>
        <Path />
      </ProjectShownProvider>,
    );
    const target = screen.getByText("purlis");
    await user.hover(target);
    await wait(TOOLTIP_DELAY * 4);
    expect(shownTip()).toBeNull();
    expect(tipOf(target)).toBeNull();
  });
});

describe("a button's tooltip", () => {
  it("is the hint on a button with words, and describes it", () => {
    render(<Button tooltip="Saves every file in the project">Save</Button>);
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).not.toHaveAttribute("title");
    expect(tipOf(save)).toBe("Saves every file in the project");
    fireEvent.keyDown(document.body, { key: "Tab" });
    act(() => save.focus());
    expect(save).toHaveAccessibleDescription("Saves every file in the project");
  });

  it("is an icon button's name, shown and not heard twice", () => {
    render(<IconButton aria-label="Close the tab" icon={<span>×</span>} />);
    const close = screen.getByRole("button", { name: "Close the tab" });
    expect(tipOf(close)).toBe("Close the tab");
    fireEvent.keyDown(document.body, { key: "Tab" });
    act(() => close.focus());
    // Named by its words (axe's aria-tooltip-name), and describing nothing.
    expect(shownTip()).toHaveAccessibleName("Close the tab");
    expect(shownTip()?.querySelector("[aria-hidden]")).toBeNull();
    expect(close).not.toHaveAttribute("aria-describedby");
  });

  it("puts an icon button's hint under its name, as its description", () => {
    render(
      <IconButton
        aria-label="Reset"
        tooltip="Takes the value this file sets out"
        icon={<span>↺</span>}
      />,
    );
    const reset = screen.getByRole("button", { name: "Reset" });
    fireEvent.keyDown(document.body, { key: "Tab" });
    act(() => reset.focus());
    const tip = shownTip();
    expect(tip?.querySelector("[aria-hidden]")).toBeNull();
    expect(tip?.querySelector('[data-part="detail"]')).toHaveTextContent(
      "Takes the value this file sets out",
    );
    expect(tip).toHaveTextContent(/^Reset/);
    expect(reset).toHaveAccessibleName("Reset");
    expect(reset).toHaveAccessibleDescription("Takes the value this file sets out");
  });

  it("does not say a toggle's name twice when the hint is its name", () => {
    render(
      <ToggleButton aria-label="Match case" tooltip="Match case">
        Aa
      </ToggleButton>,
    );
    const toggle = screen.getByRole("button", { name: "Match case" });
    expect(tipOf(toggle)).toBe("Match case");
    fireEvent.keyDown(document.body, { key: "Tab" });
    act(() => toggle.focus());
    expect(toggle).not.toHaveAttribute("aria-describedby");
  });

  it("never flips an unavailable toggle the call does not hold, and flips it once available", async () => {
    const { rerender } = render(
      <ToggleButton isDisabled tooltip="This project is read-only here.">
        Wrap
      </ToggleButton>,
    );
    const toggle = screen.getByRole("button", { name: "Wrap" });
    await user.click(toggle);
    toggle.focus();
    await user.keyboard(" ");
    await user.keyboard("{Enter}");
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    rerender(<ToggleButton tooltip="Wrap long lines">Wrap</ToggleButton>);
    await user.click(screen.getByRole("button", { name: "Wrap" }));
    expect(screen.getByRole("button", { name: "Wrap" })).toHaveAttribute("aria-pressed", "true");
  });

  it("keeps a disabled button with a hint in reach, saying why, and pressing nothing", async () => {
    const pressed = vi.fn();
    render(
      <Button isDisabled tooltip="This project is read-only here." onPress={pressed}>
        Publish
      </Button>,
    );
    const publish = screen.getByRole("button", { name: "Publish" });
    expect(publish).toHaveAttribute("aria-disabled", "true");
    expect(publish).toHaveAttribute("data-disabled", "true");
    expect(publish).not.toBeDisabled();
    expect(tipOf(publish)).toBe("This project is read-only here.");
    // Why, for good: a screen reader hears it whether or not the tooltip is up.
    expect(publish).toHaveAccessibleDescription("This project is read-only here.");
    await user.click(publish);
    publish.focus();
    await user.keyboard("{Enter}");
    expect(pressed).not.toHaveBeenCalled();
  });

  for (const variant of ["grants", "danger"] as const)
    it(`presses nothing as an unavailable ${variant} answer: a click, Enter, Space, a trigger around it, a form`, async () => {
      const pressed = vi.fn();
      const opened = vi.fn();
      const submitted = vi.fn((event: { preventDefault: () => void }) => event.preventDefault());
      const row = vi.fn();
      render(
        <form onSubmit={submitted}>
          <div role="group" onKeyDown={(event) => row(event.key)} onClick={() => row("click")}>
            <PopoverTrigger onOpenChange={opened}>
              <Button
                variant={variant}
                type="submit"
                isDisabled
                tooltip="This project is read-only here."
                onPress={pressed}
              >
                Answer
              </Button>
              <PopoverDialog aria-label="Answer">opened</PopoverDialog>
            </PopoverTrigger>
          </div>
        </form>,
      );
      const answer = screen.getByRole("button", { name: "Answer" });
      // Still its form's submit button (#3636), so ⌘⏎ finds it unavailable; its click is refused.
      expect(answer).toHaveAttribute("type", "submit");
      await user.click(answer);
      answer.focus();
      await user.keyboard("{Enter}");
      await user.keyboard(" ");
      expect(pressed).not.toHaveBeenCalled();
      expect(opened).not.toHaveBeenCalled();
      expect(screen.queryByText("opened")).toBeNull();
      expect(submitted).not.toHaveBeenCalled();
      expect(row).not.toHaveBeenCalledWith("click");
      expect(row).not.toHaveBeenCalledWith("Enter");
    });

  for (const variant of ["grants", "danger"] as const)
    it(`presses nothing as a raw unavailable ${variant} button with a reason, whatever its parents hand it`, async () => {
      const clicked = vi.fn();
      const opened = vi.fn();
      const row = vi.fn();
      render(
        <div role="group" onKeyDown={(event) => row(event.key)} onClick={() => row("click")}>
          <PopoverTrigger onOpenChange={opened}>
            <Tooltip tip="This project is read-only here.">
              <button className={variant} disabled onClick={clicked} onPointerDown={clicked}>
                Answer
              </button>
            </Tooltip>
            <PopoverDialog aria-label="Answer">opened</PopoverDialog>
          </PopoverTrigger>
        </div>,
      );
      const answer = screen.getByRole("button", { name: "Answer" });
      expect(answer).toHaveAttribute("aria-disabled", "true");
      expect(answer).toHaveAttribute("type", "button");
      expect(answer).toHaveAccessibleDescription("This project is read-only here.");
      await user.click(answer);
      answer.focus();
      await user.keyboard("{Enter}");
      await user.keyboard(" ");
      expect(clicked).not.toHaveBeenCalled();
      expect(opened).not.toHaveBeenCalled();
      expect(row).not.toHaveBeenCalledWith("click");
      expect(row).not.toHaveBeenCalledWith("Enter");
    });

  it("shows a raw icon-only button's name first, its hint under it, as an icon button does", () => {
    render(
      <Tooltip tip="This project is read-only here.">
        <button type="button" aria-label="New chat" disabled>
          <span aria-hidden="true">+</span>
        </button>
      </Tooltip>,
    );
    const plus = screen.getByRole("button", { name: "New chat" });
    expect(tipOf(plus)).toBe("New chatThis project is read-only here.");
    expect(plus).toHaveAccessibleDescription("This project is read-only here.");
  });

  it("leaves a disabled button without a hint out of reach, as it was", () => {
    render(<Button isDisabled>Publish</Button>);
    expect(screen.getByRole("button", { name: "Publish" })).toBeDisabled();
  });
});
