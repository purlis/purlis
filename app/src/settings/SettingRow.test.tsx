// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The quiet settings row** (DS-8, #1840; D3 on #1832): the label, the control and its Reset on
 * one line at one height; where the group keeps its values said once, in its header; a badge only
 * on a row whose value is kept elsewhere, the whole sentence in its tooltip; a mark on a value set
 * at this level. The Project level's "Move to…" menu is held in `ProjectLevel.test.tsx`, where
 * the files it moves between are.
 *
 * Heights are read through the window's cascade (`cascade.testkit.ts`): jsdom lays nothing out,
 * so that a select and its Reset come out the same height on screen is `settings-row.e2e.ts`'s.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { drawnWith } from "../cascade.testkit";
import { Choice, Field, SettingGroup, SettingRow, SettingsLayout } from "./components";

afterEach(cleanup);

const KEPT = "Kept in purlis.toml, which your team sees.";
const SAYS =
  "From purlis.local.toml, at the Project level: this machine only. purlis.toml has push, which this overrides.";

/** A group of two rows: Mode kept elsewhere than the group, with a value set here; Branch not. */
function Group() {
  return (
    <SettingGroup label="Saving" help="How far a save goes." kept={KEPT}>
      <SettingRow
        label="Mode"
        help="How far a save of the project goes."
        origin={SAYS}
        badge={{ label: "This machine", says: SAYS }}
        modified
        reset={{ label: "Reset", disabled: false, onReset: () => {} }}
        control={(ids) => (
          <Choice
            ids={ids}
            kind="select"
            options={[
              { value: "push", label: "push" },
              { value: "pr", label: "pr" },
            ]}
            value="pr"
            onValueChange={() => {}}
          />
        )}
      />
      <SettingRow
        label="Branch"
        help="The branch a save is meant to end up on."
        origin="Not set at this level, so the value beneath it is in force."
        control={(ids) => <Field ids={ids} kind="text" value="" onChange={() => {}} />}
      />
    </SettingGroup>
  );
}

/** The group on a settings page, as the Settings tab draws it. */
function Page() {
  return (
    <SettingsLayout
      levels={[{ id: "project", label: "Project" }]}
      level="project"
      onLevelChange={() => {}}
      groups={[{ id: "saving", label: "Saving" }]}
      group="saving"
      onGroupChange={() => {}}
      filter=""
      onFilterChange={() => {}}
    >
      <Group />
    </SettingsLayout>
  );
}

const rowOf = (label: string) =>
  screen.getByLabelText(label).closest(".ui-setting-row") as HTMLElement;

describe("the group header", () => {
  it("says where the group's values are kept once, in its one help line", () => {
    render(<Page />);
    const group = screen.getByRole("region", { name: "Saving" });

    expect(within(group).getAllByText(/Kept in/)).toHaveLength(1);
    expect(within(group).getByText(`How far a save goes. ${KEPT}`)).toBeVisible();
  });

  it("draws no line of where a value comes from under any row", () => {
    render(<Page />);

    for (const label of ["Mode", "Branch"]) {
      const row = rowOf(label);
      const lines = [...row.querySelectorAll("p")].filter(
        (one) => !one.classList.contains("sr-only"),
      );
      expect(lines.map((one) => one.textContent)).not.toContainEqual(
        expect.stringMatching(/^From |^Not set at this level|Kept in/),
      );
    }
  });

  it("still tells a screen reader where each value comes from, in the control's description", () => {
    render(<Page />);

    expect(screen.getByLabelText("Mode")).toHaveAccessibleDescription(
      `How far a save of the project goes. ${SAYS}`,
    );
    expect(screen.getByLabelText("Branch")).toHaveAccessibleDescription(
      /Not set at this level, so the value beneath it is in force\./,
    );
  });
});

