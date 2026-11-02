// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useState } from "react";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { SettingsTab } from "./SettingsTab";
import type { Level } from "./groups";
import { forgetThisLaunch } from "../regions";
import { ReadOnlySaid } from "../ReadOnlyLine";
import { GLOBAL } from "../windowprefs";
import { typeable } from "../editor/typing.testkit";
import type {
  ProjectExtension,
  SettingsEdit,
  SettingsField,
  SettingsFile,
  SettingsStep,
  SettingsWhich,
} from "../bindings";
import { pickIn, querySelectNamed } from "../ui/select/select.testkit";
import { tipOf } from "../ui/tooltip/tooltip.testkit";

/**
 * **The Settings tab at the Project level** (SE-17, #1167; the spec on #558, V89b, V89e, V89h):
 * the project's groups in V89h's order, each control writing its one key through the core as it
 * changes, Undo of the last change, and a refused write said beside its setting. The core is the
 * mock here: what it keeps of a file and what it refuses is `purlis_core::settings`'s tests.
 * What is held here is what the person sees and what the window sends.
 */

const PLANE = "/home/dev/plane";

const SHARED_TEXT = `# the team's
[plane]
mode = "push"   # how far a save goes

[[forge]]
kind = "github"
owner = "acme"
`;

const field = (keys: (string | number)[], value: SettingsField["value"]): SettingsField => ({
  path: keys.map((one) => (typeof one === "number" ? { index: one } : { key: one })),
  value,
});

const SHARED: SettingsFile = {
  which: "shared",
  file: "charter.toml",
  exists: true,
  text: SHARED_TEXT,
  refusals: [],
  parsed: true,
  fields: [
    field(["plane", "mode"], { kind: "text", value: "push" }),
    field(["forge", 0, "kind"], { kind: "text", value: "github" }),
    field(["forge", 0, "owner"], { kind: "text", value: "acme" }),
  ],
};

const LOCAL: SettingsFile = {
  which: "local",
  file: "charter.local.toml",
  exists: true,
  text: `[harness.work]\nkind = "claude"\ncommand = ["claude"]\n`,
  refusals: [],
  parsed: true,
  fields: [
    field(["harness", "work", "kind"], { kind: "text", value: "claude" }),
    field(["harness", "work", "command"], { kind: "list", value: ["claude"] }),
  ],
  // The core lists the profile as an entry of the profiles collection, with a page (ST-4).
  entries: [
    {
      collection: "profiles",
      id: "profile:work",
      label: "work",
      keys: [{ key: "harness" }, { key: "work" }],
      values: [
        { field: "name", value: "work" },
        { field: "kind", value: "claude" },
        { field: "command", value: "claude" },
      ],
    },
  ],
};

const said = (value: string | null, source = "default") => ({ value, source });

const SAVING = {
  plane: {
    mode: said("push", "shared"),
    from_share: false,
    branch: said(null),
    save_branch: said(null),
    sign: said("off"),
    autosave: said("on"),
    autosave_after: said("1m"),
  },
  repos: [],
  plane_left_out: null,
  repos_left_out: null,
};

const THEME = {
  options: [
    { value: "purlis-dark", label: "purlis-dark (built in)" },
    { value: "purlis-light", label: "purlis-light (built in)" },
  ],
  picked: null,
  file: null,
  draws: null,
  why: null,
  colour: null,
  ignored: [],
  local_left_out: null,
};

const LINTER: ProjectExtension = {
  id: "linter",
  name: "Linter",
  state: "on",
  source: "default",
  settings: [],
  ignored: [],
};

/** A file as the core would answer it after `edits`: each edited key set or gone, and a text
 *  that differs, so the next write is checked against the new one. */
function applied(file: SettingsFile, edits: readonly SettingsEdit[]): SettingsFile {
  let fields = [...file.fields];
  for (const edit of edits) {
    const at = JSON.stringify(edit.path);
    fields = fields.filter((one) => JSON.stringify(one.path) !== at);
    if (edit.value !== null) fields.push({ path: edit.path, value: edit.value });
  }
  return { ...file, exists: true, fields, text: `${file.text}# written\n` };
}

type Sent = { which: SettingsWhich; base: string | null; edits: SettingsEdit[] };

type Moved = {
  to: SettingsWhich;
  sharedBase: string | null;
  localBase: string | null;
  paths: SettingsStep[][];
};

/** Both files as the core would answer them after moving `paths` into `to`. */
function moved(
  files: Record<SettingsWhich, SettingsFile>,
  to: SettingsWhich,
  paths: readonly SettingsStep[][],
) {
  const from: SettingsWhich = to === "shared" ? "local" : "shared";
  const at = (one: SettingsField) =>
    paths.some((path) => JSON.stringify(path) === JSON.stringify(one.path));
  const carried = files[from].fields.filter(at);
  files[from] = applied(
    files[from],
    carried.map((one) => ({ path: one.path, value: null })),
  );
  files[to] = applied(
    files[to],
    carried.map((one) => ({ path: one.path, value: one.value })),
  );
}

