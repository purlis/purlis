// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The select family** (`ui/select/Select.tsx`, #2906): React Aria's select, drawn by its parts,
 * its list on the menu's surface with the menu's rows, an empty value an option like any other,
 * and a long list filtered.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { drawnWith } from "../../cascade.testkit";
import { Dialog } from "../dialog/Dialog";
import { FILTERED_FROM, Select, type SelectOption } from "./Select";
import { chosenIn, offeredIn, pickIn, selectNamed } from "./select.testkit";

afterEach(cleanup);

const SCOPES: SelectOption[] = [
  { value: "", label: "Every workspace" },
  { value: "alpha", label: "alpha" },
  { value: "beta", label: "beta" },
  { value: "gone", label: "gone", isDisabled: true },
];

function Filter({
  start = "",
  options = SCOPES,
  onPick,
  placeholder,
}: {
  start?: string;
  options?: SelectOption[];
  onPick?: (to: string) => void;
  placeholder?: string;
}) {
  const [value, setValue] = useState(start);
  return (
    <Select
      aria-label="Filter by workspace"
      value={value}
      options={options}
      placeholder={placeholder}
      onValueChange={(to) => {
        setValue(to);
        onPick?.(to);
      }}
    />
  );
}

describe("the select", () => {
  it("is a named button that opens a listbox, drawn by the family's parts", () => {
    render(<Filter start="beta" />);
    const select = selectNamed("Filter by workspace");
    expect(select).toHaveAttribute("aria-haspopup", "listbox");
    expect(select).toHaveAttribute("data-scope", "select");
    expect(select).toHaveAttribute("data-part", "root");
    expect(select).toHaveAttribute("data-size", "md");
    expect(select).toHaveTextContent("beta");
    expect(chosenIn(select)).toBe("beta");
    // R64: the chevron is drawn, a part of its own, and no screen reader hears it.
    const chevron = select.querySelector('[data-part="chevron"]');
    expect(chevron).not.toBeNull();
    expect(chevron).toHaveAttribute("aria-hidden", "true");
    expect(chevron?.querySelector("svg")).not.toBeNull();
  });

  it("is the text input's box, a control tall, ringed outside it from the keyboard", () => {
    render(<Filter start="beta" />);
    const select = selectNamed("Filter by workspace");
    expect(drawnWith(select, "block-size")).toBe("var(--size-control)");
    expect(drawnWith(select, "background-color")).toBe("var(--surface-sunken)");
    expect(drawnWith(select, "border")).toBe("1px solid var(--border-control)");
    expect(drawnWith(select, "border-radius")).toBe("var(--radius-control)");
    // R64: the chevron and a placeholder in the quiet tier.
    const chevron = select.querySelector('[data-part="chevron"]') as Element;
    expect(drawnWith(chevron, "color")).toBe("var(--text-quiet)");
    const keyboard = { focusVisible: true };
    expect(drawnWith(select, "outline", keyboard)).toBe(
      "var(--focus-ring-width) solid var(--focus-ring)",
    );
    expect(drawnWith(select, "outline-offset", keyboard)).toBe("var(--focus-ring-offset)");
  });

  it("holds an empty value as an option, and shows its words", () => {
    render(<Filter />);
    const select = selectNamed("Filter by workspace");
    expect(select).toHaveTextContent("Every workspace");
    expect(chosenIn(select)).toBe("");
  });

  it("shows its placeholder in its own part while it holds a value no option has", () => {
    render(<Filter start="nowhere" placeholder="Move to…" />);
    const select = selectNamed("Filter by workspace");
    const placeholder = select.querySelector('[data-part="placeholder"]') as Element;
    expect(placeholder).toHaveTextContent("Move to…");
    expect(drawnWith(placeholder, "color")).toBe("var(--text-quiet)");
    expect(chosenIn(select)).toBe("");
  });

  it("is reached by Tab, opened by Enter, and picks with the arrows and Enter", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Filter onPick={picked} />);
    await user.tab();
    const select = selectNamed("Filter by workspace");
    expect(select).toHaveFocus();
    await user.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    // It opens on the option it holds.
    await waitFor(() =>
      expect(within(list).getByRole("option", { name: "Every workspace" })).toHaveFocus(),
    );
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    expect(picked).toHaveBeenLastCalledWith("beta");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    // The keyboard is handed back once the list has gone, which is after its exit.
    await waitFor(() => expect(select).toHaveFocus());
  });

  it("stops at the ends of its list and does not wrap", async () => {
    const user = userEvent.setup();
    render(<Filter start="beta" />);
    act(() => selectNamed("Filter by workspace").focus());
    await user.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    await waitFor(() => expect(within(list).getByRole("option", { name: "beta" })).toHaveFocus());
    // `gone` is out of reach: beta is the last option the keyboard stops on.
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(within(list).getByRole("option", { name: "beta" })).toHaveFocus();
    await user.keyboard("{ArrowUp}{ArrowUp}{ArrowUp}{ArrowUp}");
    expect(within(list).getByRole("option", { name: "Every workspace" })).toHaveFocus();
  });

  it("closes on Escape and leaves its value as it was", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Filter start="alpha" onPick={picked} />);
    const select = selectNamed("Filter by workspace");
    act(() => select.focus());
    await user.keyboard("{Enter}");
    await screen.findByRole("listbox");
    await user.keyboard("{ArrowDown}{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    expect(picked).not.toHaveBeenCalled();
    expect(chosenIn(select)).toBe("alpha");
  });

  it("hands Escape on to what it stands in while its list is closed (D-2906-7)", async () => {
    const user = userEvent.setup();
    const heard = vi.fn();
    render(
      <div onKeyDown={(event) => heard(event.key)}>
        <Filter start="alpha" />
      </div>,
    );
    act(() => selectNamed("Filter by workspace").focus());
    await user.keyboard("{Escape}");
    expect(heard).toHaveBeenCalledWith("Escape");
  });

  it("closes only its list on Escape inside a dialog, and the dialog on the next", async () => {
    const user = userEvent.setup();
    const closed = vi.fn();
    render(
      <Dialog onClose={closed} title="New workspace">
        <Filter start="alpha" />
      </Dialog>,
    );
    const select = selectNamed("Filter by workspace");
    await waitFor(() => expect(select).toHaveFocus());
    await user.keyboard("{Enter}");
    await screen.findByRole("listbox");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    expect(closed).not.toHaveBeenCalled();
    await waitFor(() => expect(select).toHaveFocus());
    await user.keyboard("{Escape}");
    expect(closed).toHaveBeenCalledTimes(1);
  });

  it("keeps an option out of reach with a reason in the arrows' way, saying why, and never picks it (D-2906-11)", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(
      <Filter
        start="alpha"
        onPick={picked}
        options={[
          { value: "alpha", label: "alpha" },
          { value: "aider", label: "Aider", isDisabled: true, tooltip: "Aider is not installed" },
        ]}
      />,
    );
    const select = selectNamed("Filter by workspace");
    act(() => select.focus());
    await user.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    const aider = within(list).getByRole("option", { name: "Aider" });
    expect(aider).toHaveAttribute("aria-disabled", "true");
    expect(aider).toHaveAccessibleDescription("Aider is not installed");
    await user.keyboard("{ArrowDown}");
    await waitFor(() => expect(aider).toHaveFocus());
    await user.keyboard("{Enter}");
    await user.click(aider);
    expect(picked).not.toHaveBeenCalled();
    // Still open on it: nothing was picked, and Escape puts the list away.
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    expect(chosenIn(select)).toBe("alpha");
    await expect(offeredIn(select)).resolves.toEqual(["alpha", "Aider"]);
  });

  it("draws its list on the menu's surface, each option a menu row with its tick", async () => {
    const user = userEvent.setup();
    render(<Filter start="alpha" />);
    act(() => selectNamed("Filter by workspace").focus());
    await user.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    const surface = list.closest('[data-scope="popover"][data-part="root"]');
    expect(surface).toHaveAttribute("data-variant", "menu");
    expect(surface).toHaveAttribute("data-of", "select");
    const alpha = within(list).getByRole("option", { name: "alpha" });
    expect(alpha).toHaveAttribute("data-scope", "menu");
    expect(alpha).toHaveAttribute("data-part", "root");
    expect(alpha).toHaveAttribute("aria-selected", "true");
    expect(alpha.querySelector('[data-part="indicator"] svg')).not.toBeNull();
    const beta = within(list).getByRole("option", { name: "beta" });
    expect(beta.querySelector('[data-part="indicator"] svg')).toBeNull();
  });

  it("refuses an option out of reach", async () => {
    render(<Filter />);
    await expect(pickIn(selectNamed("Filter by workspace"), "gone")).rejects.toThrow(
      "out of reach",
    );
  });

  it("is held off when it is unavailable", async () => {
    render(
      <Select
        aria-label="Level"
        value="a"
        options={[{ value: "a", label: "A" }]}
        isDisabled
        onValueChange={() => {}}
      />,
    );
    const select = selectNamed("Level");
    expect(select).toBeDisabled();
    expect(select).toHaveAttribute("data-disabled");
  });

  it("keeps an unavailable select with a reason in reach, saying why, and opens nothing", async () => {
    const user = userEvent.setup();
    render(
      <Select
        aria-label="Level"
        value="a"
        options={[{ value: "a", label: "A" }]}
        isDisabled
        tooltip="purlis opens the project read-only."
        onValueChange={() => {}}
      />,
    );
    const select = selectNamed("Level");
    expect(select).toHaveAttribute("aria-disabled", "true");
    expect(select).toHaveAccessibleDescription("purlis opens the project read-only.");
    await user.tab();
    expect(select).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("says it is refused, heard with why, and draws the danger edge without moving (#3110)", async () => {
    const user = userEvent.setup();
    render(
      <>
        <Select
          aria-label="Harness"
          aria-describedby="why"
          value="a"
          options={[
            { value: "a", label: "A" },
            { value: "b", label: "B" },
          ]}
          isInvalid
          onValueChange={() => {}}
        />
        <p id="why">Pick the harness this persona runs.</p>
      </>,
    );
    const select = selectNamed("Harness");
    expect(select).toHaveAttribute("aria-invalid", "true");
    expect(select).toHaveAttribute("data-invalid");
    expect(select).toHaveAccessibleDescription("Pick the harness this persona runs.");
    expect(drawnWith(select, "border-color")).toBe("var(--danger-base)");
    // A colour only: the box keeps its size and its edge's width.
    expect(drawnWith(select, "block-size")).toBe("var(--size-control)");
    expect(drawnWith(select, "border")).toBe("1px solid var(--border-control)");
    // Still a select: Enter opens it and the arrows pick.
    await user.tab();
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("listbox")).toBeInTheDocument();
  });

  it("is not refused unless it is told so", () => {
    render(<Filter start="beta" />);
    const select = selectNamed("Filter by workspace");
    expect(select).not.toHaveAttribute("aria-invalid");
    expect(select).not.toHaveAttribute("data-invalid");
  });

  it("draws at the size it is given", () => {
    render(
      <Select
        aria-label="Page size"
        size="sm"
        value="a"
        options={[{ value: "a", label: "A" }]}
        onValueChange={() => {}}
      />,
    );
    expect(selectNamed("Page size")).toHaveAttribute("data-size", "sm");
  });
});

