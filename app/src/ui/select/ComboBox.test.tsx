// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The combobox family** (`ui/select/ComboBox.tsx`, #2906): a box with the suggestions that match
 * in a list under it, keyboard first, what is typed kept as the value, and Enter sending its form
 * while the list is closed.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { drawnWith } from "../../cascade.testkit";
import { Form } from "../field/Field";
import { ComboBox } from "./ComboBox";

afterEach(cleanup);

const SKILLS = ["purlis:handoff", "purlis:persona", "deploy"];

function Box({
  onSend,
  isDisabled,
  isInvalid,
}: {
  onSend?: (value: string) => void;
  isDisabled?: boolean;
  isInvalid?: boolean;
}) {
  const [value, setValue] = useState("");
  return (
    <Form
      aria-label="Add a skill to ops"
      onSubmit={(event) => {
        event.preventDefault();
        onSend?.(value);
      }}
    >
      <ComboBox
        label="Add a skill"
        suggestions={SKILLS}
        value={value}
        onChange={setValue}
        isDisabled={isDisabled}
        isInvalid={isInvalid}
        aria-describedby={isInvalid ? "why" : undefined}
      />
      {isInvalid && <p id="why">ops has no such skill.</p>}
      <button type="submit">Add</button>
    </Form>
  );
}

const box = () => screen.getByRole("combobox", { name: "Add a skill" });

describe("the combobox", () => {
  it("is the family's box, with its chevron, named by its label", () => {
    render(<Box />);
    const root = box().closest('[data-scope="combobox"][data-part="root"]');
    expect(root).not.toBeNull();
    expect(root).toHaveAttribute("data-size", "md");
    expect(root?.querySelector('[data-part="chevron"] svg')).not.toBeNull();
  });

  it("opens every suggestion on ↓, and the arrows stop at its ends", async () => {
    const user = userEvent.setup();
    render(<Box />);
    await user.click(box());
    await user.keyboard("{ArrowDown}");
    const list = await screen.findByRole("listbox");
    expect(within(list).getAllByRole("option")).toHaveLength(SKILLS.length);
    const aimed = () => box().getAttribute("aria-activedescendant");
    const optionId = (name: string) => within(list).getByRole("option", { name }).id;
    await waitFor(() => expect(aimed()).toBe(optionId("purlis:handoff")));
    await user.keyboard("{ArrowUp}");
    expect(aimed()).toBe(optionId("purlis:handoff"));
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown}");
    expect(aimed()).toBe(optionId("deploy"));
  });

  it("narrows its suggestions as it is typed in, and Enter writes the one it is on", async () => {
    const user = userEvent.setup();
    render(<Box />);
    await user.click(box());
    await user.keyboard("purlis");
    const list = await screen.findByRole("listbox");
    await waitFor(() => expect(within(list).getAllByRole("option")).toHaveLength(2));
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    await waitFor(() => expect(box()).toHaveValue("purlis:persona"));
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
  });

  it("closes its list on Escape and keeps what was typed", async () => {
    const user = userEvent.setup();
    render(<Box />);
    await user.click(box());
    await user.keyboard("dep");
    await screen.findByRole("listbox");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    expect(box()).toHaveValue("dep");
    expect(box()).toHaveFocus();
  });

  it("keeps a name none of its suggestions has when it is left", async () => {
    const user = userEvent.setup();
    render(<Box />);
    await user.click(box());
    await user.keyboard("my-own");
    await user.tab();
    expect(box()).toHaveValue("my-own");
  });

  it("sends its form on Enter while its list is closed", async () => {
    const user = userEvent.setup();
    const sent = vi.fn();
    render(<Box onSend={sent} />);
    await user.click(box());
    await user.keyboard("my-own");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    await user.keyboard("{Enter}");
    await waitFor(() => expect(sent).toHaveBeenCalledWith("my-own"));
  });

  it("is held off when it is unavailable", () => {
    render(<Box isDisabled />);
    expect(box()).toBeDisabled();
    expect(box().closest('[data-scope="combobox"]')).toHaveAttribute("data-disabled");
  });

  it("says it is refused, heard with why, and draws the danger edge without moving (#3110)", async () => {
    const user = userEvent.setup();
    render(<Box isInvalid />);
    expect(box()).toHaveAttribute("aria-invalid", "true");
    expect(box()).toHaveAccessibleDescription("ops has no such skill.");
    const root = box().closest('[data-scope="combobox"][data-part="root"]') as Element;
    expect(root).toHaveAttribute("data-invalid");
    expect(drawnWith(root, "border-color")).toBe("var(--danger-base)");
    expect(drawnWith(root, "border")).toBe("1px solid var(--border-control)");
    // Still a combobox: ↓ opens its suggestions.
    await user.click(box());
    await user.keyboard("{ArrowDown}");
    expect(await screen.findByRole("listbox")).toBeInTheDocument();
  });

  it("says why in the field's error line, kept from the start and heard as it lands (#3334)", async () => {
    function Refusing() {
      const [value, setValue] = useState("");
      const refused = value !== "" && !SKILLS.some((one) => one.startsWith(value));
      return (
        <ComboBox
          label="Add a skill"
          suggestions={SKILLS}
          value={value}
          onChange={setValue}
          error={refused ? "ops has no skill by that name." : undefined}
        />
      );
    }
    render(<Refusing />);
    const field = box().closest('[data-scope="field"][data-part="root"]') as Element;
    const line = field.querySelector('[data-scope="field"][data-part="error"]') as Element;
    // The room is there before it says anything, a live region, so the words are heard as they
    // land and nothing moves.
    expect(line).toHaveAttribute("aria-live", "polite");
    expect(line).not.toHaveAttribute("role");
    expect(line).toBeEmptyDOMElement();
    expect(box()).not.toHaveAttribute("aria-invalid");
    expect(box()).not.toHaveAccessibleDescription();
    // Refused while the person types, not when they leave.
    await userEvent.type(box(), "zz");
    expect(box()).toHaveFocus();
    expect(field.querySelector('[data-part="error"]')).toBe(line);
    expect(line).not.toHaveAttribute("role");
    expect(line).toHaveTextContent("ops has no skill by that name.");
    expect(box()).toHaveAttribute("aria-invalid", "true");
    expect(box()).toHaveAccessibleDescription("ops has no skill by that name.");
    const root = box().closest('[data-scope="combobox"][data-part="root"]') as Element;
    expect(root).toHaveAttribute("data-invalid");
    // Allowed again: the line empties and stays.
    await userEvent.clear(box());
    expect(line).toBeEmptyDOMElement();
    expect(line).not.toHaveAttribute("role");
    expect(box()).not.toHaveAttribute("aria-invalid");
  });

  it("takes a reason with no words as no refusal", () => {
    render(
      <ComboBox label="Add a skill" suggestions={SKILLS} value="" onChange={() => {}} error="" />,
    );
    expect(box()).not.toHaveAttribute("aria-invalid");
    expect(box()).not.toHaveAccessibleDescription();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("is not refused unless it is told so", () => {
    render(<Box />);
    expect(box()).not.toHaveAttribute("aria-invalid");
    expect(box().closest('[data-scope="combobox"]')).not.toHaveAttribute("data-invalid");
  });
});
