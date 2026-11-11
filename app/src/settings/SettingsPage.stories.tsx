// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A settings page, the aligned form** (#2919, prototype A of `prototype/settings-page`, the
 * audit's R37–R62): the side column with the level switcher, the filter and the
 * pages; the page's header naming its file with Edit as TOML; rows whose label and help are one
 * column, whose controls all start at one x, and whose ⋯ menu stands in the same place on every
 * row; the set-here mark, a badge for a value kept elsewhere, a deprecated key and an inline
 * secondary action. Each story is drawn, held to axe, walked by the keyboard and pictured in
 * every pictured theme cell (`docs/component-stories.md`).
 */
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, screen, userEvent, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { Button } from "../ui/button/Button";
import { MenuItem } from "../ui/menu/Menu";
import { Choice, Field, SettingGroup, SettingRow, SettingsLayout } from "./components";

const PAGES = [
  { id: "general", label: "General" },
  { id: "saving", label: "Saving" },
  { id: "harness", label: "Harness & profiles" },
  { id: "sandbox", label: "Sandbox" },
  { id: "hosts", label: "Your hosts", sub: true },
  { id: "dispatch", label: "Dispatch" },
];

const KEPT = "Kept in purlis.toml, which your team sees.";

/** The General page of the Project level, as the prototype draws it. */
function Page({
  onEdit,
  onReset,
  narrow = false,
}: {
  onEdit: () => void;
  onReset: () => void;
  /** Drawn in a pane narrower than 728 px (R75). */
  narrow?: boolean;
}) {
  const [level, setLevel] = useState("project");
  const [page, setPage] = useState("general");
  const [filter, setFilter] = useState("");
  const [workspace, setWorkspace] = useState("smart-ide");
  const [persona, setPersona] = useState("steward");
  const [channel, setChannel] = useState("dev");
  const [lock, setLock] = useState("");
  const [autoSave, setAutoSave] = useState(true);
  const [share, setShare] = useState("local");
  const [folded, setFolded] = useState<readonly string[]>([]);
  return (
    // The view's body, as a Settings tab is drawn in it: the side column takes its height.
    <div className="view-body" style={{ blockSize: 680, inlineSize: narrow ? 640 : 1040 }}>
      <SettingsLayout
        levels={[
          { id: "you", label: "You" },
          { id: "project", label: "Project" },
          { id: "workspace", label: "Workspace" },
        ]}
        level={level}
        onLevelChange={setLevel}
        groups={PAGES}
        group={page}
        onGroupChange={setPage}
        filter={filter}
        onFilterChange={setFilter}
        folds={{
          isOpen: (id) => !folded.includes(id),
          setOpen: (id, open) =>
            setFolded((was) => (open ? was.filter((one) => one !== id) : [...was, id])),
        }}
      >
        <SettingGroup
          label="General"
          help="What a chat starts on when nothing else names it, the update channel and the version lock."
          kept={KEPT}
          file="purlis.toml"
          onEdit={onEdit}
        >
          <SettingGroup label="Starting chats" level={3}>
            <SettingRow
              label="Default workspace"
              help="A chat that names no workspace opens here."
              modified
              reset={{ label: "Reset", disabled: false, onReset }}
              copyKey="default.workspace"
              control={(ids) => (
                <Choice
                  kind="select"
                  ids={ids}
                  options={["smart-ide", "charter", "issues"].map((one) => ({
                    value: one,
                    label: one,
                  }))}
                  value={workspace}
                  onValueChange={setWorkspace}
                />
              )}
            />
            <SettingRow
              label="Default persona"
              help="Who a new chat is, unless it names someone."
              modified
              reset={{ label: "Reset", disabled: false, onReset }}
              copyKey="default.persona"
              control={(ids) => (
                <>
                  <Choice
                    kind="select"
                    ids={ids}
                    options={["steward", "devops", "nestjs-developer"].map((one) => ({
                      value: one,
                      label: one,
                    }))}
                    value={persona}
                    onValueChange={setPersona}
                  />
                  <Button variant="ghost" size="sm">
                    {`Show ${persona}`}
                  </Button>
                </>
              )}
            />
          </SettingGroup>
          <SettingGroup label="Updates" level={3}>
            <SettingRow
              label="Update channel"
              help="Dev gets every build that passes; stable gets releases."
              modified
              badge={{
                label: "This machine",
                says: "From purlis.local.toml, at the Project level: this machine only.",
              }}
              origin="From purlis.local.toml, at the Project level: this machine only."
              reset={{ label: "Reset", disabled: false, onReset }}
              moves={
                <MenuItem id="move-shared" onAction={() => {}}>
                  Move to purlis.toml
                </MenuItem>
              }
              copyKey="update.channel"
              control={(ids) => (
                <Choice
                  kind="select"
                  ids={ids}
                  options={["stable", "dev"].map((one) => ({ value: one, label: one }))}
                  value={channel}
                  onValueChange={setChannel}
                />
              )}
            />
            <SettingRow
              label="Version lock"
              help="Hold everyone on this project to one purlis version."
              reset={{ label: "Reset", disabled: true, onReset }}
              copyKey="version.lock"
              control={(ids) => (
                <Field
                  kind="text"
                  ids={ids}
                  value={lock}
                  placeholder="Not locked"
                  onChange={setLock}
                />
              )}
            />
            <SettingRow
              label="Auto-save"
              help="Save on its own after a quiet spell."
              modified
              reset={{ label: "Reset", disabled: false, onReset }}
              copyKey="plane.auto-save"
              control={(ids) => (
                <Choice kind="switch" ids={ids} checked={autoSave} onCheckedChange={setAutoSave} />
              )}
            />
          </SettingGroup>
          <SettingGroup label="Deprecated" level={3} rarely={1}>
            <SettingRow
              label="[memory] share"
              help="Replaced by memory scopes; kept until every machine has moved."
              deprecated
              modified
              reset={{ label: "Reset", disabled: false, onReset }}
              copyKey="memory.share"
              control={(ids) => (
                <Choice
                  kind="select"
                  ids={ids}
                  options={["local", "commit", "push"].map((one) => ({ value: one, label: one }))}
                  value={share}
                  onValueChange={setShare}
                />
              )}
            />
          </SettingGroup>
        </SettingGroup>
      </SettingsLayout>
    </div>
  );
}

