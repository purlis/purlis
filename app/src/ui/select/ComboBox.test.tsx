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
import { Form } from "../field/Field";
import { ComboBox } from "./ComboBox";

afterEach(cleanup);

const SKILLS = ["purlis:handoff", "purlis:persona", "deploy"];

function Box({ onSend, isDisabled }: { onSend?: (value: string) => void; isDisabled?: boolean }) {
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
      />
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
});
