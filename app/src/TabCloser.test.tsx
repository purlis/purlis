// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Closer } from "./StripMarks";
import type { Offer } from "./actions";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **A tab's `×` and a task's `−`, as the keyboard, the pointer and a screen reader meet them**
 * (#4756, #5341, #5445, #1489). Held before #2930 moved it off a raw `<button>` onto the tabs
 * family's own close, so the move changes none of it.
 */

afterEach(cleanup);

const close = (on: Partial<Offer> = {}): Offer => ({
  id: "tab.close:3",
  title: "Close steward 3",
  available: true,
  reason: "",
  does: { verb: "nothing" },
  note: "Its chat goes on in the sidebar.",
  ...on,
});

describe("a tab's close (#2930)", () => {
  it("is a button named by its row, its × hidden, the tabs family's closer part", () => {
    render(<Closer offer={close()} onPress={() => {}} reachable />);
    const button = screen.getByRole("button", { name: "Close steward 3" });
    expect(button.querySelector("svg.lucide-x")).toHaveAttribute("aria-hidden", "true");
    expect(button).toHaveAttribute("data-scope", "tabs");
    expect(button).toHaveAttribute("data-part", "closer");
    expect(button).not.toHaveAttribute("data-minimise");
    expect(button).not.toHaveClass("ui-tabs-control");
  });

  it("is a task's minimise where its row sends the talk back, said by a − and a data mark", () => {
    render(
      <Closer
        offer={close({
          title: "Send talk back to its row",
          does: { verb: "sendBack" } as Offer["does"],
        })}
        onPress={() => {}}
      />,
    );
    const button = screen.getByRole("button", { name: "Send talk back to its row" });
    expect(button.querySelector("svg.lucide-minus")).toHaveAttribute("aria-hidden", "true");
    expect(button).toHaveAttribute("data-minimise", "");
  });

  it("runs its row on a click, on Enter and on Space", async () => {
    const onPress = vi.fn();
    const offer = close();
    render(<Closer offer={offer} onPress={onPress} reachable />);
    const button = screen.getByRole("button", { name: "Close steward 3" });
    await userEvent.click(button);
    button.focus();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard(" ");
    expect(onPress).toHaveBeenCalledTimes(3);
    expect(onPress).toHaveBeenCalledWith(offer);
  });

  it("is a Tab stop on the front tab only (#5341)", async () => {
    const { rerender } = render(<Closer offer={close()} onPress={() => {}} reachable />);
    expect(screen.getByRole("button")).toHaveAttribute("tabindex", "0");
    await userEvent.tab();
    expect(screen.getByRole("button")).toHaveFocus();
    rerender(<Closer offer={close()} onPress={() => {}} />);
    expect(screen.getByRole("button")).toHaveAttribute("tabindex", "-1");
  });

  it("never takes the keyboard on a background tab's press, and does on the front tab's (#5445)", () => {
    // As Chromium presses a button: a mouse down that focuses it unless its default is
    // prevented, then the click.
    const pressed = (button: HTMLElement) => {
      if (fireEvent.mouseDown(button)) button.focus();
      fireEvent.mouseUp(button);
      fireEvent.click(button);
    };
    const { rerender } = render(
      <>
        <input aria-label="Where the keyboard was" />
        <Closer offer={close()} onPress={() => {}} />
      </>,
    );
    const where = screen.getByRole("textbox");
    where.focus();
    pressed(screen.getByRole("button"));
    expect(where).toHaveFocus();
    rerender(
      <>
        <input aria-label="Where the keyboard was" />
        <Closer offer={close()} onPress={() => {}} reachable />
      </>,
    );
    pressed(screen.getByRole("button"));
    expect(screen.getByRole("button")).toHaveFocus();
  });

  it("says what it does and that the chat goes on, in its tooltip (#4756)", () => {
    render(<Closer offer={close()} onPress={() => {}} reachable />);
    const tip = tipOf(screen.getByRole("button", { name: "Close steward 3" }));
    expect(tip).toEqual(expect.stringContaining("Close steward 3"));
    expect(tip).toEqual(expect.stringContaining("Its chat goes on in the sidebar."));
  });
});
