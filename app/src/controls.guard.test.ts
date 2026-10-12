// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { CONTROLS_ALLOWANCE } from "./controls.guard.allowance";
import {
  classNameOf,
  classWords,
  isView,
  openingTags,
  ratchet,
  SRC,
  sourceFiles,
  withoutComments,
} from "./guards.testkit";

/**
 * **Every control wears its shared class** (DS-4, #1837; ADR 0094 §4, `docs/ui-primitives.md`
 * "The control classes").
 *
 * A control drawn without its class is drawn by whatever rule its view wrote for it, which is
 * how the window came to have about 30 button looks. So a component that draws one of these
 * gives it the class that is the whole of its look:
 *
 * | Drawn as                                                         | Wears                     |
 * | ---------------------------------------------------------------- | ------------------------- |
 * | `<button>`, or a Radix trigger or close without `asChild`        | `ui-button` (or a row, a segment or a menu row, which a button can be) |
 * | `<input>`, `<textarea>`                                          | `ui-input`; a checkbox `ui-checkbox`, a radio `ui-radio` |
 * | `<select>`, Radix Select's `Trigger`                             | `ui-select`               |
 * | `<table>`                                                        | `ui-table`                |
 * | a Radix menu's or Select's `Content` or `SubContent`             | `ui-menu`                 |
 * | a Radix menu's or Select's `Item`, `SubTrigger`, checkbox or radio item | `ui-menu-item`     |
 *
 * A Radix part is found by the name its file imports the package under (`import * as Menu from
 * "@radix-ui/react-dropdown-menu"`), so an alias is no way round. A hidden input draws nothing
 * and needs no class.
 *
 * **Today's offenders are allowed, one per line, and the list can only shrink**
 * (`controls.guard.allowance.ts`), the same ratchet as the geometry guard: an entry names the
 * control by its file and its tag as written (`<button className="tab-close">`), never by line;
 * the list must match the source exactly; and no file's list may grow past its frozen
 * `CEILING`. The surface tickets delete an entry as each control takes its class.
 */

/** The classes that are each kind's whole look. A button can also be a list row, a tree row, a
 *  segment or a menu's row (`docs/ui-primitives.md`). */
const BUTTON = ["ui-button", "ui-row", "ui-tree-row", "ui-segmented-item", "ui-menu-item"];
const WEARS: Record<string, string[]> = {
  button: BUTTON,
  input: ["ui-input"],
  "input:checkbox": ["ui-checkbox"],
  "input:radio": ["ui-radio"],
  "input:dynamic": ["ui-input", "ui-checkbox", "ui-radio"],
  textarea: ["ui-input"],
  select: ["ui-select"],
  table: ["ui-table"],
  menu: ["ui-menu"],
  "menu-item": ["ui-menu-item"],
};

/** Radix packages whose parts are controls, and what each part is. */
const RADIX_MENUS = new Set(["dropdown-menu", "context-menu", "menubar"]);
const MENU_CONTENT = new Set(["Content", "SubContent"]);
const MENU_ITEM = new Set(["Item", "SubTrigger", "CheckboxItem", "RadioItem"]);
const BUTTON_PARTS = new Set(["Trigger", "Close", "Cancel", "Action"]);