describe("the badge", () => {
  it("is drawn only on the row whose value is kept elsewhere, named by its level", () => {
    render(<Page />);

    expect(
      within(rowOf("Mode")).getByText("This machine", { selector: ".ui-setting-badge" }),
    ).toBeVisible();
    expect(
      within(rowOf("Branch")).queryByText("This machine", { selector: ".ui-setting-badge" }),
    ).toBeNull();
    // Beside the name, never inside it: the control is still named by its label alone.
    expect(screen.getByRole("combobox", { name: "Mode" })).toBeInTheDocument();
  });

  it("shows the whole sentence in its tooltip when the keyboard reaches it, and Escape hides it", async () => {
    const user = userEvent.setup();
    render(<Page />);
    const badge = within(rowOf("Mode")).getByText("This machine", {
      selector: ".ui-setting-badge",
    });

    // The row's info mark comes first, then the badge.
    within(rowOf("Mode")).getByRole("img", { name: "About Mode" }).focus();
    await user.tab();
    expect(badge).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent(SAYS);
    expect(badge).toHaveAccessibleDescription(SAYS);

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
  });

  it("shows its tooltip under the pointer", async () => {
    const user = userEvent.setup();
    render(<Page />);

    // The pointer comes onto the badge from the page.
    await user.hover(document.body);
    await user.hover(
      within(rowOf("Mode")).getByText("This machine", { selector: ".ui-setting-badge" }),
    );

    expect(await screen.findByRole("tooltip")).toHaveTextContent(SAYS);
  });
});

describe("a field's own hint (D3, D-1840-2)", () => {
  it("draws no help line under any row: the group's is the only one", () => {
    render(<Page />);
    const group = screen.getByRole("region", { name: "Saving" });

    const shown = [...group.querySelectorAll("p")].filter(
      (one) => !one.classList.contains("sr-only"),
    );
    expect(shown.map((one) => one.textContent)).toEqual([`How far a save goes. ${KEPT}`]);
  });

  it("shows the hint in the label's info tooltip, and keeps it the control's description", async () => {
    const user = userEvent.setup();
    render(<Page />);
    const mark = within(rowOf("Branch")).getByRole("img", { name: "About Branch" });

    await user.hover(mark);

    expect(await screen.findByRole("tooltip")).toHaveTextContent(
      "The branch a save is meant to end up on.",
    );
    expect(screen.getByLabelText("Branch")).toHaveAccessibleDescription(
      /^The branch a save is meant to end up on\./,
    );
  });

  it("is reached by the keyboard, and Escape hides it", async () => {
    const user = userEvent.setup();
    render(<Page />);

    // From the nav, the next stop is the first row's info mark.
    screen.getByRole("button", { name: "Saving" }).focus();
    await user.tab();
    expect(within(rowOf("Mode")).getByRole("img", { name: "About Mode" })).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent(
      "How far a save of the project goes.",
    );
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
  });
});

describe("a form outside a settings page", () => {
  it("keeps a field's hint as a line under it, with no info mark", () => {
    render(<Group />);

    expect(screen.getByText("The branch a save is meant to end up on.")).toHaveClass(
      "ui-setting-help",
    );
    expect(screen.queryByRole("img", { name: /^About / })).toBeNull();
  });
});

describe("the modified mark", () => {
  it("is on a value set at this level, and on no other", () => {
    render(<Page />);

    expect(within(rowOf("Mode")).getByRole("img", { name: "Set at this level" })).toBeVisible();
    expect(within(rowOf("Branch")).queryByRole("img", { name: "Set at this level" })).toBeNull();
  });
});

describe("one line at one height", () => {
  it("draws the select, the field and Reset each one control tall, from the tokens", () => {
    render(<Page />);

    expect(drawnWith(screen.getByLabelText("Mode"), "block-size")).toBe("var(--size-control)");
    // The box is the field family's: its box is a control tall (#2905).
    const branch = screen.getByLabelText("Branch").parentElement as Element;
    expect(drawnWith(branch, "block-size")).toBe("var(--size-control)");
    expect(drawnWith(screen.getByRole("button", { name: "Reset" }), "block-size")).toBe(
      "var(--size-control)",
    );
  });

  it("puts the label, the control and Reset on the row's one line", () => {
    render(<Page />);
    const line = rowOf("Mode").querySelector(".ui-setting-line") as HTMLElement;

    expect(within(line).getByText("Mode")).toBeInTheDocument();
    expect(within(line).getByRole("combobox", { name: "Mode" })).toBeInTheDocument();
    expect(within(line).getByRole("button", { name: "Reset" })).toBeInTheDocument();
    expect(drawnWith(line, "display")).toBe("flex");
    expect(drawnWith(line, "align-items")).toBe("center");
  });
});
