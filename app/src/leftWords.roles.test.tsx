// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { declarations } from "./guards.testkit";
import { ROLES, typeOf, wears, type Role } from "./textRoles.testkit";
import type { ListedChat } from "./chatsTree";
import { PaneCrumbs } from "./PaneCrumbs";
import { PaneFoot } from "./ui/pane-foot/PaneFoot";
import { editorToolbar, ToolbarFile } from "./ui/tabs/Tabs";
import { ProjectCrumb, TitleBarProject } from "./ui/titlebar/TitleBarWorkspaces";
import { SwitcherRow } from "./ui/titlebar/Switcher";

/**
 * **The words #1920's surfaces left on their own tokens wear the text roles** (#6389;
 * `docs/design-system.md`, "The words left over, on the roles"). Each was drawn from tokens and
 * ranked by its own rule; now each wears its role, and keeps only the tier its family's state
 * gives it (the title bar's quiet chips, a foot's muted words), which outranks the role's
 * colour, as a theme's parts do from the layer above.
 */

afterEach(cleanup);

/** `word` is drawn at `role`'s size and weight (strong, or not), in `tier`. */
function tieredAs(word: Element | null, role: Role, tier: string, what: string, strong = false) {
  expect(word, `${what} was not drawn`).toBeTruthy();
  expect(typeOf(word as Element), what).toEqual({
    size: ROLES[role].size,
    weight: strong ? "600" : ROLES[role].weight,
    colour: tier,
  });
}

describe("the title bar's own words", () => {
  it("names the project as the body's strong line, in the quiet tier beside the rail", () => {
    render(<TitleBarProject name="purlis" />);
    const project = screen.getByTestId("title-project");
    expect(project).toHaveClass("ui-text-body", "ui-text-strong");
    tieredAs(project, "body", "--text-quiet", "the project's name", true);
  });

  it("names the project in its crumb as the body's strong line, in the secondary tier", () => {
    render(<ProjectCrumb name="purlis" mark={null} open={false} onPress={() => undefined} />);
    const crumb = screen.getByRole("button", { name: /purlis/ });
    expect(crumb).toHaveClass("ui-text-body", "ui-text-strong");
    tieredAs(crumb, "body", "--text-secondary", "the crumb", true);
  });

  it("draws a switcher's row as the body and its count as the strong meta, in the waiting colour", () => {
    render(<SwitcherRow id="r" name="beta" waiting={3} active={false} onPress={() => undefined} />);
    const row = screen.getByRole("option");
    expect(row).toHaveClass("ui-text-body");
    tieredAs(row, "body", "--text-secondary", "a row at rest");
    const count = row.querySelector('[data-part="row-count"]');
    expect(count).toHaveClass("ui-text-meta", "ui-text-strong");
    tieredAs(count, "meta", "--state-waiting", "a row's count", true);
  });
});

describe("the editor's toolbar over a file", () => {
  it("wears the secondary on the toolbar itself, beside its caller's class, and keeps the name's and the path's tiers", () => {
    const { container } = render(
      <header {...editorToolbar("piece-files-head")}>
        <ToolbarFile path="app/src/main.ts" />
      </header>,
    );
    const toolbar = container.querySelector("header") as Element;
    expect(toolbar).toHaveClass("ui-text-secondary", "piece-files-head");
    tieredAs(toolbar.querySelector("b"), "secondary", "--text-primary", "the file's name", true);
    tieredAs(toolbar.querySelector("code"), "secondary", "--text-quiet", "its path");
  });
});

function listed(session: number, name: string, more: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    name,
    persona: "steward",
    workspace: "alpha",
    shell: false,
    parent: null,
    mode: "task",
    from: null,
    tab: false,
    branch: null,
    report: "sent",
    outcome: "done",
    asking: null,
    harness: "Claude Code",
    ...more,
  } as ListedChat;
}