/** Each Radix namespace a file imports, by the name it uses: `{ Menu: "dropdown-menu" }`. */
function radixNames(text: string): Map<string, string> {
  return new Map(
    [...text.matchAll(/import\s+\*\s+as\s+(\w+)\s+from\s+["']@radix-ui\/react-([\w-]+)["']/g)].map(
      ([, name, pkg]) => [name, pkg],
    ),
  );
}

/** What kind of control a tag draws, by `WEARS`' keys, or `undefined` when it is none. */
function kindOf(name: string, tag: string, radix: ReadonlyMap<string, string>): string | undefined {
  if (name === "input") {
    const type = /\stype=(?:"([^"]*)"|'([^']*)'|(\{))/.exec(tag);
    if (!type) return "input";
    if (type[3]) return "input:dynamic";
    const value = type[1] ?? type[2];
    if (value === "hidden") return undefined;
    if (value === "checkbox" || value === "radio") return `input:${value}`;
    return "input";
  }
  if (["button", "textarea", "select", "table"].includes(name)) return name;
  const [space, part] = name.split(".");
  const pkg = part ? radix.get(space) : undefined;
  if (!pkg) return undefined;
  if (RADIX_MENUS.has(pkg) || pkg === "select") {
    if (MENU_CONTENT.has(part)) return "menu";
    if (MENU_ITEM.has(part)) return "menu-item";
  }
  if (pkg === "select" && part === "Trigger") return "select";
  if (BUTTON_PARTS.has(part) && !/\sasChild\b/.test(tag)) return "button";
  return undefined;
}

/** Each control a component draws without its class, as its allowance entry: the tag's name,
 *  an input's type, and its `className` as written. */
export function unclassedControls(source: string): string[] {
  const radix = radixNames(withoutComments(source, "ts"));
  return openingTags(source, /^(?:button|input|textarea|select|table|\w+\.\w+)$/).flatMap(
    ({ name, text }) => {
      const kind = kindOf(name, text, radix);
      if (!kind) return [];
      const className = classNameOf(text);
      const words = classWords(className);
      if (WEARS[kind].some((wanted) => words.includes(wanted))) return [];
      const type =
        kind.startsWith("input:") && kind !== "input:dynamic" ? ` type="${kind.slice(6)}"` : "";
      const written =
        className === undefined
          ? ""
          : /\sclassName=\{/.test(text)
            ? ` className={${className}}`
            : ` className="${className}"`;
      return [`<${name}${type}${written}>`];
    },
  );
}

/** Every unclassed control in the window today, by file. */
function unclassedByFile(): Map<string, string[]> {
  return new Map(
    sourceFiles(isView)
      .map((file) => [file, unclassedControls(readFileSync(join(SRC, file), "utf8"))] as const)
      .filter(([, found]) => found.length > 0),
  );
}

/**
 * How long each file's list was when the guard landed: a list may only get shorter than this.
 * A frozen record, not a count to keep current: lowering one is never needed, and raising one,
 * or naming a new file, is adding a control the guard exists to refuse.
 */
const CEILING: Record<string, number> = {
  "About.tsx": 3,
  "ActivityBar.tsx": 1,
  "ActivityTab.tsx": 3,
  "AddToAChat.tsx": 1,
  "AnswerQuestion.tsx": 4,
  "App.tsx": 2,
  "ApproveExtension.tsx": 2,
  "ApprovePlane.tsx": 2,
  "AskPersona.tsx": 2,
  "AwaySummary.tsx": 2,
  "Brief.tsx": 5,
  "ChangeActions.tsx": 2,
  "ChatAsk.tsx": 2,
  "ChatStrip.tsx": 2,
  "ChatsSection.tsx": 6,
  "ClosingProject.tsx": 2,
  "Cockpit.tsx": 4,
  "CreateOnForge.tsx": 6,
  "DeleteVault.tsx": 2,
  "DeleteWorkspace.tsx": 3,
  "DispatchesTab.tsx": 11,
  "Doctor.tsx": 5,
  "DoorPage.tsx": 1,
  "EndingChat.tsx": 3,
  "Explorer.tsx": 11,
  "ExtensionAction.tsx": 2,
  "Extensions.tsx": 6,
  "FindBar.tsx": 4,
  "FinishedTasks.tsx": 13,
  "FirstRun.tsx": 4,
  "FirstTaskTab.tsx": 3,
  "ForgeQuestion.tsx": 2,
  "ForgeSignIn.tsx": 2,
  "ForgeTokenSignIn.tsx": 4,
  "GetProject.tsx": 5,
  "GitIdentityForm.tsx": 2,
  "GitMissing.tsx": 3,
  "HarnessCard.tsx": 1,
  "HarnessSetupTab.tsx": 3,
  "ImportCliLogin.tsx": 2,
  "Inbox.tsx": 10,
  "InboxElsewhere.tsx": 3,
  "KillSwitch.tsx": 1,
  "LayoutPanes.tsx": 1,
  "LinkWorkItem.tsx": 2,
  "LiveDialog.tsx": 4,
  "LivesHere.tsx": 1,
  "MakeItAProject.tsx": 2,
  "MemoryArchiveTab.tsx": 5,
  "MemoryTab.tsx": 6,
  "NeedsYou.tsx": 1,
  "NewBranch.tsx": 2,
  "NewPersona.tsx": 2,
  "NewProject.tsx": 7,
  "NewVault.tsx": 2,
  "NewWorkspace.tsx": 2,
  "NotCloned.tsx": 3,
  "Notice.tsx": 6,
  "Onboarding.tsx": 3,
  "Opener.tsx": 3,
  "Palette.tsx": 1,
  "PaneCrumbs.tsx": 1,
  "PanelList.tsx": 6,
  "PanelSection.tsx": 1,
  "Panels.tsx": 1,
  "PersonaMarkPicker.tsx": 1,
  "PersonaProfile.tsx": 3,
  "PlaneUpdated.tsx": 1,
  "PublishProject.tsx": 6,
  "QuitWarning.tsx": 4,
  "RegionFrame.tsx": 2,
  "RelaunchAsk.tsx": 2,
  "RemoveFromWorkspace.tsx": 2,
  "RemovePersona.tsx": 2,
  "RenameWorkspace.tsx": 2,
  "RepoInstructionsTab.tsx": 3,
  "RepoPicker.tsx": 2,
  "SaidLink.tsx": 1,
  "SandboxBlockNotice.tsx": 3,
  "SavingView.tsx": 11,
  "SearchTab.tsx": 4,
  "SessionRecordTab.tsx": 1,
  "StartChat.tsx": 6,
  "StatusLine.tsx": 2,
  "StripMarks.tsx": 4,
  "TabChip.tsx": 3,
  "TabRename.tsx": 1,
  "TaskAway.tsx": 2,
  "TaskBlocksNotice.tsx": 1,
  "TaskChangesTab.tsx": 4,
  "TaskEnd.tsx": 6,
  "Updates.tsx": 8,
  "VaultSignIn.tsx": 7,
  "VaultTab.tsx": 21,
  "Vaults.tsx": 1,
  "Views.tsx": 5,
  "Welcome.tsx": 4,
  "WorkspaceRepos.tsx": 3,
  "WorkspaceStrip.tsx": 1,
  "YourRepos.tsx": 3,
  "cardLink.tsx": 1,
  "editor/BranchTree.tsx": 1,
  "editor/PieceDiff.tsx": 2,
  "editor/PieceFiles.tsx": 6,
  "editor/ToYourEditor.tsx": 2,
  "references.tsx": 3,
  "settings/Collection.tsx": 14,
  "settings/DispatchGrants.tsx": 5,
  "settings/GrantedList.tsx": 4,
  "settings/NetworkLists.tsx": 1,
  "settings/OutLink.tsx": 1,
  "settings/RawToml.tsx": 4,
  "settings/SettingsTab.tsx": 6,
  "settings/accounts.tsx": 2,
  "settings/components.tsx": 9,
  "settings/discover.tsx": 1,
  "settings/dispatch.tsx": 4,
  "settings/harnessDeclarations.tsx": 1,
  "settings/sandbox.tsx": 1,
  "settings/thisMachine.tsx": 1,
  "settings/workspace.tsx": 1,
};

/** What differs between the unclassed controls in the window and the list (`ratchet`). */
export function offTheList(
  found: ReadonlyMap<string, readonly string[]>,
  allowed: Readonly<Record<string, readonly string[]>>,
  ceiling: Readonly<Record<string, number>> = CEILING,
): string[] {
  return ratchet(found, allowed, ceiling, { added: "a control without its ui- class" });
}

describe("the control guard (#1837)", () => {
  it("finds every button, field, select, table and menu in the window wearing its class, or allowed", () => {
    expect(offTheList(unclassedByFile(), CONTROLS_ALLOWANCE)).toEqual([]);
  });

  it("fails an unclassed button planted in a component", () => {
    const planted = new Map([["Fresh.tsx", unclassedControls("<button onClick={go}>Go</button>")]]);
    expect(offTheList(planted, {}, {})).toEqual([
      "Fresh.tsx: a control without its ui- class: <button>",
    ]);
  });

  it("fails an entry its control no longer bears out, and a list that grew", () => {
    const allowed = { "A.tsx": ["<button>", '<table className="grid">'] };
    expect(offTheList(new Map([["A.tsx", ["<button>"]]]), allowed, { "A.tsx": 2 })).toEqual([
      'A.tsx: stale entry, delete it from the allowance: <table className="grid">',
    ]);
    expect(offTheList(new Map([["A.tsx", allowed["A.tsx"]]]), allowed, { "A.tsx": 1 })).toEqual([
      "A.tsx: the allowance grew to 2, past 1",
    ]);
  });

  it("reads each kind of control, and passes it in its class", () => {
    const tsx = `
      import * as Menu from "@radix-ui/react-dropdown-menu";
      import * as Pick from "@radix-ui/react-select";
      import * as Pop from "@radix-ui/react-popover";
      <button className="tab-close" onClick={() => a > b}>×</button>
      <button className={cn("ui-button", on && "ui-button-primary")} />
      <button className="ui-row" />
      <input value={v} />
      <input className="ui-input" />
      <input type="checkbox" className="box" />
      <input type="checkbox" className="ui-checkbox" />
      <input type="hidden" name="x" />
      <input type={kind} className="ui-input" />
      <textarea className="notes" />
      <select className="ui-select" />
      <select value={v}>
      <table className="vault-secrets">
      <table className="ui-table ui-table-fixed">
      <Menu.Content className="ui-menu more-menu" />
      <Menu.Content sideOffset={4} />
      <Menu.Item className="ui-menu-item" />
      <Menu.Item onSelect={go} />
      <Pick.Trigger className="field" />
      <Pop.Trigger asChild><span /></Pop.Trigger>
      <Pop.Trigger>Open</Pop.Trigger>
      <Pop.Content className="row-card" />
    `;
    expect(unclassedControls(tsx)).toEqual([
      '<button className="tab-close">',
      "<input>",
      '<input type="checkbox" className="box">',
      '<textarea className="notes">',
      "<select>",
      '<table className="vault-secrets">',
      "<Menu.Content>",
      "<Menu.Item>",
      '<Pick.Trigger className="field">',
      "<Pop.Trigger>",
    ]);
  });

  it("reads what a component draws and not what a comment names", () => {
    expect(unclassedControls(`/** a <button> here */ <p />`)).toEqual([]);
    expect(unclassedControls(`// a <table> here\n<p />`)).toEqual([]);
    expect(unclassedControls(`<buttons />`)).toEqual([]);
  });

  it("lists by file and in order, so a removal is one line in one place", () => {
    for (const [file, entries] of Object.entries(CONTROLS_ALLOWANCE)) {
      expect(entries.length, `${file}: an empty list is deleted, not kept`).toBeGreaterThan(0);
    }
    expect(Object.keys(CONTROLS_ALLOWANCE)).toEqual(Object.keys(CONTROLS_ALLOWANCE).sort());
    const source = readFileSync(join(SRC, "controls.guard.allowance.ts"), "utf8");
    const entryLines = source.split("\n").filter((line) => /^\s+"(?:[^"\\]|\\.)*",$/.test(line));
    const total = Object.values(CONTROLS_ALLOWANCE).flat().length;
    expect(entryLines.length, "one entry per line").toBe(total);
  });

  it("covers the window's components, so a reader that found nothing fails", () => {
    const buttons = sourceFiles(isView).flatMap((file) =>
      openingTags(readFileSync(join(SRC, file), "utf8"), /^button$/),
    );
    expect(buttons.length).toBeGreaterThan(200);
  });
});