/** The core, as a mock. `refuse` answers a write with these reasons instead of writing it. */
function core({
  extensions = [] as ProjectExtension[],
  refuse = undefined as string[] | undefined,
  sandboxOn = false,
  /** Each write waits for `release()` before the core answers it. */
  hold = false,
  /** A move is answered with these reasons, and neither file changes. */
  refuseMove = undefined as string[] | undefined,
  shared = SHARED,
  local = LOCAL,
} = {}) {
  const files: Record<SettingsWhich, SettingsFile> = { shared, local };
  const sent: Sent[] = [];
  const moves: Moved[] = [];
  const held: (() => void)[] = [];
  let reads = 0;
  mockIPC(
    async (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "project_chats_run_on":
          return { pool: null, pools: [] };
        case "project_settings":
          reads += 1;
          return { shared: files.shared, local: files.local };
        case "project_extensions":
          return { extensions, local_left_out: null };
        case "project_saving_in_force":
          return SAVING;
        case "project_theme":
          return THEME;
        case "project_theme_drawn":
          return null;
        case "extensions_on":
          return [];
        case "sandbox_state":
          return sandboxOn
            ? {
                on: true,
                offer: false,
                said: "No chat started without it",
                never: [],
                hosts_changed: null,
                presets_changed: null,
                presets: [],
                persona_hosts: [],
                besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
                policy: null,
              }
            : {
                on: false,
                offer: false,
                said: null,
                never: [],
                hosts_changed: null,
                presets_changed: null,
                presets: [],
                persona_hosts: [],
                besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
                policy: null,
              };
        case "save_project_settings": {
          const which = given.which as SettingsWhich;
          const change = given.change as { kind: "edits"; edits: SettingsEdit[] };
          sent.push({ which, base: given.base as string | null, edits: change.edits });
          if (hold) await new Promise<void>((go) => held.push(go));
          if (refuse) return { kind: "refused", reasons: refuse };
          files[which] = applied(files[which], change.edits);
          return { kind: "saved", file: files[which] };
        }
        case "move_project_settings": {
          const move = {
            to: given.to as SettingsWhich,
            sharedBase: given.sharedBase as string | null,
            localBase: given.localBase as string | null,
            paths: given.paths as SettingsStep[][],
          };
          moves.push(move);
          if (refuseMove) return { kind: "refused", reasons: refuseMove };
          moved(files, move.to, move.paths);
          return { kind: "moved", settings: { shared: files.shared, local: files.local } };
        }
      }
      return undefined;
    },
    { shouldMockEvents: true },
  );
  return {
    sent,
    moves,
    reads: () => reads,
    files,
    /** Lets the oldest held write be answered. */
    release: () => act(async () => held.shift()?.()),
  };
}

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: {
      path: "/home/dev/.config/charter/layout.json",
      found: false,
      document: null,
      trouble: null,
    },
    theme: { path: "", found: false, document: null, trouble: null },
  };
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

/** The tab as a view tab holds it: its level is the tab's, and the switcher changes it. */
function Tab({ at = "project" }: { at?: Level }) {
  const [level, setLevel] = useState<Level>(at);
  return <SettingsTab plane={PLANE} level={level} onLevelChange={setLevel} />;
}

const nav = () => screen.getByRole("navigation", { name: "Groups" });
const groups = () =>
  within(nav())
    .getAllByRole("button")
    .map((one) => one.textContent);
const open = (name: string) => userEvent.click(within(nav()).getByRole("button", { name }));
const shown = () => screen.getByRole("region", { name: /./ });

async function atProject() {
  render(<Tab />);
  await screen.findByRole("button", { name: "General" });
}

describe("the Project level", () => {
  it("is offered beside You, and the switcher moves the tab to it", async () => {
    core();
    render(<Tab at="you" />);

    await userEvent.click(screen.getByRole("radio", { name: "Project" }));

    expect(screen.getByRole("radio", { name: "Project" })).toBeChecked();
    expect(await within(nav()).findByRole("button", { name: "General" })).toBeInTheDocument();
  });

  it("lists the groups in order when each has something to set, each profile's page under its group", async () => {
    core({ extensions: [LINTER] });
    await atProject();

    await waitFor(() =>
      expect(groups()).toEqual([
        "General",
        "Saving",
        "Harness & profiles",
        "work",
        "Sandbox",
        "Your hosts",
        "Your level",
        "Persona hosts",
        "Network",
        "Dispatch",
        "Runs on",
        "Forges",
        "Extensions",
        "Theme",
        "Appearance",
      ]),
    );
  });

  it("keeps what the files refuse on screen under a filter that matches nothing (SE-21)", async () => {
    const { files } = core();
    files.shared = {
      ...files.shared,
      refusals: [{ why: "[plane] mode is not one charter knows", key: null }],
    };
    await atProject();

    await userEvent.type(
      screen.getByRole("searchbox", { name: "Filter settings" }),
      "colour of the moon",
    );

    expect(within(nav()).queryAllByRole("button")).toEqual([]);
    expect(
      screen.getByText("No setting at this level matches “colour of the moon”.", {
        ignore: "[role=status]",
      }),
    ).toBeVisible();
    expect(screen.getByText("charter.toml: [plane] mode is not one charter knows")).toBeVisible();
  });

  it("hides a group with nothing in it, but Extensions, which links to where one is installed", async () => {
    core();
    await atProject();

    // Offered for its link to the Extensions dialog, saying why it has nothing (#1387, #1198).
    expect(groups()).toContain("Extensions");
    // #3467: no Plugins group is left, whatever the project holds.
    expect(groups()).not.toContain("Plugins");
    expect(groups()).toContain("Forges");
  });

  it("says which file a setting is kept in", async () => {
    core();
    await atProject();
    await open("Saving");

    expect(screen.getByLabelText("Mode")).toHaveAccessibleDescription(/charter\.toml/);
    await open("work");
    expect(screen.getByLabelText("work: kind")).toHaveAccessibleDescription(/charter\.local\.toml/);
  });
});

