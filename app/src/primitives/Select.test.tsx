// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The one select** (`primitives/Select.tsx`, #1860): Radix Select, drawn by `.ui-select` and the
 * one menu style. Held to what a person meets: its name, what it shows, the keyboard that opens
 * and picks, and that an empty value is an option like any other (Radix keeps `""` for "nothing
 * picked", so the primitive carries it across).
 */
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Select, type SelectOption } from "./Select";
import { chosenIn, pickIn } from "./select.testkit";

afterEach(cleanup);

const SCOPES: SelectOption[] = [
  { value: "", label: "Every workspace" },
  { value: "alpha", label: "alpha" },
  { value: "beta", label: "beta" },
  { value: "gone", label: "gone", disabled: true },
];

function Filter({ start = "", onPick }: { start?: string; onPick?: (to: string) => void }) {
  const [value, setValue] = useState(start);
  return (
    <Select
      aria-label="Filter by workspace"
      value={value}
      options={SCOPES}
      onValueChange={(to) => {
        setValue(to);
        onPick?.(to);
      }}
    />
  );
}

describe("the select", () => {
  it("is a named combobox in the one select style, showing the option it holds", () => {
    render(<Filter start="beta" />);
    const select = screen.getByRole("combobox", { name: "Filter by workspace" });
    expect(select).toHaveClass("ui-select");
    expect(select).toHaveTextContent("beta");
    expect(chosenIn(select)).toBe("beta");
  });

  it("holds an empty value as an option, and shows its words", () => {
    render(<Filter />);
    const select = screen.getByRole("combobox", { name: "Filter by workspace" });
    expect(select).toHaveTextContent("Every workspace");
    expect(chosenIn(select)).toBe("");
  });

  it("is reached by Tab, opened by Enter, and picks with the arrows and Enter", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Filter onPick={picked} />);
    await user.tab();
    const select = screen.getByRole("combobox", { name: "Filter by workspace" });
    expect(select).toHaveFocus();
    await user.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    expect(list.closest(".ui-menu")).not.toBeNull();
    expect(within(list).getByRole("option", { name: "Every workspace" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.keyboard("{ArrowDown}{Enter}");
    expect(picked).toHaveBeenLastCalledWith("alpha");
    expect(select).toHaveTextContent("alpha");
    expect(screen.queryByRole("listbox")).toBeNull();
    // Closed, the keyboard is back on the select.
    expect(select).toHaveFocus();
  });

  it("picks the empty value back, and never one out of reach", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Filter start="beta" onPick={picked} />);
    const select = screen.getByRole("combobox", { name: "Filter by workspace" });
    await pickIn(select, "Every workspace");
    expect(picked).toHaveBeenLastCalledWith("");
    await user.click(select);
    const gone = within(await screen.findByRole("listbox")).getByRole("option", { name: "gone" });
    expect(gone).toHaveAttribute("aria-disabled", "true");
    await user.keyboard("{Escape}");
    await expect(pickIn(select, "gone")).rejects.toThrow(/out of reach/);
    expect(picked).toHaveBeenCalledTimes(1);
  });

  it("says nothing is picked while it holds a value none of its options has", () => {
    render(
      <Select
        aria-label="Persona"
        value="nobody"
        placeholder="Pick a persona"
        options={SCOPES.slice(1)}
        onValueChange={() => {}}
      />,
    );
    const select = screen.getByRole("combobox", { name: "Persona" });
    expect(select).toHaveTextContent("Pick a persona");
    expect(select).toHaveAttribute("data-placeholder");
  });

  it("is named by a label's `for`, described by its help, and held off while disabled", () => {
    render(
      <>
        <label htmlFor="harness">Harness</label>
        <Select
          id="harness"
          aria-describedby="harness-help"
          value="claude"
          options={[{ value: "claude", label: "Claude Code" }]}
          disabled
          onValueChange={() => {}}
        />
        <p id="harness-help">The harness a new chat starts on.</p>
      </>,
    );
    const select = screen.getByRole("combobox", { name: "Harness" });
    expect(select).toHaveAccessibleDescription("The harness a new chat starts on.");
    expect(select).toBeDisabled();
  });
});