describe("a pane's header and foot", () => {
  it("draws the breadcrumb as the meta, which its names and its where take from it", () => {
    render(
      <PaneCrumbs
        crumbs={{
          path: [
            listed(4, "steward 4", { mode: null, report: null, outcome: null, tab: true }),
            listed(9, "talk", { parent: 4 }),
          ],
          elsewhere: "beta",
        }}
        onShow={() => undefined}
        gone={() => true}
      />,
    );
    const crumbs = screen.getByRole("navigation", { name: "Chat path" });
    wears(crumbs, "meta", "the breadcrumb");
    tieredAs(crumbs.querySelector(".crumb.own"), "meta", "--text-quiet", "a name on the way");
    tieredAs(crumbs.querySelector(".crumb-where"), "meta", "--text-quiet", "where the task is");
    // The chat on screen is the one name the line draws primary.
    tieredAs(crumbs.querySelector(".crumb.shown"), "meta", "--text-primary", "the chat shown");
  });

  it("tints the breadcrumb's names primary on the pane that has the keyboard, and keeps where it is quiet", () => {
    render(
      <div data-scope="panel" data-variant="content">
        <div data-group="">
          <div className="pane-frame" data-focused="">
            <div className="pane-head">
              <PaneCrumbs
                crumbs={{
                  path: [
                    listed(4, "steward 4", { mode: null, report: null, outcome: null, tab: true }),
                    listed(9, "talk", { parent: 4 }),
                  ],
                  elsewhere: "beta",
                }}
                onShow={() => undefined}
                gone={() => true}
              />
            </div>
          </div>
        </div>
      </div>,
    );
    const crumbs = screen.getByRole("navigation", { name: "Chat path" });
    tieredAs(crumbs.querySelector(".crumb.own"), "meta", "--text-primary", "a name, focused");
    tieredAs(crumbs.querySelector(".crumb-where"), "meta", "--text-quiet", "where, focused");
  });

  it("wears the meta on the foot itself, in the foot's own tier", () => {
    render(
      <PaneFoot className="pane-foot" label="deep: where it runs and its context">
        <span>Claude Code</span>
      </PaneFoot>,
    );
    const foot = screen.getByRole("group");
    expect(foot).toHaveClass("ui-text-meta", "pane-foot");
    tieredAs(foot, "meta", "--text-muted", "the foot");
  });
});

/**
 * The words whose components are too wide to draw here, by the class that places them and the
 * role every element written with that class wears beside it.
 */
const WORN: [file: string, placed: string, roles: string[]][] = [
  ["LayoutPanes.tsx", "pane-name", ["ui-text-heading"]],
  ["LayoutPanes.tsx", "pane-number", ["ui-text-meta"]],
  ["LayoutPanes.tsx", "pane-head-where", ["ui-text-secondary"]],
  ["PaneRail.tsx", "pane-rail-count", ["ui-text-meta", "ui-text-strong"]],
  ["TaskQuiz.tsx", "task-quiz-title", ["ui-text-body"]],
  ["TaskQuiz.tsx", "task-quiz-note-words", ["ui-text-secondary"]],
  ["ChatRows.tsx", "tab-task", ["ui-text-meta"]],
  ["Inbox.tsx", "inbox-chain", ["ui-text-secondary", "ui-text-strong"]],
  ["InboxElsewhere.tsx", "inbox-chain", ["ui-text-secondary", "ui-text-strong"]],
  ["Onboarding.tsx", "onboarding-steps", ["ui-text-secondary"]],
  ["GetProject.tsx", "get-project-folder", ["ui-text-secondary"]],
  ["GetProject.tsx", "get-project-local", ["ui-text-secondary"]],
  ["YourRepos.tsx", "your-repos-hosts", ["ui-text-secondary"]],
  ["HarnessSetupTab.tsx", "harness-row-state", ["ui-text-secondary"]],
  ["HarnessSetupTab.tsx", "harness-row-how", ["ui-text-secondary"]],
  ["settings/HostsTable.tsx", "ui-hosts-host", ["ui-text-body", "ui-text-code"]],
  ["settings/HostsTable.tsx", "ui-hosts-added", ["ui-text-secondary"]],
];

/** Every `className="…"` written in `file` that holds `placed`, as its classes. */
function classesWith(file: string, placed: string): string[][] {
  const text = readFileSync(join(process.cwd(), "src", file), "utf8");
  return [...text.matchAll(/className="([^"]*)"/g)]
    .map((found) => found[1].split(/\s+/))
    .filter((classes) => classes.includes(placed));
}