describe("every project setting there is, at the Project level", () => {
  it.each([
    [
      "General",
      ["Default workspace", "Default persona", "Update channel", "Version lock", "Branch folders"],
    ],
    [
      "Saving",
      [
        "Mode",
        "Target branch",
        "Save branch",
        "Sign commits",
        "Auto-save",
        "Auto-save after",
        "Assisted-by trailer",
        "[memory] share (deprecated)",
      ],
    ],
    ["Harness & profiles", ["Default harness", "Default profile", "Environment passed to chats"]],
    ["work", ["work: kind", "work: command", "work: environment"]],
    [
      "Sandbox",
      ["Internet access", "Certificate checks", "What chats can change", "Always protected"],
    ],
    ["Dispatch", ["Limits", "Dispatch grants", "Policy locks"]],
    ["Runs on", ["Chats run on"]],
    ["Forges", ["Forge 1: kind", "Forge 1: owner", "Forge 1: host", "Forge 1: repos never listed"]],
    ["Extensions", ["Linter: enabled"]],
    ["Appearance", ["Theme", "Icons"]],
  ])("%s has its settings", async (name, labels) => {
    core({ extensions: [LINTER] });
    await atProject();
    await waitFor(() => expect(groups()).toContain(name));

    await open(name);

    expect(shown()).toHaveAccessibleName(name);
    for (const label of labels) expect(within(shown()).getByLabelText(label)).toBeInTheDocument();
  });
});