/** The control the row named `label` draws: the one its `<label for>` names. */
function controlOf(canvasElement: HTMLElement, label: string): HTMLElement | null {
  const name = within(canvasElement).getByText(label, { selector: "label" });
  return document.getElementById((name as HTMLLabelElement).htmlFor);
}

const meta = {
  component: Page,
  args: { onEdit: fn(), onReset: fn() },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **The page**: the header names its file, then the rows. The keyboard goes from the pages to
 * Edit as TOML, then through each row's controls and its ⋯, never stopping on a help line or a
 * set-here mark.
 */
export const AlignedForm: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("heading", { level: 2, name: "General" })).toBeVisible();
    await expect(canvas.getByText("purlis.toml", { selector: "code" })).toBeVisible();
    await expect(canvas.getByRole("heading", { level: 3, name: "Starting chats" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Deprecated · 1" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );

    canvas.getByRole("button", { name: "General" }).focus();
    await userEvent.tab();
    const edit = canvas.getByRole("button", { name: "Edit as TOML" });
    await expect(edit).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(args.onEdit).toHaveBeenCalledTimes(1);

    await userEvent.tab();
    await expect(controlOf(canvasElement, "Default workspace")).toHaveFocus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "More for Default workspace" })).toHaveFocus();
    expectFocusInSight();

    // Every control on screen starts at one x, whatever its label says (R54); a folded group's
    // rows are drawn and hidden.
    const onScreen = (selector: string) =>
      [...canvasElement.querySelectorAll<HTMLElement>(selector)].filter(
        (one) => one.getClientRects().length > 0,
      );
    const starts = onScreen(".ui-setting-control").map((one) =>
      Math.round(one.getBoundingClientRect().left),
    );
    await expect(new Set(starts).size).toBe(1);
    // And every ⋯ stands in one place (R56).
    const ends = onScreen(".ui-setting-act button").map((one) =>
      Math.round(one.getBoundingClientRect().right),
    );
    await expect(new Set(ends).size).toBe(1);
  },
};

/**
 * **A row's ⋯ menu**, opened from the keyboard: Reset, a move, Copy key, with a rule between
 * each; Escape closes it on its button, and Reset puts the keyboard back on the control.
 */
export const RowMenu: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const more = canvas.getByRole("button", { name: "More for Update channel" });
    // From the row's control, as the keyboard comes: Tab onto the ⋯, Enter opens it.
    controlOf(canvasElement, "Update channel")?.focus();
    await userEvent.tab();
    await untilFocused(() => more, "Tab onto the row's ⋯");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "More for Update channel" });
    await expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.textContent),
    ).toEqual(["Reset", "Move to purlis.toml", "Copy key update.channel"]);
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => more, "back on the row's ⋯ after Escape");

    await userEvent.keyboard("{Enter}");
    await screen.findByRole("menu", { name: "More for Update channel" });
    await userEvent.keyboard("{Enter}");
    await expect(args.onReset).toHaveBeenCalledTimes(1);
    await untilFocused(
      () => controlOf(canvasElement, "Update channel"),
      "on the control its Reset reset",
    );
    expectFocusInSight();
  },
};

/**
 * **A rarely used group, folded** (R61): its title and its count, the chevron after both; the
 * keyboard opens it, and what it holds is drawn with the rest of the form.
 */
export const FoldedGroup: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const fold = canvas.getByRole("button", { name: "Deprecated · 1" });
    fold.focus();
    await userEvent.keyboard("{Enter}");
    await expect(fold).toHaveAttribute("aria-expanded", "true");
    await expect(canvas.getByText("deprecated", { selector: "[data-scope=badge]" })).toBeVisible();
    expectFocusInSight();
    await userEvent.tab();
    await expect(controlOf(canvasElement, "[memory] share")).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Reset out of reach**: the value is not set at this level (R58), drawn in the faint tier. */
export const ResetOutOfReach: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const more = canvas.getByRole("button", { name: "More for Version lock" });
    controlOf(canvasElement, "Version lock")?.focus();
    await userEvent.tab();
    await untilFocused(() => more, "Tab onto the row's ⋯");
    await userEvent.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "More for Version lock" });
    await expect(within(menu).getByRole("menuitem", { name: "Reset" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await userEvent.keyboard("{Enter}");
    await expect(args.onReset).not.toHaveBeenCalled();
    expectFocusInSight();
  },
};

/**
 * **Where Settings is narrow** (R75): under 728 px the pages are one sideways row over the page,
 * and each row's label goes over its control, its ⋯ still at the row's end.
 */
export const Narrow: Story = {
  args: { narrow: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const list = canvas.getByRole("navigation", { name: "Groups" }).querySelector("ul");
    await expect(getComputedStyle(list as Element).display).toBe("flex");
    await expect(getComputedStyle(list as Element).flexDirection).toBe("row");
    const name = canvas.getByText("Default workspace", { selector: "label" });
    const control = controlOf(canvasElement, "Default workspace") as HTMLElement;
    await expect(control.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      name.getBoundingClientRect().bottom - 1,
    );
    const more = canvas.getByRole("button", { name: "More for Default workspace" });
    more.focus();
    expectFocusInSight();
  },
};