describe("the words placed by a view's class", () => {
  it("wear their role wherever the class is written", () => {
    const bare = WORN.flatMap(([file, placed, roles]) => {
      const sites = classesWith(file, placed);
      // A section of the Inbox's own (Notices, Updates) is the panel family's title, which
      // wears the label itself; only a chain in purlis's words has no role of its own.
      const counted = sites.filter((classes) => classes.length > 1 || placed !== "inbox-chain");
      if (counted.length === 0) return [`${file}: no .${placed} is written`];
      return counted
        .filter((classes) => !roles.every((role) => classes.includes(role)))
        .map((classes) => `${file}: .${placed} wears "${classes.join(" ")}"`);
    });
    expect(bare).toEqual([]);
  });

  it("draw every .came-back line as the secondary, but the warning on the danger wash", () => {
    const src = join(process.cwd(), "src");
    const lines = readdirSync(src, { recursive: true })
      .map(String)
      .filter((file) => file.endsWith(".tsx") && !/\.(test|stories)\.tsx$/.test(file))
      .flatMap((file) =>
        [...readFileSync(join(src, file), "utf8").matchAll(/className="([^"]*)"/g)]
          .map((found) => found[1].split(/\s+/))
          .filter((classes) => classes.includes("came-back"))
          .map((classes) => ({ file, classes })),
      );
    // There are about ninety; a scan that finds none has lost them, not passed.
    expect(lines.length).toBeGreaterThan(80);
    const off = lines
      .filter(({ classes }) =>
        classes.includes("runs-as-you")
          ? !classes.includes("ui-text-body")
          : !classes.includes("ui-text-secondary"),
      )
      .map(({ file, classes }) => `${file}: ${classes.join(" ")}`);
    expect(off).toEqual([]);
  });

  it("head a dialog's sections with the heading and its lists with the meta", () => {
    const dialogs = [
      "About.tsx",
      "ApproveExtension.tsx",
      "ApprovePlane.tsx",
      "DeleteVault.tsx",
      "DeleteWorkspace.tsx",
      "Doctor.tsx",
      "Extensions.tsx",
      "SkillChangeDialogs.tsx",
      "StartChat.tsx",
      "GitMissing.tsx",
      "settings/profiles.tsx",
      "settings/signing.tsx",
      "settings/PoolLimits.tsx",
    ];
    const bare = dialogs.flatMap((file) =>
      [...readFileSync(join(process.cwd(), "src", file), "utf8").matchAll(/<h([34])\b[^>]*>/g)]
        .filter(([tag]) => !/className="[^"]*ui-text-/.test(tag))
        .map(([tag]) => `${file}: ${tag}`),
    );
    expect(bare).toEqual([]);
  });
});

/** The words that wear a role now, by sheet, and the type their rules no longer write. */
const MOVED: [sheet: string, selector: RegExp, properties: string[]][] = [
  [
    "ui/titlebar/titlebar.css",
    /^\[data-part="(project|workspace|count|row-count|crumb)"\]\[data-scope="titlebar"\]$/,
    ["font-size", "font-weight", "font"],
  ],
  [
    "ui/titlebar/titlebar.css",
    /^\[data-part="(key|empty)"\]\[data-scope="titlebar"\]$/,
    ["font-size", "font-weight", "font-family", "color"],
  ],
  ["ui/tabs/tabs.css", /^\[data-scope="editor"\]\[data-part="toolbar"\]$/, ["font-size"]],
  [
    "ui/pane-foot/pane-foot.css",
    /^\[data-scope="pane-foot"\]\[data-part="(root|label|value)"\]$/,
    ["font-size"],
  ],
  [
    "paneFrame.css",
    /^\.pane-chips > \.pane-(name|number)$|^\.pane-rail-count$|^\.task-quiz-(title|sub|why|note|note-words|note-who|foot)$/,
    ["font-size", "font-weight", "color"],
  ],
  [
    "App.css",
    /^\.pane-crumbs( \.crumb-(where|by))?$|^\.tab-task$|^\.inbox-chain$|^\.came-back( code)?$|^\.welcome-pane \.came-back$|^\.onboarding-steps$|^\.get-project-(folder|local)$|^\.your-repos-hosts$|^\.git-missing h3$|^\.harness-row-(state|how)$|^\.ui-hosts-(host|port|added)$|^\.signing-others h3$|^\.pool-limits-head h4$|^\[data-scope="dialog"\] h[34]$/,
    ["font-size", "font-weight", "color"],
  ],
];

describe("the rules of the words that moved", () => {
  it("write no size, weight or colour of words of their own for what now wears a role", () => {
    const typed = MOVED.flatMap(([sheet, selector, properties]) =>
      declarations(readFileSync(join(process.cwd(), "src", sheet), "utf8"))
        .filter(({ context }) =>
          context.some((one) =>
            one.split(/,\s*/).some((part) => selector.test(part.replace(/\s+/g, " ").trim())),
          ),
        )
        .filter(({ property }) => properties.includes(property))
        .map(
          ({ context, property, value }) => `${sheet} ${context.join(" ")} | ${property}: ${value}`,
        ),
    );
    expect(typed).toEqual([]);
  });
});
