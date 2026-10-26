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
  declarations,
  isView,
  openingTags,
  ratchet,
  SRC,
  sourceFiles,
  type StyleSource,
  tagStyleKeys,
  windowFiles,
  withoutComments,
  RADIX_FROM,
} from "./guards.testkit";

/**
 * **Every control is its family's, or wears its shared class** (DS-4, #1837; ADR 0094 §4, ADR
 * 0104, `docs/ui-primitives.md` "Every control is a family's").
 *
 * A control drawn without its family or its class is drawn by whatever rule its view wrote for
 * it, which is how the window came to have about 30 button looks. So a component that draws one
 * of these natively gives it the class that is the whole of its look, or none is right and the
 * control is its family's wrapper (`ui/`, #3294):
 *
 * | Drawn as                                                         | Wears                     |
 * | ---------------------------------------------------------------- | ------------------------- |
 * | `<button>`, or a Radix trigger or close without `asChild`        | a list's row or a tree's row (`ui-row`, `ui-tree-row`); a button is the button family's `Button` (#2904), a segment the choice family's, a menu's row the menu family's |
 * | `<input>`, `<textarea>`                                          | nothing: a box is the field family's `TextField` or `TextArea` (#2905), a checkbox or a radio the choice family's (#2907) |
 * | `<select>`, Radix Select's `Trigger`                             | nothing: the select family's `Select` (#2906) |
 * | `<table>`                                                        | `ui-table`                |
 * | a Radix menu's or Select's `Content`, `SubContent`, `Item`, `SubTrigger`, checkbox or radio item | nothing: the menu and popover families' (#2909) |
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

/** The classes that are each kind's whole look, each drawn by `primitives/collections.css`. A
 *  native button can be a list's row or a tree's row (`docs/ui-primitives.md`). An empty list is
 *  a kind that is a family's (`ui/`), so no class makes a native one right. */
const WEARS: Record<string, string[]> = {
  button: ["ui-row", "ui-tree-row"],
  // A box a person types in is the field family's (#2905).
  input: [],
  // A checkbox and a radio are the choice families' (#2907).
  "input:checkbox": [],
  "input:radio": [],
  "input:dynamic": [],
  textarea: [],
  // A select is the select family's (#2906).
  select: [],
  table: ["ui-table"],
  // A menu and its rows are the menu and popover families' (#2909).
  menu: [],
  "menu-item": [],
};

/** Radix packages whose parts are controls, and what each part is. */
const RADIX_MENUS = new Set(["dropdown-menu", "context-menu", "menubar"]);
const MENU_CONTENT = new Set(["Content", "SubContent"]);
const MENU_ITEM = new Set(["Item", "SubTrigger", "CheckboxItem", "RadioItem"]);
const BUTTON_PARTS = new Set(["Trigger", "Close", "Cancel", "Action"]);