describe("a change at the Project level", () => {
  it("writes its one key to its file as it is picked, against the text it was read as", async () => {
    const { sent } = core();
    await atProject();
    await open("Saving");

    await pickIn(screen.getByLabelText("Mode"), "commit");

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      which: "shared",
      base: SHARED_TEXT,
      edits: [
        { path: [{ key: "plane" }, { key: "mode" }], value: { kind: "text", value: "commit" } },
      ],
    });
    expect(screen.getByLabelText("Mode")).toHaveValue("commit");
  });

  it("writes a typed value once, when the field is left, and not at every key", async () => {
    const { sent } = core();
    await atProject();

    // ST-1 made the defaults pickers: Branch folders is one line of text.
    await userEvent.type(screen.getByLabelText("Branch folders"), "../wt");
    expect(sent).toHaveLength(0);
    await userEvent.tab();

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].edits).toEqual([
      { path: [{ key: "plane" }, { key: "worktrees" }], value: { kind: "text", value: "../wt" } },
    ]);
  });

  it("writes a local setting to charter.local.toml", async () => {
    const { sent } = core();
    await atProject();
    await open("work");

    await pickIn(screen.getByLabelText("work: kind"), "codex");

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].which).toBe("local");
  });

  it("undoes the last change by writing the value it had back", async () => {
    const { sent } = core();
    await atProject();
    await open("Saving");
    await pickIn(screen.getByLabelText("Mode"), "commit");

    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));

    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[1].which).toBe("shared");
    expect(sent[1].edits).toEqual([
      { path: [{ key: "plane" }, { key: "mode" }], value: { kind: "text", value: "push" } },
    ]);
    expect(screen.getByLabelText("Mode")).toHaveValue("push");
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
  });

  it("undoes a key that was not set by taking it out again", async () => {
    const { sent } = core();
    await atProject();
    await open("Saving");
    // Off in force and not set here: turning the switch on writes the key (#1865).
    await userEvent.click(await screen.findByRole("switch", { name: "Sign commits" }));

    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));

    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[1].edits).toEqual([{ path: [{ key: "plane" }, { key: "sign" }], value: null }]);
  });

  it("says a refused write beside its setting, and shows what is on disk", async () => {
    core({ refuse: ["plane.mode in charter.toml is not a mode"] });
    await atProject();
    await open("Saving");

    await pickIn(screen.getByLabelText("Mode"), "pr");

    const row = (await screen.findByText("plane.mode in charter.toml is not a mode")).closest(
      ".ui-setting-row",
    );
    expect(row).not.toBeNull();
    expect(within(row as HTMLElement).getByLabelText("Mode")).toHaveValue("push");
    expect(screen.getByRole("alert")).toHaveTextContent("plane.mode in charter.toml is not a mode");
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
  });

  it("offers no Undo that would take the sandbox off once it is turned on", async () => {
    const { sent } = core();
    await atProject();
    await open("Sandbox");

    await userEvent.click(screen.getByRole("button", { name: "Turn the sandbox on" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    await waitFor(() =>
      expect(screen.getByRole("group", { name: "Sandbox" })).toHaveTextContent(
        "On for everyone in this project.",
      ),
    );
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
  });

  it("offers no way back to not set while turning the sandbox on is being written", async () => {
    const { sent, release, files } = core({ hold: true });
    await atProject();
    await open("Sandbox");

    await userEvent.click(screen.getByRole("button", { name: "Turn the sandbox on" }));

    expect(screen.queryByRole("button", { name: "Turn the sandbox on" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Reset" })).toBeNull();
    await waitFor(() => expect(sent).toHaveLength(1));
    await release();
    await waitFor(() =>
      expect(screen.getByRole("group", { name: "Sandbox" })).toHaveTextContent(
        "On for everyone in this project.",
      ),
    );
    expect(sent).toHaveLength(1);
    expect(files.shared.fields).toContainEqual(
      field(["sandbox", "mode"], { kind: "text", value: "on" }),
    );
  });

  it("writes the environment passed to chats to charter.local.toml", async () => {
    const { sent } = core();
    await atProject();
    await open("Harness & profiles");

    await userEvent.type(screen.getByLabelText("Environment passed to chats"), "LANG");
    await userEvent.tab();

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      which: "local",
      edits: [
        { path: [{ key: "chat_env" }, { key: "pass" }], value: { kind: "list", value: ["LANG"] } },
      ],
    });
  });

  it("keeps a typed value that goes back to what it was while the first write is pending", async () => {
    const { sent, release } = core({ hold: true });
    await atProject();
    const box = () => screen.getByLabelText("Branch folders");

    await userEvent.type(box(), "../wt");
    await userEvent.tab();
    await userEvent.clear(box());
    await userEvent.tab();
    await release();
    await waitFor(() => expect(sent).toHaveLength(2));
    await release();

    expect(sent[1].edits).toEqual([
      { path: [{ key: "plane" }, { key: "worktrees" }], value: null },
    ]);
    await waitFor(() => expect(box()).toHaveValue(""));
  });

  it("offers no Undo while a change to that setting is still being written", async () => {
    const { release } = core({ hold: true });
    await atProject();
    await open("Saving");
    await pickIn(screen.getByLabelText("Mode"), "commit");
    await release();
    expect(await screen.findByRole("button", { name: "Undo" })).toBeInTheDocument();

    await pickIn(screen.getByLabelText("Mode"), "pr");

    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    await release();
    expect(await screen.findByRole("button", { name: "Undo" })).toBeInTheDocument();
  });

  it("puts the focus back on the setting once Undo is pressed", async () => {
    core();
    await atProject();
    await open("Saving");
    await pickIn(screen.getByLabelText("Mode"), "commit");

    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));

    expect(screen.getByLabelText("Mode")).toHaveFocus();
  });

  it("reads a file changed outside the tab again before the next write", async () => {
    const { sent, files } = core();
    await atProject();
    await open("Saving");
    const outside = `${files.shared.text}# edited by hand\n`;
    files.shared = { ...files.shared, text: outside };

    await act(async () => {
      await emit("plane-changed", { plane: PLANE, changes: [], answers: [{ answer: "settings" }] });
    });
    await pickIn(screen.getByLabelText("Mode"), "commit");

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].base).toBe(outside);
  });

  it("never offers to take the sandbox off once the project has it on", async () => {
    core({ sandboxOn: true });
    SHARED.fields.push(field(["sandbox", "mode"], { kind: "text", value: "on" }));
    try {
      await atProject();
      await open("Sandbox");

      expect(screen.getByRole("group", { name: "Sandbox" })).toHaveTextContent(
        "On for everyone in this project.",
      );
      expect(screen.queryByRole("button", { name: "Turn the sandbox on" })).toBeNull();
      expect(querySelectNamed("Sandbox mode")).toBeNull();
    } finally {
      SHARED.fields.pop();
    }
  });
});

/** The row a setting is drawn in, found by its control's label. */
function rowOf(label: string): HTMLElement {
  const row = screen.getByLabelText(label).closest(".ui-setting-row");
  if (!(row instanceof HTMLElement)) throw new Error(`no row for ${label}`);
  return row;
}

const MODE = [{ key: "plane" }, { key: "mode" }];

/** Opens a row's "Move to…" menu (DS-8, #1840), and answers the menu. */
async function moveMenu(label: string): Promise<HTMLElement> {
  await userEvent.click(within(rowOf(label)).getByRole("button", { name: "Move to…" }));
  return screen.findByRole("menu");
}

/** A Local file that sets the project's save mode, over charter.toml's `push`. */
const LOCAL_MODE: SettingsFile = {
  ...LOCAL,
  text: `${LOCAL.text}[plane]\nmode = "pr"\n`,
  fields: [...LOCAL.fields, field(["plane", "mode"], { kind: "text", value: "pr" })],
};

