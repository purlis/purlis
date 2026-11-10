// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A settings page is the aligned form** (#2919, prototype A of `prototype/settings-page`, the
 * audit's R37–R62): the page's header names its file, each row's help is a line under its label,
 * and every row ends in the same action column.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Field, SettingGroup, SettingRow, SettingsLayout } from "./components";
import { MenuItem } from "../ui/menu/Menu";
import { SETTINGS_GROUPS, setGroupOpen, useFolds } from "./navGroups";

afterEach(cleanup);

function page(group: React.ReactNode) {
  render(
    <SettingsLayout
      levels={[
        { id: "you", label: "You" },
        { id: "project", label: "Project" },
      ]}
      level="project"
      onLevelChange={() => {}}
      groups={[{ id: "project.general", label: "General" }]}
      group="project.general"
      onGroupChange={() => {}}
      filter=""
      onFilterChange={() => {}}
    >
      {group}
    </SettingsLayout>,
  );
}

const versionLock = (
  <SettingRow
    label="Version lock"
    help="Hold everyone on this project to one purlis version."
    control={(ids) => <Field kind="text" ids={ids} value="" onChange={() => {}} />}
  />
);

describe("the side column (R38)", () => {
  it("holds the level switcher above the pages, with no title of its own (#3147)", () => {
    page(null);

    const side = screen.getByRole("radiogroup", { name: "Level" }).parentElement as HTMLElement;
    expect(side).toHaveClass("ui-settings-side");
    expect(within(side).getByRole("navigation", { name: "Groups" })).toBeInTheDocument();
    // The tab names the view: nothing in it says Settings of itself.
    expect(screen.queryByText("Settings", { exact: true })).toBeNull();
  });
});

describe("a page's header (R48, R49)", () => {
  it("is the page's title, its blurb and the file it is kept in, set as code", () => {
    page(
      <SettingGroup
        label="General"
        help="What a chat starts on when nothing else names it."
        kept="Kept in charter.toml, which your team sees."
        file="charter.toml"
      >
        {versionLock}
      </SettingGroup>,
    );

    const region = screen.getByRole("region", { name: "General" });
    expect(within(region).getByRole("heading", { level: 2, name: "General" })).toBeVisible();
    expect(
      within(region).getByText("What a chat starts on when nothing else names it."),
    ).toBeVisible();
    expect(within(region).getByText("charter.toml", { selector: "code" })).toBeVisible();
  });

  it("opens the file's text from Edit as TOML, after the file it names", async () => {
    const onEdit = vi.fn();
    page(
      <SettingGroup
        label="General"
        kept="Kept in charter.toml, which your team sees."
        file="charter.toml"
        onEdit={onEdit}
      >
        {versionLock}
      </SettingGroup>,
    );

    await userEvent.setup().click(screen.getByRole("button", { name: "Edit as TOML" }));

    expect(onEdit).toHaveBeenCalledTimes(1);
  });

  it("offers no Edit as TOML for a file with no text to edit", () => {
    page(
      <SettingGroup label="Appearance" kept="Kept in layout.json." file="layout.json">
        {versionLock}
      </SettingGroup>,
    );

    expect(screen.queryByRole("button", { name: "Edit as TOML" })).toBeNull();
    expect(screen.getByText("layout.json", { selector: "code" })).toBeVisible();
  });
});

describe("a row's help (R52)", () => {
  it("is a line under its label that describes the control, with no info mark", () => {
    page(<SettingGroup label="General">{versionLock}</SettingGroup>);

    const help = screen.getByText("Hold everyone on this project to one purlis version.");
    expect(help).toBeVisible();
    expect(help).not.toHaveClass("sr-only");
    expect(screen.getByRole("textbox", { name: "Version lock" })).toHaveAccessibleDescription(
      "Hold everyone on this project to one purlis version.",
    );
    expect(screen.queryByRole("img", { name: "About Version lock" })).toBeNull();
  });

  it("sits in the label's column, before the control's", () => {
    page(<SettingGroup label="General">{versionLock}</SettingGroup>);

    const help = screen.getByText("Hold everyone on this project to one purlis version.");
    const label = screen.getByText("Version lock", { selector: "label" });
    expect(help.closest(".ui-setting-name")).toBe(label.closest(".ui-setting-name"));
  });
});

/** A row on a page whose value is set here, with a Reset, a move and a key. */
function rowWithMenu({ set = true }: { set?: boolean } = {}) {
  const onReset = vi.fn();
  const onMove = vi.fn();
  page(
    <SettingGroup label="Saving">
      <SettingRow
        label="Mode"
        help="How far a save goes."
        modified={set}
        reset={{ label: "Reset", disabled: !set, onReset }}
        moves={
          <MenuItem id="local" onAction={onMove}>
            Move to charter.local.toml
          </MenuItem>
        }
        copyKey="plane.mode"
        control={(ids) => <Field kind="text" ids={ids} value="pr" onChange={() => {}} />}
      />
    </SettingGroup>,
  );
  return { onReset, onMove, user: userEvent.setup() };
}