/** Each Radix namespace a file imports, by the name it uses: `{ Menu: "dropdown-menu" }`. */
function radixNames(text: string): Map<string, string> {
  return new Map(
    [
      ...text.matchAll(
        new RegExp(
          String.raw`import\s+\*\s+as\s+(\w+)\s+from\s+["']${RADIX_FROM}([\w-]+)["']`,
          "g",
        ),
      ),
    ].map(([, name, pkg]) => [name, pkg]),
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
      // The component layer is where a family wraps its native element (#2904).
      .filter((file) => !file.startsWith("ui/"))
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
  "Doctor.tsx": 4,
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
  "RegionFrame.tsx": 1,
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
  "StripMarks.tsx": 4,
  "TabChip.tsx": 3,
  "TabRename.tsx": 1,
  "TaskAway.tsx": 2,
  "TaskBlocksNotice.tsx": 1,
  "TaskChangesTab.tsx": 4,
  "TaskEnd.tsx": 6,
  "Updates.tsx": 7,
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
  return ratchet(found, allowed, ceiling, {
    added: "a control without its family or its ui- class",
  });
}

describe("the control guard (#1837)", () => {
  it("finds every button, field, select, table and menu in the window wearing its class, or allowed", () => {
    expect(offTheList(unclassedByFile(), CONTROLS_ALLOWANCE)).toEqual([]);
  });

  it("fails an unclassed button planted in a component", () => {
    const planted = new Map([["Fresh.tsx", unclassedControls("<button onClick={go}>Go</button>")]]);
    expect(offTheList(planted, {}, {})).toEqual([
      "Fresh.tsx: a control without its family or its ui- class: <button>",
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
      <button className={cn("go", on && "go-on")} />
      <button className="ui-row" />
      <button className={cn("ui-tree-row", open && "open")} />
      <input value={v} />
      <input className="find" />
      <input type="checkbox" className="box" />
      <input type="hidden" name="x" />
      <input type={kind} className="pick" />
      <textarea className="notes" />
      <select className="wide" />
      <table className="vault-secrets">
      <table className="ui-table ui-table-fixed">
      <Menu.Content className="more-menu" />
      <Menu.Item onSelect={go} />
      <Pick.Trigger className="field" />
      <Pop.Trigger asChild><span /></Pop.Trigger>
      <Pop.Trigger>Open</Pop.Trigger>
      <Pop.Content className="row-card" />
    `;
    expect(unclassedControls(tsx)).toEqual([
      '<button className="tab-close">',
      '<button className={cn("go", on && "go-on")}>',
      "<input>",
      '<input className="find">',
      '<input type="checkbox" className="box">',
      '<input className="pick">',
      '<textarea className="notes">',
      '<select className="wide">',
      '<table className="vault-secrets">',
      '<Menu.Content className="more-menu">',
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
    // Native buttons and the button family's, which a button left native would have been.
    const buttons = sourceFiles(isView).flatMap((file) =>
      openingTags(
        readFileSync(join(SRC, file), "utf8"),
        /^(?:button|Button|IconButton|ToggleButton)$/,
      ),
    );
    expect(buttons.length).toBeGreaterThan(200);
  });
});

/**
 * **Every control is a control tall, and every strip a strip tall** (#2449; ADR 0094 D5). The
 * geometry guard does not read a box's size (#2319), but two sizes are the scale's own: a
 * control's height is `--size-control` (28px at the default text size) and a strip's or a pane
 * header's is `--size-chrome` (21px). A stylesheet that sets a `height`, `min-height`,
 * `block-size` or `min-block-size` on one of them, and a component that sets one on a control
 * in an inline style or a Tailwind class, takes it from those two.
 *
 * **What is a control** is said by the rule's subject, the last part of its selector, where the
 * box it draws is. Each selector of a list is read on its own (a list is split at its own
 * commas, not those inside brackets), a nested rule as the full selector it stands for, and an
 * `:is()` or `:where()` as each selector in it. The subject is a control when it is:
 * - a `button`, `select` or `textarea` element, or an `input` that is not a checkbox or a radio
 *   (`input:not([type=checkbox])` is one);
 * - a class some component gives a button, a field or a select (`controlClasses`), unless that
 *   tag is a row of a list (`ui-row`, `ui-tree-row`), whose height is the list's.
 *
 * A family's own sheet (`ui/`) keys its control on attributes, not on a class, so its sizes are
 * the family's to hold; a class a view gives a family's member is read here like any other.
 *
 * A control takes `--size-control`. It takes `--size-chrome` only as the small form: a part of
 * the selector before it is a strip. A field of several lines is a whole number of controls
 * (`calc(var(--size-control) * 3)`). A checkbox, a radio and a switch are drawn at their own
 * mark's size and are not read.
 *
 * **What is a strip or a pane header** is a class named for one (`STRIP`): a bar, a strip, a
 * head or a row of tabs, the three strips by name, the pane's top line, the status line, and
 * the two rows of controls a strip or a pane's top line draws as part of itself
 * (`STRIP_PARTS`); a chart's bar and the two columns down a side are not (`NOT_A_STRIP`). It
 * takes `--size-chrome`.
 *
 * Either may also be as tall as its content or its container (`auto`, `100%`, `0`, `none`), or
 * read a custom property that holds the token it may take: a property brings its token's rule,
 * so one holding `--size-chrome` is no way to a full control at the chrome height. The small
 * form's "inside a strip" is a descendant or a child of one, not a sibling. **A named exception** gives its
 * reason (`HEIGHT_EXCEPTIONS`); the list is exact, as the allowance is, so an exception whose
 * rule is gone fails until it is deleted.
 *
 * **What it reads is what a rule declares.** A control with no height of its own (one a
 * padding makes tall) is not checked, and a custom property is held by its value wherever it
 * is declared, not by the scope it is declared in.
 */

/** A tag wearing one of these is a row of a list, not a control on its own. */
const NOT_ON_ITS_OWN = ["ui-row", "ui-tree-row"];

/** The kinds of control whose classes are held to the control height. */
const HELD_KINDS = new Set(["button", "input", "input:dynamic", "textarea", "select", "menu-item"]);

/** The button family's members (`ui/button/Button.tsx`, #2904): a class one is given places a
 *  control, so a view's height on it is a control's. */
const BUTTON_FAMILY = /^(?:Button|IconButton|ToggleButton)$/;

/** `tag` without its `icon={…}` prop. */
function withoutIcon(tag: string): string {
  const at = /\sicon=\{/.exec(tag);
  if (!at) return tag;
  let depth = 0;
  for (let i = at.index + at[0].length - 1; i < tag.length; i++) {
    if (tag[i] === "{") depth++;
    else if (tag[i] === "}" && --depth === 0) return tag.slice(0, at.index) + tag.slice(i + 1);
  }
  return tag;
}

/** Each class a component gives a control it draws (`HELD_KINDS`), rows aside. */
export function controlClasses(source: string): string[] {
  const radix = radixNames(withoutComments(source, "ts"));
  return openingTags(
    source,
    /^(?:button|input|textarea|select|Button|IconButton|ToggleButton|\w+\.\w+)$/,
  ).flatMap((tag) => {
    const { name } = tag;
    // A family member's `icon` is an element of its own, whose class is the icon's.
    const text = BUTTON_FAMILY.test(name) ? withoutIcon(tag.text) : tag.text;
    const kind = BUTTON_FAMILY.test(name) ? "button" : kindOf(name, text, radix);
    if (!kind || !HELD_KINDS.has(kind)) return [];
    const className = classNameOf(text) ?? "";
    const strings = /\sclassName=\{/.test(text)
      ? [...className.matchAll(/["'`]([^"'`]*)["'`]/g)].map(([, words]) => words)
      : [className];
    const words = strings.flatMap((words) => words.split(/\s+/)).filter(Boolean);
    return words.some((word) => NOT_ON_ITS_OWN.includes(word)) ? [] : words;
  });
}

/** A strip or a pane header, by its class's name: a bar, a strip, a head or a row of tabs, the
 *  three strips by name (ADR 0036), the pane's top line and the status line. */
const STRIP =
  /^(?:bar|tabs|projects|workspaces|status-line|pane-chips|strip(?:-[\w-]+)?|[\w-]+-(?:bar|strip|head|tabs))$/;

/** Named like a strip and not one: a chart's bar and the two columns down a side. */
const NOT_A_STRIP = new Set(["chart-bar", "state-bar", "activity-bar"]);

/** The chips on a pane's top line (#2484): the breadcrumb, Brief, the gauge, the harness, where
 *  a handed-off chat came from, its work item and its skills (#2775). Each is as tall as the line, the chrome
 *  height, so the line is one height with the ways to end a task beside them. */
export const PANE_LINE_CHIPS = [
  "pane-crumbs",
  "pane-brief",
  "chat-gauge",
  "pane-harness",
  "pane-from",
  "pane-work-item",
  "pane-skills",
];

/** What is drawn as part of a strip or a pane's top line, as tall as it is: the ways to end a
 *  pane's task (#1488), the chip a tab wears for its tasks (#1487) and the top line's chips. */
const STRIP_PARTS = new Set(["pane-task-ends", "tab-tasks", ...PANE_LINE_CHIPS]);

const HEIGHT_PROPERTY = /^(?:min-)?(?:height|block-size)$/;

/** A height that is no length of its own: its content's, its container's, or none. */
const AS_TALL_AS = /^(?:auto|100%|0|none|fit-content|min-content|max-content)$/;

/** `list` split at its commas outside brackets and strings. */
export function splitList(list: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < list.length; i++) {
    const char = list[i];
    if (char === '"' || char === "'") {
      const close = list.indexOf(char, i + 1);
      i = close < 0 ? list.length : close;
    } else if (char === "(" || char === "[") depth++;
    else if (char === ")" || char === "]") depth--;
    else if (char === "," && depth === 0) {
      parts.push(list.slice(start, i).trim());
      start = i + 1;
    }
  }
  parts.push(list.slice(start).trim());
  return parts.filter(Boolean);
}

/** Where the subject starts inside a selector: after the last combinator outside brackets.
 *  `.tabs .tab:has(> .tab-tasks)` is `.tab:has(> .tab-tasks)`. */
function subjectStart(selector: string): number {
  let depth = 0;
  let start = 0;
  for (let i = 0; i < selector.length; i++) {
    const char = selector[i];
    if (char === "(" || char === "[") depth++;
    else if (char === ")" || char === "]") depth--;
    else if (depth === 0 && /[\s>+~]/.test(char)) start = i + 1;
  }
  return start;
}

/** The full selectors a nested rule stands for: each level put in its parent's place at `&`,
 *  or after it as a descendant. At-rules are no level. */
export function nestedSelectors(context: readonly string[]): string[] {
  let selectors = [""];
  for (const level of context.filter((part) => !part.startsWith("@"))) {
    selectors = selectors.flatMap((parent) =>
      splitList(level).map((own) =>
        parent === ""
          ? own.replace(/&/g, ":root")
          : own.includes("&")
            ? own.replace(/&/g, `:is(${parent})`)
            : `${parent} ${own}`,
      ),
    );
  }
  return selectors;
}

/** One selector as plain ones: each `:is()` or `:where()` in it opened, a selector per item of
 *  its list, until none is left. */
export function opened(selector: string): string[] {
  const at = /:(?:is|where)\(/.exec(selector);
  if (!at) return [selector.trim()];
  let depth = 1;
  let end = at.index + at[0].length;
  for (; end < selector.length && depth > 0; end++) {
    if (selector[end] === "(") depth++;
    else if (selector[end] === ")") depth--;
  }
  const inner = selector.slice(at.index + at[0].length, end - 1);
  const before = selector.slice(0, at.index);
  const after = selector.slice(end);
  return splitList(inner).flatMap((item) => opened(`${before}${item}${after}`));
}

/** The classes a compound names, and its element, with what `:not()`, `:has()` and the other
 *  functional pseudo-classes say about other elements taken out. */
function partsOf(compound: string): { element?: string; classes: string[]; toggle: boolean } {
  let own = compound;
  for (let was = ""; was !== own;) {
    was = own;
    own = own.replace(/:[\w-]+\([^()]*\)/g, "");
  }
  const toggle = /\[type=["']?(?:checkbox|radio)["']?\]/.test(own);
  own = own.replace(/\[[^\]]*\]/g, "").replace(/:[\w-]+/g, "");
  return {
    element: /^[a-z]+/.exec(own)?.[0],
    classes: [...own.matchAll(/\.([\w-]+)/g)].map(([, name]) => name),
    toggle,
  };
}

const isStrip = (name: string) =>
  (STRIP.test(name) && !NOT_A_STRIP.has(name)) || STRIP_PARTS.has(name);

/** The compounds a selector's subject sits inside: each one a descendant (` `) or child (`>`)
 *  combinator leads from. A sibling (`+`, `~`) is beside the subject, not around it, though
 *  what is around the sibling is around the subject too. */
function ancestorsOf(selector: string): string[] {
  const compounds: string[] = [];
  const combinators: string[] = [];
  let depth = 0;
  let current = "";
  let pending = "";
  for (const char of selector.trim()) {
    if (char === "(" || char === "[") depth++;
    else if (char === ")" || char === "]") depth--;
    if (depth === 0 && /[\s>+~]/.test(char)) {
      if (current) {
        compounds.push(current);
        current = "";
        pending = " ";
      }
      if (char !== " " && !/\s/.test(char)) pending = char;
      continue;
    }
    if (!current && compounds.length > 0) combinators.push(pending);
    current += char;
  }
  if (current) compounds.push(current);
  // combinators[i] sits between compounds[i] and compounds[i + 1].
  return compounds.slice(0, -1).filter((_, i) => !["+", "~"].includes(combinators[i]));
}

/** What a plain selector's subject is, for this guard: a control (small, when it sits inside a
 *  strip), a strip, or neither. A strip's part (`STRIP_PARTS`) is a strip,
 *  whatever else it is. Otherwise a subject whose own class is a control's is a control, however
 *  its other classes are named. */
export function subjectKind(
  selector: string,
  controls: ReadonlySet<string>,
): "a control" | "a small control" | "a strip" | undefined {
  const subject = selector.trim().slice(subjectStart(selector.trim()));
  if (subject.includes("::")) return undefined;
  const { element, classes, toggle } = partsOf(subject);
  // A part of a strip by name is the strip's height, even a button (a top-line chip); only a
  // class that is a strip by its pattern (`-head`, `-bar`) gives way to a control class.
  if (classes.some((name) => STRIP_PARTS.has(name))) return "a strip";
  const controlClass = classes.some((name) => controls.has(name));
  if (!controlClass && classes.some(isStrip)) return "a strip";
  const control =
    controlClass ||
    (element === "input" && !toggle) ||
    (element !== undefined && ["button", "select", "textarea"].includes(element));
  if (!control) return undefined;
  const around = ancestorsOf(selector).flatMap((compound) => partsOf(compound).classes);
  return around.some(isStrip) ? "a small control" : "a control";
}

type Height = "--size-control" | "--size-chrome";

/** The custom properties that hold one of the two heights, directly or through another one, and
 *  which one each holds. Where a property is declared is not followed: one that holds a token
 *  anywhere holds it. */
function heightCustoms(sheets: readonly string[]): Map<string, Height> {
  const all = sheets
    .flatMap((css) => declarations(css))
    .filter(({ property }) => property.startsWith("--"));
  const held = new Map<string, Height>([
    ["--size-control", "--size-control"],
    ["--size-chrome", "--size-chrome"],
  ]);
  for (let grew = true; grew;) {
    grew = false;
    for (const { property, value } of all) {
      const read = /^var\(\s*(--[\w-]+)\s*\)$/.exec(value)?.[1];
      const token = read === undefined ? undefined : held.get(read);
      if (token && !held.has(property)) {
        held.set(property, token);
        grew = true;
      }
    }
  }
  return held;
}

type Kind = "a control" | "a small control" | "a strip";

/** Whether a height is on the tokens for what it is set on, read straight or through a custom
 *  property that holds one (`heightCustoms`): the chrome height for a strip or a small control,
 *  the control height for any control. */
function onTheTokens(kind: Kind, value: string, held: ReadonlyMap<string, Height>) {
  if (AS_TALL_AS.test(value)) return true;
  const read = /^var\(\s*(--[\w-]+)\s*\)$/.exec(value)?.[1];
  const token = read === undefined ? undefined : held.get(read);
  if (token === "--size-chrome") return kind !== "a control";
  if (token === "--size-control") return kind !== "a strip";
  if (kind === "a strip") return false;
  return /^calc\(\s*var\(--size-control\)\s*\*\s*\d+\s*\)$/.test(value);
}

/** Each height off the tokens in a stylesheet, as `<rule> | <property>: <value> (<kind>)`: a
 *  rule fails when any one selector it stands for does, named by the first that does. */
export function cssHeights(
  css: string,
  controls: ReadonlySet<string>,
  held: ReadonlyMap<string, Height> = heightCustoms([css]),
): string[] {
  return declarations(css).flatMap(({ context, property, value }) => {
    if (!HEIGHT_PROPERTY.test(property)) return [];
    const kinds = nestedSelectors(context)
      .flatMap(opened)
      .map((selector) => subjectKind(selector, controls));
    const off = kinds.find((kind) => kind !== undefined && !onTheTokens(kind, value, held));
    if (!off) return [];
    return [`${context.join(" › ")} | ${property}: ${value} (${off})`];
  });
}

/** A Tailwind utility that sets a height (`h-7`, `min-h-8`, `size-6`), not to `auto` or `full`. */
const HEIGHT_UTILITY = /^(?:min-)?(?:h|size)-(?!auto$|full$)\S+$/;
const HEIGHT_KEY = /^(?:height|minHeight|blockSize|minBlockSize)$/;

/** Each height a component sets on a control it draws, in a Tailwind class or in its `style`,
 *  followed as the geometry guard follows one (`tagStyleKeys`: written inline, held in a
 *  variable, a member of a local object, a branch of a ternary, a local function's return, an
 *  import). The tokens are the primitives' classes; a component writes none of its own. */
export function componentHeights(source: string, from: StyleSource = {}): string[] {
  const radix = radixNames(withoutComments(source, "ts"));
  return openingTags(source, /^(?:button|input|textarea|select|\w+\.\w+)$/).flatMap((tag) => {
    const { name, text } = tag;
    const kind = kindOf(name, text, radix);
    if (!kind || !HELD_KINDS.has(kind)) return [];
    const found: string[] = [];
    for (const word of classWords(classNameOf(text))) {
      if (HEIGHT_UTILITY.test(word.split(":").pop() ?? word)) found.push(`<${name}> class ${word}`);
    }
    for (const { label, key } of tagStyleKeys(source, tag, from)) {
      if (HEIGHT_KEY.test(key)) found.push(`<${name}> ${label}`);
    }
    return found;
  });
}

/**
 * **The heights that are not the tokens, each with its reason.** Exact, as the allowance is: an
 * entry whose rule is gone fails until it is deleted, and a new one says why here.
 */
const HEIGHT_EXCEPTIONS: Record<string, Record<string, string>> = {
  "App.css": {
    ".title-bar | min-height: max(40px, 2.1rem) (a strip)":
      "the system's title bar, 40px as the prototype draws it (R90): on macOS the traffic lights are placed at its own title bar's height whatever the page draws, and a shorter bar has them over the regions (ADR 0054)",
    ".tasks-menu-place | height: var(--size-chrome) (a control)":
      "an icon square at the end of a menu's row, the small control height, so the row stays a control tall",
  },
};

/** Every height off the tokens in the window today, by file: its stylesheets (the core aside)
 *  and its components. */
function heightsByFile(planted: Readonly<Record<string, string>> = {}): Map<string, string[]> {
  const text = (file: string) => planted[file] ?? readFileSync(join(SRC, file), "utf8");
  const others = windowFiles(planted);
  const views = sourceFiles(isView);
  const controls = new Set(views.flatMap((file) => controlClasses(text(file))));
  const sheets = sourceFiles((name) => name.endsWith(".css"))
    .filter((file) => file !== "theme/geometry.css")
    .map((file) => [file, readFileSync(join(SRC, file), "utf8")] as const);
  const held = heightCustoms(sheets.map(([, text]) => text));
  const byFile = new Map<string, string[]>();
  for (const [file, text] of sheets) byFile.set(file, cssHeights(text, controls, held));
  for (const file of views) byFile.set(file, componentHeights(text(file), { file, read: others }));
  return new Map([...byFile].filter(([, found]) => found.length > 0));
}

/** The top line's chips (`PANE_LINE_CHIPS`) that no rule in `css` sets `height:
 *  var(--size-chrome)` on, as a chip of the line: a strip's part, or a control inside a strip. */
export function chipsNotChromeTall(css: string, controls: ReadonlySet<string>): string[] {
  const tall = new Set<string>();
  for (const { context, property, value } of declarations(css)) {
    if (property !== "height" || value !== "var(--size-chrome)") continue;
    for (const selector of nestedSelectors(context).flatMap(opened)) {
      const kind = subjectKind(selector, controls);
      if (kind !== "a strip" && kind !== "a small control") continue;
      const subject = selector.trim().slice(subjectStart(selector.trim()));
      for (const name of partsOf(subject).classes) tall.add(name);
    }
  }
  return PANE_LINE_CHIPS.filter((chip) => !tall.has(chip));
}

/** The components the pane's top line draws a chip with, and the chip each draws. */
const LINE_CHIP_COMPONENTS: Record<string, string> = {
  PaneCrumbs: "pane-crumbs",
  BriefButton: "pane-brief",
  ChatGauge: "chat-gauge",
  HarnessChip: "pane-harness",
  SkillsChip: "pane-skills",
};

/** A component that draws its one child as itself (`asChild`): the line reads through it. */
const LINE_WRAPPERS = new Set(["Menued"]);

/** What each `<div className="pane-chips">` in a component's source holds: each element drawn
 *  straight in it (through `LINE_WRAPPERS` and fragments), as a chip of `PANE_LINE_CHIPS` or,
 *  for one that is not, as `<Name className>`. A tag written inside another's props
 *  (`state={<StateShown />}`) is not a child. */
export function lineChildren(source: string): { chips: string[]; unlisted: string[] } {
  const text = withoutComments(source, "ts");
  const opens = openingTags(text, /./);
  const closes = [...text.matchAll(/<\/([A-Za-z][\w.]*)\s*>/g)].map((hit) => ({
    index: hit.index,
    name: hit[1],
  }));
  const chips: string[] = [];
  const unlisted: string[] = [];
  for (const line of opens.filter((tag) =>
    classWords(classNameOf(tag.text)).includes("pane-chips"),
  )) {
    type Event = { index: number; close?: string; tag?: (typeof opens)[number] };
    const after = line.index + line.text.length;
    let reach = after;
    const events: Event[] = [];
    for (const tag of opens) {
      if (tag.index < after || tag.index < reach) continue;
      events.push({ index: tag.index, tag });
      reach = tag.index + tag.text.length;
    }
    const inProps = (index: number) =>
      events.some(({ tag }) => tag && index > tag.index && index < tag.index + tag.text.length);
    for (const close of closes)
      if (close.index >= after && !inProps(close.index))
        events.push({ index: close.index, close: close.name });
    events.sort((a, b) => a.index - b.index);
    let depth = 0;
    const wrapped: number[] = [];
    for (const event of events) {
      if (event.close !== undefined) {
        if (LINE_WRAPPERS.has(event.close) && wrapped.at(-1) === depth) {
          wrapped.pop();
          continue;
        }
        if (--depth < 0) break;
        continue;
      }
      const tag = event.tag;
      if (tag === undefined) continue;
      const closed = /\/\s*>$/.test(tag.text);
      if (depth === 0 && LINE_WRAPPERS.has(tag.name)) {
        if (!closed) wrapped.push(depth);
        continue;
      }
      if (depth === 0) {
        const own = classWords(classNameOf(tag.text));
        const chip =
          LINE_CHIP_COMPONENTS[tag.name] ?? own.find((word) => PANE_LINE_CHIPS.includes(word));
        if (chip !== undefined && PANE_LINE_CHIPS.includes(chip)) chips.push(chip);
        else unlisted.push(`<${tag.name} ${own.join(" ")}>`.replace(" >", ">"));
      }
      if (!closed) depth++;
    }
  }
  return { chips, unlisted };
}

/** Whether some component draws an element whose `className` hands the DOM `chip`, written as
 *  a string, in `cn(…)` or beside other classes. */
function drawn(chip: string, sources: readonly string[]): boolean {
  return sources.some((source) =>
    openingTags(source, /./).some((tag) => classWords(classNameOf(tag.text)).includes(chip)),
  );
}

/** What differs between the heights found and the named exceptions (`ratchet`). */
export function offTheHeights(
  found: ReadonlyMap<string, readonly string[]>,
  exceptions: Readonly<Record<string, Readonly<Record<string, string>>>> = HEIGHT_EXCEPTIONS,
): string[] {
  const allowed = Object.fromEntries(
    Object.entries(exceptions).map(([file, entries]) => [file, Object.keys(entries)]),
  );
  const ceiling = Object.fromEntries(
    Object.entries(allowed).map(([file, entries]) => [file, entries.length]),
  );
  return ratchet(found, allowed, ceiling, {
    added: "take the height from --size-control or --size-chrome, or name the exception",
  });
}

describe("the control and chrome heights (#2449)", () => {
  it("finds every control a control tall and every strip a strip tall, or a named exception", () => {
    expect(offTheHeights(heightsByFile())).toEqual([]);
  });

  it("follows a control's style wherever the file hands it on, in the real tree (#2566)", () => {
    const tsx = `${readFileSync(join(SRC, "LayoutPanes.tsx"), "utf8")}
import { plantedTall } from "./theme/theme";
const plantedSizes = { row: { minHeight: 30 } };
const plantedHeld = { height: "1.9rem" };
const plantedBox = () => ({ blockSize: 31 });
export const PlantedMember = () => <button className="planted" style={plantedSizes.row} />;
export const PlantedEither = ({ on }: { on: boolean }) => (
  <button className="planted" style={on ? plantedHeld : undefined} />
);
export const PlantedCall = () => <button className="planted" style={plantedBox()} />;
export const PlantedImported = () => <button className="planted" style={plantedTall} />;
`;
    const script = `${readFileSync(join(SRC, "theme/theme.ts"), "utf8")}
export const plantedTall = { minBlockSize: 32 };
`;
    const found = heightsByFile({ "LayoutPanes.tsx": tsx, "theme/theme.ts": script });
    expect(offTheHeights(found)).toEqual(
      [
        "<button> style plantedSizes.row.minHeight",
        "<button> style plantedHeld.height",
        "<button> style plantedBox().blockSize",
        "<button> style plantedTall.minBlockSize",
      ].map(
        (entry) =>
          `LayoutPanes.tsx: take the height from --size-control or --size-chrome, or name the exception: ${entry}`,
      ),
    );
  });

  it("fails a control's hand-written height planted in the window's own stylesheet", () => {
    const found = heightsByFile();
    const css = readFileSync(join(SRC, "App.css"), "utf8");
    const planted = `${css}\n.planted button { height: 1.9rem; }\n.planted-head { min-height: 1.4rem; }\n`;
    const controls = new Set(
      sourceFiles(isView).flatMap((file) => controlClasses(readFileSync(join(SRC, file), "utf8"))),
    );
    found.set("App.css", cssHeights(planted, controls));
    expect(offTheHeights(found)).toEqual([
      "App.css: take the height from --size-control or --size-chrome, or name the exception: .planted button | height: 1.9rem (a control)",
      "App.css: take the height from --size-control or --size-chrome, or name the exception: .planted-head | min-height: 1.4rem (a strip)",
    ]);
  });

  it("fails an exception whose rule is gone", () => {
    expect(
      offTheHeights(new Map(), { "A.css": { ".gone | height: 3px (a control)": "why" } }),
    ).toEqual([
      "A.css: stale entry, delete it from the allowance: .gone | height: 3px (a control)",
    ]);
  });

  it("knows a control and a strip by the rule's subject", () => {
    const controls = new Set(["task-end", "closer", "pane-harness"]);
    const kinds = [
      "button",
      ".bar button:hover:not(:disabled)",
      "input[type=text]",
      "input[type='checkbox']",
      "input:not([type=checkbox])",
      ".actions > .task-end",
      ".find-bar > .closer",
      ".pane-task-ends .task-end",
      ".tab .closer::before",
      ".tabs .tab:has(> .tab-tasks)",
      ".tabs .tab > .tab-tasks",
      ".half-head",
      ".status-line",
      ".panels .panel-head",
      ".bar",
      ".pane-chips",
      ".strip-doing",
      ".chart-bars .chart-bar",
      ".state-bar",
      ".activity-bar",
      ".activity-tab svg",
      ".box",
      ".bar + button",
      ".bar ~ .x + button",
      ".bar .x + button",
      ".bar > .x ~ button",
      ".closer.toolbar-head",
      ".task-end.panel-head",
      ".tab-tasks.task-end",
      ".pane-harness",
    ].map((selector) => subjectKind(selector, controls));
    expect(kinds).toEqual([
      "a control",
      "a small control",
      "a control",
      undefined,
      "a control",
      "a control",
      "a small control",
      "a small control",
      undefined,
      undefined,
      "a strip",
      "a strip",
      "a strip",
      "a strip",
      "a strip",
      "a strip",
      "a strip",
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      "a control",
      "a control",
      "a small control",
      "a small control",
      "a control",
      "a control",
      "a strip",
      "a strip",
    ]);
  });

  it("splits a list at its own commas, opens :is() and :where(), and follows nesting", () => {
    expect(splitList(".a:is(.b, .c) button, .d")).toEqual([".a:is(.b, .c) button", ".d"]);
    expect(splitList(`[title="a, b"] button, .d`)).toEqual([`[title="a, b"] button`, ".d"]);
    expect(opened(":where(.x, .y) :is(button, .z)")).toEqual([
      ".x button",
      ".x .z",
      ".y button",
      ".y .z",
    ]);
    expect(nestedSelectors(["@media (width > 1px)", ".bar, .rail", "& button", "&:hover"])).toEqual(
      [":is(:is(.bar) button):hover", ":is(:is(.rail) button):hover"],
    );
    expect(nestedSelectors([".tabs", ".tab-tasks"])).toEqual([".tabs .tab-tasks"]);
    const css = `
      .rail { :is(button, .x) { height: 1.9rem; } }
      .bar { & button { height: var(--size-chrome); } }
      .row { & button { height: var(--size-chrome); } }
      .a, .b:is(.c, .d) { height: 3px; }
      :where(.half-head) { min-height: 1.4rem; }
    `;
    expect(cssHeights(css, new Set())).toEqual([
      ".rail › :is(button, .x) | height: 1.9rem (a control)",
      ".row › & button | height: var(--size-chrome) (a control)",
      ":where(.half-head) | min-height: 1.4rem (a strip)",
    ]);
  });

  it("passes the tokens, a token held in a custom property, a field of lines, and no length", () => {
    const css = `
      :root { --row: var(--size-chrome); --lead: var(--row); }
      .task-end { block-size: var(--size-control); }
      .find-bar .task-end { block-size: var(--size-chrome); }
      .bar button { block-size: var(--size-chrome); }
      textarea { min-block-size: calc(var(--size-control) * 3); }
      .closer { min-block-size: var(--block); }
      :root { --block: var(--size-control); }
      .bar button { min-block-size: var(--lead); }
      .bar { min-height: var(--size-chrome); }
      .actions > .task-end { block-size: auto; }
      .pane-head { height: 100%; }
      .x button { max-height: 3rem; width: 7px; }
      .not-a-control { height: 1.3rem; }
    `;
    expect(cssHeights(css, new Set(["task-end", "closer"]))).toEqual([]);
    expect(cssHeights(".bar { min-height: var(--size-control); }", new Set())).toEqual([
      ".bar | min-height: var(--size-control) (a strip)",
    ]);
    expect(cssHeights("button { height: calc(var(--size-control) * 1.5); }", new Set())).toEqual([
      "button | height: calc(var(--size-control) * 1.5) (a control)",
    ]);
    // A custom property brings its token's rule: a control is at least a control tall.
    expect(
      cssHeights(
        ":root { --lead: var(--size-chrome); } .closer { min-block-size: var(--lead); }",
        new Set(["closer"]),
      ),
    ).toEqual([".closer | min-block-size: var(--lead) (a control)"]);
    expect(cssHeights(".panel button { height: var(--size-chrome); }", new Set())).toEqual([
      ".panel button | height: var(--size-chrome) (a control)",
    ]);
    expect(
      cssHeights(".bar { height: inherit; } button { min-height: unset; }", new Set()),
    ).toEqual([".bar | height: inherit (a strip)", "button | min-height: unset (a control)"]);
  });

  it("reads a control's height in an inline style or a Tailwind class", () => {
    const tsx = `
      <button className="closer h-7" />
      <button className="closer h-full" style={{ minHeight: 30, width: 9 }} />
      <input className="find md:min-h-8" />
      <div className="h-7" style={{ height: 4 }} />
    `;
    expect(componentHeights(tsx)).toEqual([
      "<button> class h-7",
      "<button> style minHeight",
      "<input> class md:min-h-8",
    ]);
  });

  it("gives every class a control wears, and not a row's or a toggle's", () => {
    const tsx = `
      <button className="closer task-end" />
      <button className="chat ui-row" />
      <button className={cn("file", open && "ui-tree-row")} />
      <input type="checkbox" className="box" />
      <select className={cn("pick", wide && "wide")} />
    `;
    expect(controlClasses(tsx)).toEqual(["closer", "task-end", "pick", "wide"]);
  });

  it("holds every chip on a pane's top line at the chrome height (#2484)", () => {
    const css = readFileSync(join(SRC, "App.css"), "utf8");
    const controls = new Set(
      sourceFiles(isView).flatMap((file) => controlClasses(readFileSync(join(SRC, file), "utf8"))),
    );
    expect(chipsNotChromeTall(css, controls)).toEqual([]);
    // Each chip is one a component draws, so the list does not outlive the line.
    const views = sourceFiles(isView).map((file) => readFileSync(join(SRC, file), "utf8"));
    for (const chip of PANE_LINE_CHIPS) expect(drawn(chip, views), chip).toBe(true);
    // And each component named for a chip draws it.
    for (const [name, chip] of Object.entries(LINE_CHIP_COMPONENTS)) {
      const own = views.find((text) => new RegExp(`function ${name}\\b`).test(text));
      expect(own !== undefined && drawn(chip, [own]), `${name} draws .${chip}`).toBe(true);
    }
  });

  it("reads every child of the pane's top line, and fails one that is not a listed chip", () => {
    const source = readFileSync(join(SRC, "LayoutPanes.tsx"), "utf8");
    const { chips, unlisted } = lineChildren(source);
    expect(unlisted).toEqual([]);
    expect(new Set(chips)).toEqual(new Set(PANE_LINE_CHIPS));
    const planted = `
      <div className="pane-chips">
        {a && (<Menued on={x}><PaneCrumbs crumbs={c} state={<StateShown shown={s} />} /></Menued>)}
        <BriefButton of={b} />
        {from && <span className={cn("pane-from", quiet && "quiet")}>{from}</span>}
        <span className="pane-seventh">new</span>
        <NewChip />
      </div>
      <div className="other"><Ignored /></div>
    `;
    expect(lineChildren(planted)).toEqual({
      chips: ["pane-crumbs", "pane-brief", "pane-from"],
      unlisted: ["<span pane-seventh>", "<NewChip>"],
    });
  });

  it("fails a top-line chip with no chrome height, and one held off it", () => {
    const controls = new Set(["pane-harness", "pane-brief"]);
    expect(
      chipsNotChromeTall(".pane-chips > .chat-gauge { height: var(--size-chrome); }", controls),
    ).toEqual(PANE_LINE_CHIPS.filter((chip) => chip !== "chat-gauge"));
    // A chip off the chrome height fails, a button chip as any other.
    expect(cssHeights(".pane-from { min-height: 1.25rem; }", controls)).toEqual([
      ".pane-from | min-height: 1.25rem (a strip)",
    ]);
    expect(
      cssHeights(
        ".pane-chips > .pane-harness { height: var(--size-control); } .pane-brief { min-height: var(--size-control); }",
        controls,
      ),
    ).toEqual([
      ".pane-chips > .pane-harness | height: var(--size-control) (a strip)",
      ".pane-brief | min-height: var(--size-control) (a strip)",
    ]);
  });

  it("covers the window, so a reader that found nothing fails", () => {
    const controls = sourceFiles(isView).flatMap((file) =>
      controlClasses(readFileSync(join(SRC, file), "utf8")),
    );
    // Fewer since the field (#2905), the status items and pills (#2913) and the menus (#2909)
    // became families of their own.
    expect(new Set(controls).size).toBeGreaterThan(70);
    const read = declarations(readFileSync(join(SRC, "App.css"), "utf8")).filter(({ property }) =>
      HEIGHT_PROPERTY.test(property),
    );
    expect(read.length).toBeGreaterThan(90);
  });
});