describe("a long select", () => {
  const MANY: SelectOption[] = Array.from({ length: FILTERED_FROM + 2 }, (_, at) => ({
    value: `ws-${at}`,
    label: at === 7 ? "devops" : `workspace ${at}`,
  }));

  it("opens with a filter that takes the keyboard and narrows the list", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Filter start="ws-0" options={MANY} onPick={picked} />);
    act(() => selectNamed("Filter by workspace").focus());
    await user.keyboard("{Enter}");
    const filter = await screen.findByRole("textbox", { name: "Filter" });
    await waitFor(() => expect(filter).toHaveFocus());
    await user.keyboard("dev");
    const list = screen.getByRole("listbox");
    await waitFor(() => expect(within(list).getAllByRole("option")).toHaveLength(1));
    await user.keyboard("{Enter}");
    expect(picked).toHaveBeenLastCalledWith("ws-7");
  });

  it("names only what is drawn, open after open: every id it points at is in the page (D-2906-14)", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Filter start="" options={MANY} onPick={picked} />);
    const select = selectNamed("Filter by workspace");
    /** Each id an element of the page points at that no element has. */
    const dangling = () =>
      [
        ...document.querySelectorAll("[aria-controls], [aria-activedescendant], [aria-labelledby]"),
      ].flatMap((one) =>
        ["aria-controls", "aria-activedescendant", "aria-labelledby"].flatMap((name) =>
          (one.getAttribute(name) ?? "")
            .split(/\s+/)
            .filter((id) => id !== "" && document.getElementById(id) === null)
            .map((id) => `${name}=${id}`),
        ),
      );
    for (const round of ["first", "again"]) {
      act(() => select.focus());
      await user.keyboard("{Enter}");
      const filter = await screen.findByRole("textbox", { name: "Filter" });
      await waitFor(() => expect(filter).toHaveFocus());
      expect(dangling(), `${round}, open`).toEqual([]);
      await user.keyboard("dev");
      // At once, while React Aria holds back moving its aim so the letter typed is heard.
      expect(dangling(), `${round}, typed`).toEqual([]);
      const devops = await screen.findByRole("option", { name: "devops" });
      await waitFor(() => expect(filter).toHaveAttribute("aria-activedescendant", devops.id));
      expect(dangling(), `${round}, narrowed`).toEqual([]);
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
      await waitFor(() => expect(select).toHaveFocus());
      expect(dangling(), `${round}, closed`).toEqual([]);
    }
    expect(picked).not.toHaveBeenCalled();
    // Narrowed, Enter picks the one drawn.
    await user.keyboard("{Enter}");
    await screen.findByRole("textbox", { name: "Filter" });
    await user.keyboard("dev{Enter}");
    expect(picked).toHaveBeenLastCalledWith("ws-7");
  });

  it("offers every option while nothing is typed", async () => {
    render(<Filter start="ws-0" options={MANY} />);
    expect(await offeredIn(selectNamed("Filter by workspace"))).toHaveLength(MANY.length);
  });

  it("a short one has no filter", async () => {
    const user = userEvent.setup();
    render(<Filter />);
    act(() => selectNamed("Filter by workspace").focus());
    await user.keyboard("{Enter}");
    await screen.findByRole("listbox");
    expect(screen.queryByRole("textbox", { name: "Filter" })).toBeNull();
  });
});