describe("one form for Shared and Local (SE-18)", () => {
  it("says once, in General's header, where its values are kept, with one help line and none per row (DS-8)", async () => {
    core();
    await atProject();
    await open("General");

    const group = screen.getByRole("region", { name: "General" });
    expect(within(group).getAllByText(/Kept in charter\.toml/)).toHaveLength(1);
    // Exactly one help line is drawn in the group: its own.
    expect(
      [...group.querySelectorAll("p")].filter((one) => !one.classList.contains("sr-only")),
    ).toHaveLength(1);
    const rows = [...group.querySelectorAll<HTMLElement>(".ui-setting-row")];
    expect(rows.length).toBeGreaterThan(1);
    for (const row of rows) {
      expect(row).not.toHaveTextContent(/Kept in|From charter|at the Project level/);
      // No help line of its own (D3): the hint is the label's tooltip.
      expect(row.querySelectorAll("p.ui-setting-help")).toHaveLength(0);
    }
  });

  it("says once, in Dispatch's header, where its tables' values are kept (DS-10)", async () => {
    core();
    await atProject();
    await open("Dispatch");

    const group = screen.getByRole("region", { name: "Dispatch" });
    expect(
      within(group).getAllByText(/Kept in charter\.toml, which your team sees\./),
    ).toHaveLength(1);
  });

  it("badges a row kept in the other file than its group's, with that level", async () => {
    core();
    await atProject();
    await open("Harness & profiles");

    // The group keeps its values on this machine; the default harness is the team's.
    const group = screen.getByRole("region", { name: "Harness & profiles" });
    expect(within(group).getAllByText(/Kept in charter\.local\.toml/)).toHaveLength(1);
    expect(
      within(rowOf("Default harness")).getByText("Team", { selector: "[data-scope=badge]" }),
    ).toBeVisible();
    expect(
      within(rowOf("Default profile")).queryByText("Team", { selector: "[data-scope=badge]" }),
    ).toBeNull();
  });

  it("says which level and file each value comes from, and offers its reset", async () => {
    core();
    await atProject();
    await open("Saving");

    expect(screen.getByLabelText("Mode")).toHaveAccessibleDescription(
      /From charter\.toml, at the Project level: shared with your team\./,
    );
    expect(within(rowOf("Mode")).getByRole("button", { name: "Reset" })).toBeInTheDocument();
    expect(screen.getByLabelText("Target branch")).toHaveAccessibleDescription(
      /Not set at this level/,
    );
    expect(within(rowOf("Target branch")).queryByRole("button", { name: "Reset" })).toBeNull();
  });

  it("moves a value to this machine only from its Move to menu", async () => {
    const { sent, moves } = core();
    await atProject();
    await open("Saving");

    const menu = await moveMenu("Mode");
    // Held in charter.toml, so the only place it can go is the other file.
    expect(within(menu).getAllByRole("menuitem")).toHaveLength(1);
    expect(moves).toEqual([]);
    await userEvent.click(
      // Which file that is, and who sees it, is the row's description and tooltip (the menus'
      // rule 2), not a line under its words.
      within(menu).getByRole("menuitem", {
        name: "This machine",
        description: "charter.local.toml, on this machine only",
      }),
    );

    await waitFor(() => expect(moves).toHaveLength(1));
    expect(moves[0]).toEqual({
      to: "local",
      sharedBase: SHARED_TEXT,
      localBase: LOCAL.text,
      paths: [MODE],
    });
    expect(sent).toEqual([]);
    await waitFor(() =>
      expect(screen.getByLabelText("Mode")).toHaveAccessibleDescription(
        /From charter\.local\.toml, at the Project level: this machine only\./,
      ),
    );
    expect(screen.getByLabelText("Mode")).toHaveValue("push");
    // Now kept elsewhere than its group keeps its values: badged with where.
    expect(
      within(rowOf("Mode")).getByText("This machine", { selector: "[data-scope=badge]" }),
    ).toBeVisible();
  });

  it("moves a value back to charter.toml", async () => {
    const { moves, files } = core({ local: LOCAL_MODE });
    await atProject();
    await open("Saving");

    await userEvent.click(within(await moveMenu("Mode")).getByRole("menuitem", { name: "Team" }));

    await waitFor(() => expect(moves).toHaveLength(1));
    expect(moves[0]).toMatchObject({ to: "shared", paths: [MODE] });
    expect(files.shared.fields).toContainEqual(
      field(["plane", "mode"], { kind: "text", value: "pr" }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("Mode")).toHaveAccessibleDescription(/From charter\.toml/),
    );
  });

  it("names the team's value a move to charter.toml replaces", async () => {
    core({ local: LOCAL_MODE });
    await atProject();
    await open("Saving");

    const team = within(await moveMenu("Mode")).getByRole("menuitem", { name: "Team" });
    expect(team).toHaveAccessibleDescription("charter.toml, which your team sees, replacing push");
    // No Undo puts the team's value back, so that much stays under the row's words.
    expect(team.querySelector('[data-part="description"]')).toHaveTextContent(
      "Replaces push there.",
    );
  });

  it("puts the focus back on Move to… once a move is chosen", async () => {
    const { moves } = core();
    await atProject();
    await open("Saving");

    await userEvent.click(
      within(await moveMenu("Mode")).getByRole("menuitem", { name: /^This machine/ }),
    );

    await waitFor(() => expect(moves).toHaveLength(1));
    await waitFor(() =>
      expect(within(rowOf("Mode")).getByRole("button", { name: "Move to…" })).toHaveFocus(),
    );
  });

  it("puts the focus back on the setting once Reset is pressed", async () => {
    const { sent } = core({ local: LOCAL_MODE });
    await atProject();
    await open("Saving");

    await userEvent.click(within(rowOf("Mode")).getByRole("button", { name: "Reset" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(screen.getByLabelText("Mode")).toHaveFocus();
  });

  it("does not call the default profile an override of charter.toml's default harness", async () => {
    const shared = {
      ...SHARED,
      fields: [...SHARED.fields, field(["harness", "default"], { kind: "text", value: "claude" })],
    };
    const local = {
      ...LOCAL,
      fields: [...LOCAL.fields, field(["harness", "default"], { kind: "text", value: "work" })],
    };
    core({ shared, local });
    await atProject();
    await open("Harness & profiles");

    expect(screen.getByLabelText("Default profile")).toHaveValue("work");
    // Kept where its group keeps its values: no badge.
    expect(
      within(rowOf("Default profile")).queryByText(/Team|This machine/, {
        selector: "[data-scope=badge]",
      }),
    ).toBeNull();
    expect(screen.getByLabelText("Default profile")).not.toHaveAccessibleDescription(/overrides/);
    expect(screen.getByLabelText("Default profile")).toHaveAccessibleDescription(
      /From charter\.local\.toml, at the Project level: this machine only\./,
    );
  });

  it("moves nothing while the arrows pass over the menu, and nothing when it is closed", async () => {
    const { moves, sent } = core();
    await atProject();
    await open("Saving");

    within(rowOf("Mode")).getByRole("button", { name: "Move to…" }).focus();
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("menu");
    await userEvent.keyboard("{ArrowDown}{ArrowUp}{ArrowDown}");
    await userEvent.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(moves).toEqual([]);
    expect(sent).toEqual([]);
  });

  it("badges a Local value that overrides a Shared one; its reset lets the Shared value show", async () => {
    const { sent } = core({ local: LOCAL_MODE });
    await atProject();
    await open("Saving");

    expect(screen.getByLabelText("Mode")).toHaveValue("pr");
    const badge = within(rowOf("Mode")).getByText("This machine", {
      selector: "[data-scope=badge]",
    });
    expect(screen.getByLabelText("Mode")).toHaveAccessibleDescription(
      /charter\.toml has push, which this overrides/,
    );
    // The whole sentence is the badge's tooltip.
    expect(tipOf(badge)).toContain(
      "From charter.local.toml, at the Project level: this machine only. charter.toml has push, which this overrides.",
    );

    await userEvent.click(within(rowOf("Mode")).getByRole("button", { name: "Reset" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      which: "local",
      base: LOCAL_MODE.text,
      edits: [{ path: MODE, value: null }],
    });
    await waitFor(() => expect(screen.getByLabelText("Mode")).toHaveValue("push"));
    expect(
      within(rowOf("Mode")).queryByText("This machine", { selector: "[data-scope=badge]" }),
    ).toBeNull();
    expect(screen.getByLabelText("Mode")).toHaveAccessibleDescription(/From charter\.toml/);
  });

  it("undoes a reset by writing the value back", async () => {
    const { sent } = core({ local: LOCAL_MODE });
    await atProject();
    await open("Saving");
    await userEvent.click(within(rowOf("Mode")).getByRole("button", { name: "Reset" }));

    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));

    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[1]).toMatchObject({
      which: "local",
      edits: [{ path: MODE, value: { kind: "text", value: "pr" } }],
    });
  });

  it("writes a value no file holds to the file picked for it, and the pick writes nothing", async () => {
    const { sent, moves } = core();
    await atProject();
    await open("Saving");

    const menu = await moveMenu("Target branch");
    // No file holds it: the menu says where the next value goes, and moves nothing.
    expect(within(menu).queryAllByRole("menuitem")).toEqual([]);
    await userEvent.click(within(menu).getByRole("menuitemradio", { name: /^This machine/ }));
    expect(sent).toEqual([]);
    expect(
      within(rowOf("Target branch")).getByText("This machine", { selector: "[data-scope=badge]" }),
    ).toBeVisible();
    await userEvent.type(screen.getByLabelText("Target branch"), "trunk");
    await userEvent.tab();

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      which: "local",
      edits: [
        { path: [{ key: "plane" }, { key: "branch" }], value: { kind: "text", value: "trunk" } },
      ],
    });
    expect(moves).toEqual([]);
  });

  it("says a refused move beside its setting, with the value where it was", async () => {
    core({ refuseMove: ["git would commit charter.local.toml"] });
    await atProject();
    await open("Saving");

    await userEvent.click(
      within(await moveMenu("Mode")).getByRole("menuitem", { name: /^This machine/ }),
    );

    expect(await within(rowOf("Mode")).findByRole("alert")).toHaveTextContent(
      "git would commit charter.local.toml",
    );
    expect(screen.getByLabelText("Mode")).toHaveAccessibleDescription(/From charter\.toml/);
  });

  it.each([
    ["Harness & profiles", "Default profile"],
    ["work", "work: kind"],
    ["work", "work: command"],
    ["work", "work: environment"],
    ["Harness & profiles", "Environment passed to chats"],
  ])("never offers Shared on %s for %s, which only this machine may hold", async (group, label) => {
    core();
    await atProject();
    await open(group);

    expect(within(rowOf(label)).queryByRole("button", { name: "Move to…" })).toBeNull();
  });

  it.each([
    ["General", "Branch folders"],
    ["Sandbox", "Internet access"],
    ["Forges", "Forge 1: kind"],
  ])(
    "offers no file choice in %s for %s, which only charter.toml may hold",
    async (group, label) => {
      core();
      await atProject();
      await open(group);

      expect(within(rowOf(label)).queryByRole("button", { name: "Move to…" })).toBeNull();
    },
  );

  it("never offers to reset or move the sandbox once it is on", async () => {
    const shared = {
      ...SHARED,
      fields: [...SHARED.fields, field(["sandbox", "mode"], { kind: "text", value: "on" })],
    };
    core({ sandboxOn: true, shared });
    await atProject();
    await open("Sandbox");

    const status = screen.getByRole("group", { name: "Sandbox" });
    const row = status.closest(".ui-setting-row") as HTMLElement;
    expect(within(row).queryByRole("button", { name: "Reset" })).toBeNull();
    expect(within(row).queryByRole("button", { name: "Move to…" })).toBeNull();
    expect(status).toHaveAccessibleDescription(/From charter\.toml/);
  });

  it("shows no group twice, and each setting once", async () => {
    core({ extensions: [LINTER], local: LOCAL_MODE });
    await atProject();
    await waitFor(() => expect(groups()).toContain("Appearance"));

    expect(new Set(groups()).size).toBe(groups().length);
    const labels: string[] = [];
    for (const name of groups() as string[]) {
      await open(name);
      // A page that is a collection alone (Your hosts, #1341) has no rows of its own.
      for (const label of within(shown()).queryAllByText(/./, { selector: ".ui-setting-label" }))
        labels.push(label.textContent ?? "");
    }
    expect(labels.filter((one, at) => labels.indexOf(one) !== at)).toEqual([]);
    expect(labels.filter((one) => one === "Mode")).toHaveLength(1);
  });
});

