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
import { Choice, Field, SettingGroup, SettingRow } from "./components";

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

const rowOf = (label: string) =>
  screen.getByLabelText(label).closest(".ui-setting-row") as HTMLElement;

describe("the group header", () => {
  it("says where the group's values are kept once, in its one help line", () => {
    render(<Group />);
    const group = screen.getByRole("region", { name: "Saving" });

    expect(within(group).getAllByText(/Kept in/)).toHaveLength(1);
    expect(within(group).getByText(`How far a save goes. ${KEPT}`)).toBeVisible();
  });

  it("draws no line of where a value comes from under any row", () => {
    render(<Group />);

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
    render(<Group />);

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
    render(<Group />);

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
    render(<Group />);
    const badge = within(rowOf("Mode")).getByText("This machine", {
      selector: ".ui-setting-badge",
    });

    await user.tab();
    expect(badge).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent(SAYS);
    expect(badge).toHaveAccessibleDescription(SAYS);

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
  });

  it("shows its tooltip under the pointer", async () => {
    const user = userEvent.setup();
    render(<Group />);

    await user.hover(
      within(rowOf("Mode")).getByText("This machine", { selector: ".ui-setting-badge" }),
    );

    expect(await screen.findByRole("tooltip")).toHaveTextContent(SAYS);
  });
});

describe("the modified mark", () => {
  it("is on a value set at this level, and on no other", () => {
    render(<Group />);

    expect(within(rowOf("Mode")).getByRole("img", { name: "Set at this level" })).toBeVisible();
    expect(within(rowOf("Branch")).queryByRole("img", { name: "Set at this level" })).toBeNull();
  });
});

describe("one line at one height", () => {
  it("draws the select, the field and Reset each one control tall, from the tokens", () => {
    render(<Group />);

    expect(drawnWith(screen.getByLabelText("Mode"), "block-size")).toBe("var(--size-control)");
    expect(drawnWith(screen.getByLabelText("Branch"), "block-size")).toBe("var(--size-control)");
    expect(drawnWith(screen.getByRole("button", { name: "Reset" }), "block-size")).toBe(
      "var(--size-control)",
    );
  });

  it("puts the label, the control and Reset on the row's one line", () => {
    render(<Group />);
    const line = rowOf("Mode").querySelector(".ui-setting-line") as HTMLElement;

    expect(within(line).getByText("Mode")).toBeInTheDocument();
    expect(within(line).getByRole("combobox", { name: "Mode" })).toBeInTheDocument();
    expect(within(line).getByRole("button", { name: "Reset" })).toBeInTheDocument();
    expect(drawnWith(line, "display")).toBe("flex");
    expect(drawnWith(line, "align-items")).toBe("center");
  });
});