const more = (label: string) => screen.getByRole("button", { name: `More for ${label}` });

describe("a row's ⋯ menu (R56–R59)", () => {
  it("is one button at the row's end, in the action column, with Reset, the moves and Copy key", async () => {
    const { user } = rowWithMenu();

    expect(more("Mode").closest(".ui-setting-act")).not.toBeNull();
    // Reset and Move are the menu's, never buttons of their own on a page.
    expect(screen.queryByRole("button", { name: "Reset" })).toBeNull();
    await user.click(more("Mode"));

    const menu = screen.getByRole("menu", { name: "More for Mode" });
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.textContent),
    ).toEqual(["Reset", "Move to charter.local.toml", "Copy key plane.mode"]);
    expect(within(menu).getAllByRole("separator")).toHaveLength(2);
  });

  it("says what it holds in its tooltip", async () => {
    const { user } = rowWithMenu();

    await user.tab();
    while (document.activeElement !== more("Mode")) await user.tab();

    expect(await screen.findByRole("tooltip")).toHaveTextContent("Reset, move to another file");
  });

  it("resets the value, closes, and puts the keyboard back on the control", async () => {
    const { onReset, user } = rowWithMenu();

    await user.click(more("Mode"));
    await user.click(screen.getByRole("menuitem", { name: "Reset" }));

    expect(onReset).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("textbox", { name: "Mode" })).toHaveFocus();
  });

  it("holds Reset out of reach while the value is not set at this level (R58)", async () => {
    const { onReset, user } = rowWithMenu({ set: false });

    await user.click(more("Mode"));
    const reset = screen.getByRole("menuitem", { name: "Reset" });
    expect(reset).toHaveAttribute("aria-disabled", "true");
    await user.click(reset);

    expect(onReset).not.toHaveBeenCalled();
  });

  it("moves the value from the menu", async () => {
    const { onMove, user } = rowWithMenu();

    await user.click(more("Mode"));
    await user.click(screen.getByRole("menuitem", { name: "Move to charter.local.toml" }));

    expect(onMove).toHaveBeenCalledTimes(1);
  });

  it("copies the key", async () => {
    const { user } = rowWithMenu();
    const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();

    await user.click(more("Mode"));
    await user.click(screen.getByRole("menuitem", { name: "Copy key plane.mode" }));

    expect(writeText).toHaveBeenCalledWith("plane.mode");
  });

  it("closes on Escape, back on its button", async () => {
    const { user } = rowWithMenu();

    await user.click(more("Mode"));
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("menu")).toBeNull();
    expect(more("Mode")).toHaveFocus();
  });

  it("is not drawn on a row with nothing to offer, whose column stays", () => {
    page(<SettingGroup label="General">{versionLock}</SettingGroup>);

    expect(screen.queryByRole("button", { name: /^More for / })).toBeNull();
    expect(document.querySelector(".ui-setting-act")).not.toBeNull();
  });
});

describe("the set-here mark (R53)", () => {
  it("says Set at this level under the pointer, and is no stop of the keyboard", async () => {
    const { user } = rowWithMenu();
    const mark = screen.getByRole("img", { name: "Set at this level" });

    await user.hover(document.body);
    await user.hover(mark);

    expect(await screen.findByRole("tooltip")).toHaveTextContent("Set at this level");
    expect(mark).not.toHaveAttribute("tabindex");
  });

  it("is drawn before the label, on a value set here only", () => {
    rowWithMenu({ set: false });

    expect(screen.queryByRole("img", { name: "Set at this level" })).toBeNull();
  });
});

describe("a page's groups (R60, R61)", () => {
  function sections() {
    page(
      <SettingGroup label="Saving" kept="Kept in charter.toml." file="charter.toml">
        <SettingGroup label="The project" level={3}>
          {versionLock}
        </SettingGroup>
        <SettingGroup
          label="Repo · ElasticPress"
          level={3}
          help="This repo's own settings, beside the project's."
        >
          <SettingRow
            label="ElasticPress: mode"
            control={(ids) => <Field kind="text" ids={ids} value="" onChange={() => {}} />}
          />
        </SettingGroup>
        <SettingGroup label="Branches" level={3} rarely={2}>
          <SettingRow
            label="Target branch"
            control={(ids) => <Field kind="text" ids={ids} value="" onChange={() => {}} />}
          />
          <SettingRow
            label="Save branch"
            control={(ids) => <Field kind="text" ids={ids} value="" onChange={() => {}} />}
          />
        </SettingGroup>
      </SettingGroup>,
    );
    return userEvent.setup();
  }

  it("heads each group with its title, and a group's one line under it", () => {
    sections();

    expect(screen.getByRole("heading", { level: 3, name: "The project" })).toBeVisible();
    const repo = screen.getByRole("region", { name: "Repo · ElasticPress" });
    expect(within(repo).getByText("This repo's own settings, beside the project's.")).toBeVisible();
  });

  it("folds a rarely used group, its count after its title and its chevron after both", async () => {
    const user = sections();

    const fold = screen.getByRole("button", { name: "Branches · 2" });
    expect(fold).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("textbox", { name: "Target branch", hidden: true })).not.toBeVisible();
    // Trailing (#2799 rule 1, the audit's deviation 2): the twist comes after the words.
    const twist = fold.querySelector('[data-part="twist"]') as HTMLElement;
    expect(twist.compareDocumentPosition(within(fold).getByText("Branches · 2"))).toBe(
      Node.DOCUMENT_POSITION_PRECEDING,
    );

    await user.click(fold);

    expect(fold).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("textbox", { name: "Target branch" })).toBeVisible();
  });
});