describe("the Project level of a read-only project (#826)", () => {
  const SENTENCE =
    "this project requires the feature memory-proposals, which purlis 0.8.0 does not have; " +
    "purlis 0.9.0 or later has it. purlis opens the project read-only and writes nothing to it.";

  it("shows a committed file's settings with their controls off and the sentence as why; the nav still moves", async () => {
    core();
    render(
      <ReadOnlySaid.Provider value={SENTENCE}>
        <Tab />
      </ReadOnlySaid.Provider>,
    );
    await screen.findByRole("button", { name: "General" });
    await open("Saving");

    const mode = screen.getByLabelText("Mode");
    expect(mode).toBeDisabled();
    // Why it is off: its group\'s description, which its legend\'s tooltip says too (#2908).
    expect(mode.closest("fieldset")).toHaveAccessibleDescription(SENTENCE);
    // The level switcher and the nav are not the project's files: still on.
    expect(screen.getByRole("radio", { name: "You" })).toBeEnabled();
  });

  /** The tab, on a read-only project, at the Project level. */
  async function readOnly() {
    core();
    render(
      <ReadOnlySaid.Provider value={SENTENCE}>
        <Tab />
      </ReadOnlySaid.Provider>,
    );
    await screen.findByRole("button", { name: "General" });
  }

  it("keeps on each row that writes only this machine's file, row by row (#1759)", async () => {
    await readOnly();

    await open("Harness & profiles");
    // The default harness is the committed file's; the default profile and the environment
    // are charter.local.toml's, which the core still writes on a read-only project.
    expect(screen.getByLabelText("Default harness")).toBeDisabled();
    expect(screen.getByLabelText("Default profile")).toBeEnabled();
    expect(screen.getByLabelText("Environment passed to chats")).toBeEnabled();
    expect(
      screen.getByLabelText("Environment passed to chats").closest("fieldset.read-only-off"),
    ).toBeNull();
    // A profile's own page is this machine's file throughout.
    await open("work");
    expect(screen.getByLabelText("work: kind")).toBeEnabled();
  });

  it("keeps a collection of this machine's file on, and one of the committed file off (#1759)", async () => {
    await readOnly();

    await open("Your hosts");
    expect(screen.getByRole("button", { name: "Add host" })).toBeEnabled();
    await open("Forges");
    const add = screen.getByRole("button", { name: "Add forge" });
    expect(add).toBeDisabled();
    // Why it is off: its group\'s description, which its legend\'s tooltip says too (#2908).
    expect(add.closest("fieldset")).toHaveAccessibleDescription(SENTENCE);
  });

  it("keeps a row that may move to the committed file off, wherever its value is (#1759)", async () => {
    await readOnly();

    await open("Saving");

    // Mode may be kept in either file, and its file choice writes the committed one.
    expect(screen.getByLabelText("Mode")).toBeDisabled();
  });

  it("keeps Discover off: it catalogues repos into a committed file (#1795)", async () => {
    await readOnly();

    await open("Saving");

    const row = screen.getByRole("group", { name: "Repos not catalogued" });
    const discover = within(row).getByRole("button", { name: "Discover repos" });
    expect(discover).toBeDisabled();
    // Why it is off: its group\'s description, which its legend\'s tooltip says too (#2908).
    expect(discover.closest("fieldset")).toHaveAccessibleDescription(SENTENCE);
  });

  it("keeps a row held in this machine's file on, with only its move off (#1759)", async () => {
    const { sent, moves } = core({ local: LOCAL_MODE });
    render(
      <ReadOnlySaid.Provider value={SENTENCE}>
        <Tab />
      </ReadOnlySaid.Provider>,
    );
    await screen.findByRole("button", { name: "General" });
    await open("Saving");

    // Mode is kept in charter.local.toml here: its value and its reset write that file alone.
    expect(screen.getByLabelText("Mode")).toBeEnabled();
    expect(screen.getByLabelText("Mode")).toHaveValue("pr");
    // Its Move to… writes the committed file too, so that is what is off, with the reason.
    const move = within(rowOf("Mode")).getByRole("button", { name: "Move to…" });
    expect(move).toBeDisabled();
    // Why it is off: its group\'s description, which its legend\'s tooltip says too (#2908).
    expect(move.closest("fieldset[disabled]")).toHaveAccessibleDescription(SENTENCE);

    await userEvent.click(within(rowOf("Mode")).getByRole("button", { name: "Reset" }));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({ which: "local", edits: [{ path: MODE, value: null }] });
    expect(moves).toEqual([]);
    // Now the committed file's value is in force, and the row writes that file: off.
    await waitFor(() => expect(screen.getByLabelText("Mode")).toBeDisabled());
  });

  it("keeps Edit as TOML on for this machine's file and off for the committed one (#1759)", async () => {
    await readOnly();
    const raw = (name: string) => screen.findByRole("textbox", { name: `${name}, as TOML` });
    const editAsToml = (name: string) =>
      userEvent.click(
        within(screen.getByRole("group", { name: "Edit as TOML" })).getByRole("button", { name }),
      );

    await editAsToml("charter.local.toml");
    expect(typeable(await raw("charter.local.toml"))).toBe(true);
    await editAsToml("charter.toml");
    expect(typeable(await raw("charter.toml"))).toBe(false);
  });

  it("keeps a group's links out of Settings on: they write nothing (#1759)", async () => {
    await readOnly();

    await open("Extensions");

    expect(screen.getByRole("button", { name: "Go to Extensions…" })).toBeEnabled();
  });

  it("leaves on, in every group, only what writes this machine's file or nothing (#1759)", async () => {
    await readOnly();
    const on: string[] = [];
    for (const group of groups()) {
      if (group === null) continue;
      await open(group);
      for (const control of shown().querySelectorAll<HTMLElement>(
        "input, select, textarea, button",
      )) {
        // A select's hidden native twin, for a form's autofill, is no control a person reaches.
        if (control.closest('[data-testid="hidden-select-container"]') !== null) continue;
        // Off: disabled, inside a disabled fieldset, or unavailable and kept in reach for its
        // reason (`aria-disabled`, #2908).
        const off =
          (control as HTMLInputElement).disabled ||
          control.getAttribute("aria-disabled") === "true" ||
          control.closest("fieldset[disabled]") !== null;
        if (off) continue;
        const name =
          control.getAttribute("aria-label") ??
          (control as HTMLInputElement).labels?.[0]?.textContent ??
          control.textContent ??
          "";
        on.push(`${group}: ${name.trim()}`);
      }
    }
    // Every one is charter.local.toml's (the default profile, the environment, the profiles and
    // a profile's page, Your hosts, Your level), this machine's own (Network's folders), or writes nothing
    // (a link, a Notice's Dismiss). A service on this Mac's port and its scope are this
    // machine's too (#1970). A control that writes the
    // committed file, or one this list does not name yet, fails here.
    expect(on).toEqual([
      "Harness & profiles: Default profile",
      "Harness & profiles: Environment passed to chats",
      "Harness & profiles: Open work",
      "Harness & profiles: Remove work",
      "Harness & profiles: Add profile",
      "work: Rename work",
      "work: Remove work",
      "work: work: kind",
      "work: Reset",
      "work: work: command",
      "work: Reset",
      "work: work: environment",
      "Your hosts: Add host",
      "Your level: This project",
      // This machine's folders, the Notice saying the mock core answered nothing, and the link
      // to the Dispatch page (#1759).
      "Network: Folders a block's Allow may name",
      "Network: Dismiss",
      "Network: A service on this Mac",
      "Network: Allowed for",
      "Network: Open that page",
      // Where this machine runs the project's chats, kept in its own store (#4536).
      "Runs on: This machineChats run here, in this machine's sandbox.",
      "Runs on: Add a pool…",
      "Extensions: Go to Extensions…",
      // Settings › Theme writes this machine alone (#2331); with no gallery from the mock core
      // it has no card and its buttons are off.
    ]);
  });

  it("leaves You as it is: this machine's settings are still written", async () => {
    core();
    render(
      <ReadOnlySaid.Provider value={SENTENCE}>
        <Tab at="you" />
      </ReadOnlySaid.Provider>,
    );
    await within(nav()).findAllByRole("button");
    expect(document.querySelector("fieldset.read-only-off")).toBeNull();
  });
});
