// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The disclosure family** (#3675): a quiet trigger that says `aria-expanded` and names the
 * panel it controls, folded unless told, the owner's to hold, one twist that turns, and a form's
 * refused field inside a fold opening it. That nothing moves is held by the stories in a real
 * browser (`Disclosure.stories.tsx`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { Form, TextField } from "../field/Field";
import { Disclosure, DisclosureGroup } from "./Disclosure";

afterEach(cleanup);

const trigger = (name: string) => screen.getByRole("button", { name });

describe("a disclosure", () => {
  it("starts folded: its trigger says so and its panel is out of the reading order", () => {
    render(
      <Disclosure label="More: branch, folder, sign-in">
        <p>Branch</p>
      </Disclosure>,
    );
    const more = trigger("More: branch, folder, sign-in");
    expect(more).toHaveAttribute("aria-expanded", "false");
    expect(more).toHaveAttribute("data-variant", "quiet");
    const panel = document.getElementById(more.getAttribute("aria-controls") ?? "");
    expect(panel).not.toBeNull();
    expect(panel).toHaveAttribute("hidden");
    expect(screen.queryByRole("group")).toBeNull();
  });

  it("opens under its trigger, its panel named by it, and folds again", () => {
    render(
      <Disclosure label="Show SKILL.md">
        <pre>name: deploy</pre>
      </Disclosure>,
    );
    const show = trigger("Show SKILL.md");
    const twist = show.querySelector('[data-scope="disclosure"][data-part="twist"]');
    act(() => fireEvent.click(show));
    expect(show).toHaveAttribute("aria-expanded", "true");
    const panel = screen.getByRole("group", { name: "Show SKILL.md" });
    expect(panel).toHaveTextContent("name: deploy");
    // The trigger comes first in the reading order, its panel straight after.
    expect(show.compareDocumentPosition(panel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // One twist, turned, never swapped for another chevron.
    expect(show.querySelector('[data-scope="disclosure"][data-part="twist"]')).toBe(twist);
    expect(show.querySelectorAll("svg")).toHaveLength(1);
    act(() => fireEvent.click(show));
    expect(show).toHaveAttribute("aria-expanded", "false");
  });

  it("is never drawn as a trigger whose menu is open", () => {
    render(
      <Disclosure label="Show its last text" defaultExpanded>
        <p>text</p>
      </Disclosure>,
    );
    const show = trigger("Show its last text");
    expect(show).toHaveAttribute("aria-expanded", "true");
    expect(show).not.toHaveAttribute("data-open");
  });

  it("is the owner's to hold", () => {
    function Held() {
      const [open, setOpen] = useState(true);
      return (
        <>
          <p data-testid="kept">{open ? "open" : "folded"}</p>
          <Disclosure label="More" isExpanded={open} onExpandedChange={setOpen}>
            <p>fields</p>
          </Disclosure>
        </>
      );
    }
    render(<Held />);
    expect(trigger("More")).toHaveAttribute("aria-expanded", "true");
    act(() => fireEvent.click(trigger("More")));
    expect(screen.getByTestId("kept")).toHaveTextContent("folded");
    expect(trigger("More")).toHaveAttribute("aria-expanded", "false");
  });

  it("heads a section when told its level", () => {
    render(
      <Disclosure label="What it adds" level={3}>
        <p>files</p>
      </Disclosure>,
    );
    expect(screen.getByRole("heading", { level: 3, name: "What it adds" })).toContainElement(
      trigger("What it adds"),
    );
  });

  it("opens one at a time in a group", () => {
    render(
      <DisclosureGroup>
        <Disclosure id="skill" label="Show SKILL.md">
          <p>skill</p>
        </Disclosure>
        <Disclosure id="script" label="Show scripts/repro.sh">
          <p>script</p>
        </Disclosure>
      </DisclosureGroup>,
    );
    act(() => fireEvent.click(trigger("Show SKILL.md")));
    expect(trigger("Show SKILL.md")).toHaveAttribute("aria-expanded", "true");
    act(() => fireEvent.click(trigger("Show scripts/repro.sh")));
    expect(trigger("Show scripts/repro.sh")).toHaveAttribute("aria-expanded", "true");
    expect(trigger("Show SKILL.md")).toHaveAttribute("aria-expanded", "false");
  });
});

describe("a refused field inside a folded disclosure", () => {
  function AddSource({ onAdd }: { onAdd: () => void }) {
    const [folder, setFolder] = useState("/skills");
    return (
      <Form
        onSubmit={(event) => {
          event.preventDefault();
          onAdd();
        }}
      >
        <Disclosure label="More: branch, folder, sign-in">
          <TextField
            label="Folder in the repository"
            value={folder}
            onChange={setFolder}
            validate={(value) => (value.startsWith("/") ? "Without a leading /." : undefined)}
          />
        </Disclosure>
        <button type="submit">Add source</button>
      </Form>
    );
  }

  it("opens it, takes the keyboard, and is said once, by the form's summary (#3291)", () => {
    let added = 0;
    render(<AddSource onAdd={() => (added += 1)} />);
    const more = trigger("More: branch, folder, sign-in");
    expect(more).toHaveAttribute("aria-expanded", "false");
    const form = screen.getByRole("button", { name: "Add source" }).closest("form");
    if (form === null) throw new Error("Add source is in no form");
    act(() => fireEvent.submit(form));
    expect(added).toBe(0);
    expect(more).toHaveAttribute("aria-expanded", "true");
    const folder = screen.getByRole("textbox", { name: "Folder in the repository" });
    expect(folder).toHaveFocus();
    // One signal for the refusal: the form's one summary, and nothing the fold says of itself.
    // The field's own words are its description, quiet while the summary says it.
    const alerts = screen.getAllByRole("alert");
    expect(alerts).toHaveLength(1);
    expect(alerts[0]).toHaveTextContent("Folder in the repository: Without a leading /.");
    expect(folder).toHaveAccessibleDescription("Without a leading /.");
    expect(document.querySelectorAll('[aria-live]:not([data-part="error"])')).toHaveLength(0);
    expect(document.querySelector('[data-part="error"]')).toHaveAttribute("aria-live", "off");
  });
});