describe("the nav's groups (R41–R47)", () => {
  const NAV = [
    { id: "project.general", label: "General" },
    { id: "project.sandbox", label: "Sandbox" },
    { id: "project.sandbox.mine", label: "Your hosts", sub: true, count: 18 },
    { id: "project.sandbox.level", label: "Your level", sub: true },
    { id: "project.harness", label: "Harness & profiles" },
    { id: "project.harness.work", label: "work", sub: true, count: 2 },
    { id: "project.dispatch", label: "Dispatch" },
  ];

  /** The layout with the window's kept folds, as the Settings tab hands them. */
  function Kept(props: Omit<Parameters<typeof SettingsLayout>[0], "folds">) {
    return <SettingsLayout {...props} folds={useFolds()} />;
  }

  function nav(group = "project.general") {
    const onGroupChange = vi.fn();
    const view = render(
      <Kept
        levels={[{ id: "project", label: "Project" }]}
        level="project"
        onLevelChange={() => {}}
        groups={NAV}
        group={group}
        onGroupChange={onGroupChange}
        filter=""
        onFilterChange={() => {}}
      >
        {null}
      </Kept>,
    );
    return { onGroupChange, view, user: userEvent.setup() };
  }

  const pages = () =>
    within(screen.getByRole("navigation", { name: "Groups" }))
      .getAllByRole("button")
      .filter((one) => !one.hasAttribute("aria-expanded"))
      .map((one) => one.textContent);

  beforeEach(() => SETTINGS_GROUPS.forget());

  it("opens every group at first, Sandbox among them, each with its toggle (R42, R47)", () => {
    nav();

    expect(screen.getByRole("button", { name: "Collapse Sandbox" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(screen.getByRole("button", { name: "Collapse Harness & profiles" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(pages()).toEqual([
      "General",
      "Sandbox",
      "Your hosts 18",
      "Your level",
      "Harness & profiles",
      "work 2",
      "Dispatch",
    ]);
  });

  it("folds a group from its own toggle, apart from its page, and the count moves up (R42, R46)", async () => {
    const { onGroupChange, user } = nav();

    await user.click(screen.getByRole("button", { name: "Collapse Sandbox" }));

    expect(pages()).toEqual(["General", "Sandbox 18", "Harness & profiles", "work 2", "Dispatch"]);
    expect(onGroupChange).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Expand Sandbox" })).toHaveFocus();
  });

  it("remembers which groups are folded, as a new launch reads them (R47)", async () => {
    const { user, view } = nav();
    await user.click(screen.getByRole("button", { name: "Collapse Harness & profiles" }));
    view.unmount();

    nav();

    expect(pages()).toContain("Harness & profiles 2");
    expect(pages()).not.toContain("work 2");
    expect(SETTINGS_GROUPS.written()).toEqual({ folded: ["project.harness"] });
  });

  it("opens a group when one of its pages becomes current (R44)", () => {
    setGroupOpen("project.harness", false);
    nav("project.harness.work");

    expect(screen.getByRole("button", { name: "Collapse Harness & profiles" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^work/ })).toHaveAttribute("aria-current", "page");
  });

  it("opens a group when its own page is chosen (R45)", async () => {
    setGroupOpen("project.harness", false);
    const { onGroupChange, user } = nav();

    await user.click(screen.getByRole("button", { name: /^Harness & profiles/ }));

    expect(onGroupChange).toHaveBeenCalledWith("project.harness");
    expect(screen.getByRole("button", { name: "Collapse Harness & profiles" })).toBeInTheDocument();
  });

  it("reaches a group's toggle from its page with →, and back with ←", async () => {
    const { user } = nav();

    screen.getByRole("button", { name: "General" }).focus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("button", { name: "Sandbox" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Collapse Sandbox" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Expand Sandbox" })).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("button", { name: /^Sandbox/ })).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("button", { name: /^Harness & profiles/ })).toHaveFocus();
  });
});
