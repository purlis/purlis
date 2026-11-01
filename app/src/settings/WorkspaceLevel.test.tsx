// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsTab } from "./SettingsTab";
import { forgetGroups } from "./links";
import type { Level } from "./groups";
import { ViewPane } from "../Views";
import { settingsView, workspaceSettingsTitle, workspaceSettingsView } from "../tabs";
import type {
  ProjectExtension,
  ProjectTheme,
  SettingsChange,
  SettingsEdit,
  WorkspaceSettings as Settings,
  WorkspaceSettingsSaved,
} from "../bindings";
import { BUILT_IN } from "../theme/theme";
import { retype, textOf } from "../editor/typing.testkit";
import { offeredIn, pickIn } from "../ui/select/select.testkit";
import { PURLIS_LIGHT } from "../theme/handed.testkit";

/** Two custom colours, as `#rrggbb`: taken from the built-in themes, because no colour is written
 *  outside `src/theme/` — a test's included (`literals.test.ts`). */
const PICKED = PURLIS_LIGHT.values["accent.base"];
const HELD = BUILT_IN["purlis-dark"].values["accent.base"];

/**
 * **The Settings tab at the Workspace level** (SE-20, #1170; the spec on #558, V89b, V89e,
 * V89h): one workspace's settings — Live · Repos · Extensions · Appearance · Plugins — each file
 * setting written as its own key through the core as it changes, with Undo and a refused write
 * said beside it, as the Project level's are. It replaces the Workspace settings page
 * (charter-app#280), and everything that page reached is held here: what is in force and which
 * layer decided it, the theme and the colour, the harness plugins, LIVE and LOCAL, and the
 * repos. What the core keeps and refuses is `purlis_core::settings::workspace`'s tests.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  // Which group each level shows is the window's, for as long as it runs (SE-22).
  forgetGroups();
});

const PLANE = "/home/dev/plane";

const MANIFEST =
  '{\n  "name": "alpha",\n  "settings": {"extensions": {"stats": {"enabled": false}}}\n}\n';

const ALPHA: Settings = {
  workspace: "alpha",
  file: "workspaces/alpha/workspace.json",
  exists: true,
  text: MANIFEST,
  refusals: [],
  parsed: true,
  fields: [
    {
      path: [{ key: "extensions" }, { key: "stats" }, { key: "enabled" }],
      value: { kind: "bool", value: false },
    },
  ],
  live: true,
};

const EXTENSIONS: ProjectExtension[] = [
  {
    id: "stats",
    name: "Persona statistics",
    state: "off",
    source: "workspace",
    settings: [],
    ignored: [
      {
        file: "workspaces/alpha/workspace.json",
        why: "workspaces/alpha/workspace.json sets extensions.stats.settings.nope, which stats does not declare — purlis hands it nothing",
      },
      {
        file: "charter.local.toml",
        why: "a Local sentence that is not this section's",
      },
    ],
  },
  { id: "solarized", name: "Solarized", state: "on", source: "local", settings: [], ignored: [] },
];

/** What `project_theme` answers in a workspace that picks nothing and has no colour. */
const NO_THEME: ProjectTheme = {
  options: [
    { value: "purlis-dark", label: "purlis-dark (built in)" },
    { value: "purlis-light", label: "purlis-light (built in)" },
    { value: "system", label: "Follow the system" },
  ],
  picked: null,
  file: null,
  draws: null,
  why: null,
  colour: null,
  ignored: [],
  local_left_out: null,
};

/** A manifest as the core would answer it after `edits`: each edited key set or gone, and a
 *  text that differs, so the next write is checked against the new one. */
function applied(file: Settings, edits: readonly SettingsEdit[]): Settings {
  let fields = [...file.fields];
  for (const edit of edits) {
    const at = JSON.stringify(edit.path);
    fields = fields.filter((one) => JSON.stringify(one.path) !== at);
    if (edit.value !== null) fields.push({ path: edit.path, value: edit.value });
  }
  return { ...file, exists: true, fields, text: `${file.text} ` };
}

function core(
  settings: Settings = ALPHA,
  saved?: (sent: Record<string, unknown>) => WorkspaceSettingsSaved,
  theme: ProjectTheme = NO_THEME,
  leftOut: string | null = null,
  {
    extensions = EXTENSIONS,
    // `drop_repo` takes the clone, and the manifest — a hand's — keeps its row (#1228).
    dropsClean = false,
    iconThemes = [{ extension: "solarized", name: "Seti", text: "{}" }],
    iconsDrawn = null as string | null,
    // The pools this machine keeps (#2046): none, and nothing about pools shows.
    pools = [] as string[],
  } = {},
) {
  let file = settings;
  /** The pool alpha's chats run on, as this machine's store says it (#2046). */
  let runsOn: string | null = null;
  /** What alpha's sends to a runner also leave out (#2046). */
  let leaveOut: string[] = ["secrets/"];
  /** What `alpha` names and this machine has not cloned (#1228). */
  const absent = ["web"];
  const cloned = ["api"];
  const sent: Record<string, unknown>[] = [];
  const asked: [string, Record<string, unknown>][] = [];
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      asked.push([cmd, given]);
      if (cmd === "workspace_settings") return file;
      if (cmd === "docker_pools")
        return {
          pools: pools.map((name) => ({
            name,
            image: "ghcr.io/purlis/purlis-runner@sha256:0123",
            limits: { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 },
            suspend_after: "24h",
          })),
          defaults: { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 },
          suspend_after: "24h",
        };
      if (cmd === "chats_run_on")
        return { pool: runsOn, here: runsOn === null, project: null, pools };
      if (cmd === "send_leaves_out") return leaveOut;
      if (cmd === "set_send_leaves_out") {
        leaveOut = given.entries as string[];
        return leaveOut;
      }
      if (cmd === "set_chats_run_on") {
        runsOn = (given.pool as string | null) ?? null;
        return { pool: runsOn, here: runsOn === null, project: null, pools };
      }
      // The project's two files, empty, for a tab that is at the Project level.
      if (cmd === "project_settings")
        return Object.fromEntries(
          (["shared", "local"] as const).map((which) => [
            which,
            {
              which,
              file: which === "shared" ? "charter.toml" : "charter.local.toml",
              exists: false,
              text: "",
              refusals: [],
              parsed: true,
              fields: [],
            },
          ]),
        );
      if (cmd === "project_extensions") return { extensions, local_left_out: leftOut };
      if (cmd === "extensions_on") return [];
      if (cmd === "extension_icon_themes") return iconThemes;
      if (cmd === "project_icons_drawn") return iconsDrawn;
      if (cmd === "project_theme") return { ...theme, local_left_out: leftOut };
      if (cmd === "project_theme_drawn") return theme.draws;
      // Settings › Theme's cards (#2331): Purlis Classic and the project's folder, which the
      // project commits as its default.
      if (cmd === "theme_gallery")
        return {
          cards: [
            { id: "purlis-classic", name: "Purlis Classic", from: "built-in" },
            { id: "brand", name: "Brand", from: "project" },
          ].map((one) => ({
            ...one,
            key: one.from === "project" ? `project:${one.id}` : one.id,
            level: one.from === "project" ? "project" : null,
            preview: false,
            extension: null,
            appearances: [{ id: "dark", appearance: "dark", high_contrast: false }],
            options: [],
            refused: false,
            said: [],
            warnings: [],
            floors: [],
            follows: null,
            duplicable: one.from === "built-in",
            folder: one.from === "project",
            stamp: "0",
          })),
          yours: null,
          drawn: "project:brand",
          chosen: "project:brand",
          options: [],
          level: "project-default",
          from: "charter.toml",
        };
      if (cmd === "reachable_repos")
        return {
          repos: [
            { name: "api", path: "acme/api", description: "" },
            { name: "web", path: "acme/web", description: "" },
          ],
          trouble: [],
        };
      if (cmd === "workspace_repos")
        return {
          workspace: given.workspace,
          repos: cloned.map((name) => ({ name })),
          cache_refused: null,
        };
      if (cmd === "workspace_panels")
        return {
          workspace: given.workspace,
          repos: [...cloned],
          paths: {},
          absent: [...absent],
          refused: [],
          todos: [],
          todos_refused: null,
          personas: [],
          persona: null,
          sessions: [],
          contributed: [],
        };
      if (cmd === "drop_repo_membership") {
        absent.splice(absent.indexOf(String(given.repo)), 1);
        return [`Removed '${String(given.repo)}' from workspace 'alpha'.`];
      }
      if (cmd === "drop_repo" && dropsClean) {
        const repo = String(given.repo);
        cloned.splice(cloned.indexOf(repo), 1);
        absent.push(repo);
        return [`Removed '${repo}' from workspace 'alpha'.`];
      }
      if (cmd === "drop_repo")
        throw {
          said: "Refusing to remove 'api' — this would discard work: api: uncommitted changes. Push or commit first.",
          at_risk: [{ what: "api", said: "api: uncommitted changes" }],
        };
      if (cmd === "workspace_live_preview")
        return { live: file.live, files: [], remote: null, mode: "push" };
      if (cmd === "save_workspace_settings") {
        sent.push(given);
        const answer = saved?.(given);
        if (answer) return answer;
        const change = given.change as SettingsChange;
        file =
          change.kind === "raw"
            ? { ...file, exists: true, text: change.text }
            : applied(file, change.edits);
        return { kind: "saved", settings: file };
      }
      return undefined;
    },
    { shouldMockEvents: true },
  );
  return {
    sent,
    asked: (cmd: string) => asked.filter(([one]) => one === cmd).map(([, given]) => given),
  };
}

/** The edits a form's write sent (`save_workspace_settings`' change). */
const editsOf = (sent: Record<string, unknown>) =>
  (sent.change as SettingsChange & { edits?: SettingsEdit[] }).edits;

/** The tab as a view tab holds it, about `alpha`: its level is the tab's. */
function Tab({ at = "workspace" }: { at?: Level }) {
  const [level, setLevel] = useState<Level>(at);
  return <SettingsTab plane={PLANE} workspace="alpha" level={level} onLevelChange={setLevel} />;
}

const nav = () => screen.getByRole("navigation", { name: "Groups" });
const groups = () =>
  within(nav())
    .getAllByRole("button")
    .map((one) => one.textContent);
const open = (name: string) => userEvent.click(within(nav()).getByRole("button", { name }));
const shown = () => screen.getByRole("region", { name: /./ });

/** The tab at alpha's level, read, showing `group`. */
async function at(group = "Live") {
  render(<Tab />);
  await within(await screen.findByRole("navigation", { name: "Groups" })).findByRole("button", {
    name: "Live",
  });
  if (group !== "Live") await open(group);
  return shown();
}

describe("the Workspace level", () => {
  it("is offered beside You and Project for a tab about a workspace, and not otherwise", async () => {
    core();
    render(<Tab at="you" />);

    expect(screen.getByRole("radio", { name: "Workspace" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "Workspace" }));
    expect(screen.getByRole("radio", { name: "Workspace" })).toBeChecked();
    expect(await within(nav()).findByRole("button", { name: "Live" })).toBeInTheDocument();

    cleanup();
    render(<SettingsTab plane={PLANE} level="you" />);
    expect(screen.queryByRole("radio", { name: "Workspace" })).toBeNull();
  });

  it("lists the five groups in order for that workspace", async () => {
    const { asked } = core();
    await at();

    await waitFor(() =>
      expect(groups()).toEqual(["Live", "Repos", "Dispatch", "Extensions", "Theme", "Appearance"]),
    );
    expect(asked("workspace_settings")).toContainEqual({ plane: PLANE, workspace: "alpha" });
    expect(asked("project_extensions")).toContainEqual({ plane: PLANE, workspace: "alpha" });
  });

  it("offers where its chats run only once this machine keeps a pool, and sets it on this machine (#2046)", async () => {
    const { asked } = core(ALPHA, undefined, NO_THEME, null, { pools: ["builds"] });
    await at();
    await waitFor(() => expect(groups()).toContain("Runs on"));
    expect(groups().indexOf("Runs on")).toBe(groups().indexOf("Repos") + 1);
    await open("Runs on");
    const runs = screen.getByRole("region", { name: "Runs on" });
    expect(runs).toHaveTextContent("Kept on this machine only, never in a committed file.");
    const here = await within(runs).findByRole("radio", { name: /This machine/ });
    await waitFor(() => expect(here).toBeChecked());

    await userEvent.click(within(runs).getByRole("radio", { name: /Pool builds/ }));

    await waitFor(() =>
      expect(within(runs).getByRole("radio", { name: /Pool builds/ })).toBeChecked(),
    );
    expect(asked("set_chats_run_on")).toEqual([
      { plane: PLANE, workspace: "alpha", pool: "builds" },
    ]);
  });

  it("lets the person name more to leave out of what a runner gets, once a pool is kept (#2046)", async () => {
    const { asked } = core(ALPHA, undefined, NO_THEME, null, { pools: ["builds"] });
    await at();
    await waitFor(() => expect(groups()).toContain("Sent to runners"));
    expect(groups().indexOf("Sent to runners")).toBe(groups().indexOf("Runs on") + 1);
    await open("Sent to runners");
    const sends = screen.getByRole("region", { name: "Sent to runners" });
    expect(sends).toHaveTextContent("Kept in the project's settings file, which your team sees.");
    const box = await within(sends).findByRole("textbox", { name: /Also leave out/ });
    await waitFor(() => expect(box).toHaveValue("secrets/"));

    await userEvent.type(box, "\nconfig/local.yml\n");
    await userEvent.tab();

    await waitFor(() =>
      expect(asked("set_send_leaves_out")).toEqual([
        { plane: PLANE, workspace: "alpha", entries: ["secrets/", "config/local.yml"] },
      ]),
    );
  });

  it("hides a group with nothing in it, but Extensions, which links to where one is installed", async () => {
    core(ALPHA, undefined, NO_THEME, null, { extensions: [] });
    await at();

    // Extensions is offered for its link to the Extensions dialog (#1387), and says why it has
    // nothing (#1198).
    await waitFor(() =>
      expect(groups()).toEqual(["Live", "Repos", "Dispatch", "Extensions", "Theme", "Appearance"]),
    );
    await userEvent.click(
      within(screen.getByRole("navigation", { name: "Groups" })).getByRole("button", {
        name: "Extensions",
      }),
    );
    const extensions = screen.getByRole("region", { name: "Extensions" });
    expect(extensions).toHaveTextContent(
      "No extension is installed on this machine or named by this workspace.",
    );
    expect(within(extensions).getByRole("button", { name: "Go to Extensions…" })).toBeVisible();
  });

  it("says where it sits between the project's two files, and that no secret goes in it", async () => {
    core();
    await at();

    expect(
      screen.getByText(/read between its project's shared settings and this machine's own/),
    ).toHaveTextContent("vault:<vault>/<key>");
  });

  it("says a setting of a LIVE workspace is committed, and of a LOCAL one stays here, once in its group's header", async () => {
    core();
    await at("Appearance");
    await screen.findByLabelText("Theme");
    const appearance = () => screen.getByRole("region", { name: "Appearance" });
    expect(
      within(appearance()).getAllByText(
        /Kept in workspaces\/alpha\/workspace\.json, committed with this LIVE workspace; your team sees it\./,
      ),
    ).toHaveLength(1);
    // Never under the setting itself (DS-8): its row says what the value does, and its badge
    // where it comes from only when that is somewhere else.
    expect(screen.getByLabelText("Theme")).not.toHaveAccessibleDescription(/Kept in/);

    cleanup();
    core({ ...ALPHA, live: false });
    await at("Appearance");
    await screen.findByLabelText("Theme");
    expect(appearance()).toHaveTextContent(/stays on this machine while the workspace is not LIVE/);
  });

  it("says what purlis does not take from the settings as they stand", async () => {
    core({
      ...ALPHA,
      refusals: [
        { why: "settings.colour in workspaces/alpha/workspace.json is not read", key: ["colour"] },
      ],
    });
    await at();

    expect(
      screen.getByText("settings.colour in workspaces/alpha/workspace.json is not read"),
    ).toBeInTheDocument();
  });
});

describe("every workspace setting there is, at the Workspace level", () => {
  it.each([
    ["Live", ["Published with the project"]],
    ["Repos", ["Cloned here", "Not cloned here"]],
    ["Dispatch", ["Limits"]],
    ["Extensions", ["Persona statistics: enabled", "Solarized: enabled"]],
    ["Theme", ["Themes", "Appearance", "Options", "Theme check", "This theme"]],
    ["Appearance", ["Theme", "Colour", "Icons"]],
  ])("%s holds %j", async (name, labels) => {
    core();
    const group = await at(name);

    await waitFor(() =>
      expect(
        [...group.querySelectorAll(".ui-setting-label")].map((one) => one.textContent),
      ).toEqual(labels),
    );
  });

  it("keeps your theme for the workspace on this machine, from its card, and says which level draws (#2323, #2331)", async () => {
    const { asked, sent } = core();
    const group = await at("Theme");

    const brand = await within(group).findByRole("option", { name: /^Brand/ });
    expect(group).toHaveTextContent("Drawn here: Brand");
    await userEvent.click(brand);

    await waitFor(() =>
      expect(asked("theme_choose")).toEqual([
        { plane: PLANE, workspace: "alpha", theme: "brand", appearance: null, from: "project" },
      ]),
    );
    // Never a project file.
    expect(sent).toHaveLength(0);
  });

  it("writes the icons a workspace's file trees draw once picked, as settings.theme.icons", async () => {
    const { sent } = core();
    const group = await at("Appearance");

    const pick = within(group).getByLabelText("Icons");
    await waitFor(async () =>
      expect(await offeredIn(pick)).toEqual([
        "not set — the project's pick",
        "purlis-icons (built in)",
        "Seti (Solarized)",
      ]),
    );
    await pickIn(pick, "solarized/Seti");

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(editsOf(sent[0])).toEqual([
      {
        path: [{ key: "theme" }, { key: "icons" }],
        value: { kind: "text", value: "solarized/Seti" },
      },
    ]);
  });

  it("says why the icons this workspace picks are not drawn here", async () => {
    core({
      ...ALPHA,
      fields: [
        ...ALPHA.fields,
        {
          path: [{ key: "theme" }, { key: "icons" }],
          value: { kind: "text", value: "stats/Bars" },
        },
      ],
    });
    const group = await at("Appearance");

    expect(within(group).getByLabelText("Icons")).toHaveValue("stats/Bars");
    await waitFor(() =>
      expect(group).toHaveTextContent(
        "workspaces/alpha/workspace.json picks “Bars” from stats, but stats is off in this workspace — so the built-in purlis-icons is drawn",
      ),
    );
  });

  it("says nothing of its pick while charter.local.toml's wins over it", async () => {
    const picksBars: Settings = {
      ...ALPHA,
      fields: [
        ...ALPHA.fields,
        {
          path: [{ key: "theme" }, { key: "icons" }],
          value: { kind: "text", value: "stats/Bars" },
        },
      ],
    };
    const { asked } = core(picksBars, undefined, NO_THEME, null, {
      iconsDrawn: "solarized/Seti",
    });
    const group = await at("Appearance");

    await waitFor(() => expect(asked("project_icons_drawn").length).toBeGreaterThan(0));
    await waitFor(() => expect(within(group).getByLabelText("Icons")).toHaveValue("stats/Bars"));
    expect(group).not.toHaveTextContent("picks “Bars”");
  });
});

describe("Live", () => {
  it("names the switch's group by its row", async () => {
    core();
    const group = await at("Live");

    expect(
      within(group).getByRole("group", { name: "Published with the project" }),
    ).toContainElement(within(group).getByRole("button", { name: "Make local…" }));
  });

  it("offers the same switch as the workspace's menu, through the same confirmation", async () => {
    core({ ...ALPHA, live: false });
    const group = await at("Live");

    expect(group).toHaveTextContent("LOCAL: its charter, memory and todos stay on this machine.");
    await userEvent.click(within(group).getByRole("button", { name: "Make live…" }));

    expect(await screen.findByRole("alertdialog", { name: "Make alpha live?" })).toBeTruthy();
  });

  it("offers to make a LIVE workspace local", async () => {
    core();
    const group = await at("Live");

    expect(group).toHaveTextContent(
      "LIVE: its charter, memory and todos are published with the project.",
    );
    expect(within(group).getByRole("button", { name: "Make local…" })).toBeInTheDocument();
  });
});

describe("Repos", () => {
  it("ticks what is cloned here, and says the core's refusal to remove one holding work beside it", async () => {
    const { asked } = core();
    const group = await at("Repos");

    const api = await within(group).findByRole("checkbox", { name: "api" });
    await waitFor(() => expect(api).toBeChecked());
    await userEvent.click(api);
    await userEvent.click(within(group).getByRole("button", { name: "Remove 1" }));

    expect(await within(group).findByRole("alert")).toHaveTextContent("api: uncommitted changes");
    expect(asked("drop_repo")).toEqual([{ plane: PLANE, workspace: "alpha", repo: "api" }]);
  });
});

describe("Repos not cloned here", () => {
  it("offers to remove one from the workspace, and asks first", async () => {
    const { asked } = core();
    const group = await at("Repos");

    const named = await within(group).findByRole("button", {
      name: "Remove web from workspace…",
    });
    await userEvent.click(named);
    const asking = await screen.findByRole("alertdialog", { name: "Remove web from alpha?" });
    expect(asked("drop_repo_membership")).toEqual([]);
    await userEvent.click(within(asking).getByRole("button", { name: "Remove from workspace" }));

    await waitFor(() =>
      expect(
        within(group).queryByRole("button", { name: "Remove web from workspace…" }),
      ).toBeNull(),
    );
    expect(asked("drop_repo_membership")).toEqual([
      { plane: PLANE, workspace: "alpha", repo: "web" },
    ]);
    expect(asked("drop_repo")).toEqual([]);
  });

  it("lists a clone unticked from a hand's manifest as soon as the picker applies", async () => {
    core(ALPHA, undefined, NO_THEME, null, { dropsClean: true });
    const group = await at("Repos");

    const api = await within(group).findByRole("checkbox", { name: "api" });
    await waitFor(() => expect(api).toBeChecked());
    await userEvent.click(api);
    await userEvent.click(within(group).getByRole("button", { name: "Remove 1" }));

    expect(
      await within(group).findByRole("button", { name: "Remove api from workspace…" }),
    ).toHaveAttribute("data-variant", "danger");
  });

  it("removes nothing when the question is cancelled", async () => {
    const { asked } = core();
    const group = await at("Repos");

    await userEvent.click(
      await within(group).findByRole("button", { name: "Remove web from workspace…" }),
    );
    const asking = await screen.findByRole("alertdialog", { name: "Remove web from alpha?" });
    await userEvent.click(within(asking).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(asked("drop_repo_membership")).toEqual([]);
    expect(
      within(group).getByRole("button", { name: "Remove web from workspace…" }),
    ).toBeInTheDocument();
  });
});

describe("Extensions", () => {
  it("says what is in force in this workspace, and which layer decided it", async () => {
    core();
    const group = await at("Extensions");

    await waitFor(() =>
      expect(group).toHaveTextContent("Persona statistics: off — turned off in this workspace"),
    );
    expect(group).toHaveTextContent("Solarized: on — enabled in charter.local.toml");
    // Switches at what is in force (#1865): this file's off, and the project's on beneath.
    expect(
      within(group).getByRole("switch", { name: "Persona statistics: enabled" }),
    ).not.toBeChecked();
    expect(within(group).getByRole("switch", { name: "Solarized: enabled" })).toBeChecked();
    // Only this file's sentences.
    expect(group).toHaveTextContent("which stats does not declare");
    expect(group).not.toHaveTextContent("a Local sentence");
  });

  it("writes a Reset as that one key, against the manifest it was read as, at once", async () => {
    const { sent, asked } = core();
    const group = await at("Extensions");
    const row = within(group)
      .getByRole("switch", { name: "Persona statistics: enabled" })
      .closest(".ui-setting-row") as HTMLElement;

    await userEvent.click(within(row).getByRole("button", { name: "Reset" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      plane: PLANE,
      workspace: "alpha",
      base: MANIFEST,
      change: {
        kind: "edits",
        edits: [
          { path: [{ key: "extensions" }, { key: "stats" }, { key: "enabled" }], value: null },
        ],
      },
    });
    await waitFor(() => expect(asked("project_extensions").length).toBeGreaterThan(1));
  });

  it("sends a manifest that is not there yet as new", async () => {
    const { sent } = core({ ...ALPHA, exists: false, text: "", fields: [] });
    const group = await at("Extensions");

    await userEvent.click(within(group).getByRole("switch", { name: "Solarized: enabled" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0].base).toBeNull();
  });

  it("undoes the last change by writing the value it had back", async () => {
    const { sent } = core();
    const group = await at("Extensions");
    await userEvent.click(
      within(group).getByRole("switch", { name: "Persona statistics: enabled" }),
    );

    await userEvent.click(await within(group).findByRole("button", { name: "Undo" }));

    await waitFor(() => expect(sent).toHaveLength(2));
    expect(editsOf(sent[1])).toEqual([
      {
        path: [{ key: "extensions" }, { key: "stats" }, { key: "enabled" }],
        value: { kind: "bool", value: false },
      },
    ]);
    await waitFor(() =>
      expect(
        within(group).getByRole("switch", { name: "Persona statistics: enabled" }),
      ).not.toBeChecked(),
    );
  });

  it("says the value comes from the workspace's file, and its reset takes it out (SE-18)", async () => {
    const { sent } = core();
    const group = await at("Extensions");
    const box = () => within(group).getByLabelText("Persona statistics: enabled");
    expect(box()).toHaveAccessibleDescription(
      /From workspaces\/alpha\/workspace\.json, at the Workspace level\./,
    );
    const row = box().closest(".ui-setting-row") as HTMLElement;
    expect(within(row).queryByRole("radio")).toBeNull();

    await userEvent.click(within(row).getByRole("button", { name: "Reset" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(editsOf(sent[0])).toEqual([
      { path: [{ key: "extensions" }, { key: "stats" }, { key: "enabled" }], value: null },
    ]);
  });

  it("says a refused write beside its setting, in the core's own words", async () => {
    core(ALPHA, () => ({
      kind: "refused",
      reasons: ["workspaces/alpha/workspace.json changed on disk since this tab read it"],
    }));
    const group = await at("Extensions");

    await userEvent.click(within(group).getByRole("switch", { name: "Solarized: enabled" }));

    expect(await within(group).findByRole("alert")).toHaveTextContent(
      "workspaces/alpha/workspace.json changed on disk since this tab read it",
    );
    // Back at what is in force: not set here, so the project's on beneath.
    expect(within(group).getByRole("switch", { name: "Solarized: enabled" })).toBeChecked();
  });
});

describe("Appearance (charter-app#281)", () => {
  const PICKS_LIGHT: ProjectTheme = {
    ...NO_THEME,
    picked: "purlis-light",
    file: "workspaces/alpha/workspace.json",
    draws: "purlis-light",
    colour: "teal",
  };
  const SET: Settings = {
    ...ALPHA,
    fields: [
      ...ALPHA.fields,
      { path: [{ key: "theme" }, { key: "use" }], value: { kind: "text", value: "purlis-light" } },
      { path: [{ key: "theme" }, { key: "colour" }], value: { kind: "text", value: "teal" } },
    ],
  };

  it("asks the theme in this workspace, and says what is drawn here and which file picked it", async () => {
    const { asked } = core(SET, undefined, PICKS_LIGHT);
    const group = await at("Appearance");

    const pick = await within(group).findByLabelText("Theme");
    expect(pick).toHaveValue("purlis-light");
    expect((await offeredIn(pick))[0]).toBe("not set — the project's pick");
    await waitFor(() =>
      expect(pick).toHaveAccessibleDescription(
        /^Drawn in this workspace: purlis-light \(built in\), picked in workspaces\/alpha\/workspace\.json\. The terminal follows the window\./,
      ),
    );
    for (const one of asked("project_theme"))
      expect(one).toEqual({ plane: PLANE, workspace: "alpha" });
  });

  it("says when the project's file picked the theme drawn here", async () => {
    core(ALPHA, undefined, {
      ...NO_THEME,
      picked: "system",
      file: "charter.toml",
      draws: "system",
    });
    const group = await at("Appearance");

    await waitFor(() =>
      expect(within(group).getByLabelText("Theme")).toHaveAccessibleDescription(
        /^Drawn in this workspace: Follow the system, picked in charter\.toml\./,
      ),
    );
  });

  it("says why the project's pick falls back here", async () => {
    const why =
      "charter.toml picks “Solarized Dark” from solarized, but solarized is off in this workspace — so the built-in theme, Purlis, is drawn";
    core(ALPHA, undefined, {
      ...NO_THEME,
      picked: "solarized/Solarized Dark",
      file: "charter.toml",
      draws: "purlis-dark",
      why,
    });
    const group = await at("Appearance");

    await waitFor(() => expect(group).toHaveTextContent(why));
    expect(within(group).getByLabelText("Theme")).toHaveAccessibleDescription(
      /^Drawn in this workspace: purlis-dark \(built in\)\. The terminal follows the window\./,
    );
  });

  it("says a grey colour tints nothing", async () => {
    const grey = BUILT_IN["purlis-dark"].values["text.muted"];
    core(
      {
        ...ALPHA,
        fields: [
          { path: [{ key: "theme" }, { key: "colour" }], value: { kind: "text", value: grey } },
        ],
      },
      undefined,
      { ...NO_THEME, colour: grey },
    );
    const group = await at("Appearance");

    await waitFor(() =>
      expect(group).toHaveTextContent(
        `${grey} is a grey, which has no hue to tint with — so this workspace is drawn without a colour.`,
      ),
    );
  });

  it("shows a value that is neither a name nor #rrggbb as held, not as a custom colour", async () => {
    const short = BUILT_IN["purlis-dark"].values["text.muted"].slice(0, 4);
    core({
      ...ALPHA,
      fields: [
        { path: [{ key: "theme" }, { key: "colour" }], value: { kind: "text", value: short } },
      ],
    });
    const group = await at("Appearance");

    expect(await within(group).findByLabelText("Colour")).toHaveValue(short);
    expect(within(group).queryByLabelText("Custom colour")).toBeNull();
  });

  it("offers the eight colours and a custom one, and says what a colour tints", async () => {
    core(SET, undefined, PICKS_LIGHT);
    const group = await at("Appearance");

    const colour = await within(group).findByLabelText("Colour");
    expect(colour).toHaveValue("teal");
    expect(await offeredIn(colour)).toEqual([
      "none — the theme as it is",
      "Red",
      "Orange",
      "Yellow",
      "Green",
      "Teal",
      "Blue",
      "Purple",
      "Pink",
      "Custom…",
    ]);
    expect(colour).toHaveAccessibleDescription(
      /^Tints this workspace's tab, its chat strip, the title bar's mark and the accent while it is in front, as the theme says\. The terminal keeps the theme's own colours\./,
    );
    expect(within(group).queryByLabelText("Custom colour")).toBeNull();
  });

  it("writes a colour under settings.theme at once, and a custom one as the #rrggbb picked", async () => {
    const { sent } = core();
    const group = await at("Appearance");

    await pickIn(await within(group).findByLabelText("Colour"), "purple");
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(editsOf(sent[0])).toEqual([
      { path: [{ key: "theme" }, { key: "colour" }], value: { kind: "text", value: "purple" } },
    ]);

    await pickIn(within(group).getByLabelText("Colour"), "custom");
    await waitFor(() => expect(sent).toHaveLength(2));
    const custom = await within(group).findByLabelText("Custom colour");
    // A drag is every step of `input`; only the pick, `change`, is written.
    fireEvent.input(custom, { target: { value: PICKED } });
    await act(() => new Promise((settle) => setTimeout(settle, 20)));
    expect(sent).toHaveLength(2);
    fireEvent.change(custom, { target: { value: PICKED } });
    await waitFor(() => expect(sent).toHaveLength(3));
    expect(editsOf(sent[2])).toEqual([
      { path: [{ key: "theme" }, { key: "colour" }], value: { kind: "text", value: PICKED } },
    ]);
  });

  it("writes a colour dragged in the well once, when the well is left", async () => {
    const { sent } = core(
      {
        ...ALPHA,
        fields: [
          { path: [{ key: "theme" }, { key: "colour" }], value: { kind: "text", value: HELD } },
        ],
      },
      undefined,
      { ...NO_THEME, colour: HELD },
    );
    const group = await at("Appearance");
    const custom = await within(group).findByLabelText("Custom colour");

    fireEvent.input(custom, { target: { value: PICKED } });
    fireEvent.blur(custom);

    await waitFor(() => expect(sent).toHaveLength(1));
    await act(() => new Promise((settle) => setTimeout(settle, 20)));
    expect(sent).toHaveLength(1);
    expect(editsOf(sent[0])).toEqual([
      { path: [{ key: "theme" }, { key: "colour" }], value: { kind: "text", value: PICKED } },
    ]);
  });

  it("shows a custom colour the file holds as custom, with its value", async () => {
    core(
      {
        ...ALPHA,
        fields: [
          { path: [{ key: "theme" }, { key: "colour" }], value: { kind: "text", value: HELD } },
        ],
      },
      undefined,
      { ...NO_THEME, colour: HELD },
    );
    const group = await at("Appearance");

    expect(await within(group).findByLabelText("Colour")).toHaveValue("custom");
    expect(within(group).getByLabelText("Custom colour")).toHaveValue(HELD);
  });

  it("says why a pick or a colour in this file is not used, and only this file's", async () => {
    core(SET, undefined, {
      ...NO_THEME,
      ignored: [
        {
          file: "workspaces/alpha/workspace.json",
          why: "workspaces/alpha/workspace.json sets settings.theme.colour to 3, which is not a colour",
        },
        { file: "charter.toml", why: "a Shared sentence that is not this section's" },
      ],
    });
    const group = await at("Appearance");

    await waitFor(() => expect(group).toHaveTextContent("settings.theme.colour to 3"));
    expect(group).not.toHaveTextContent("a Shared sentence");
  });

  it("has the window ask this workspace's theme again after a write", async () => {
    const { asked } = core(SET, undefined, PICKS_LIGHT);
    const group = await at("Appearance");
    await waitFor(() => expect(asked("project_theme_drawn")).toHaveLength(1));

    await pickIn(within(group).getByLabelText("Colour"), "red");

    await waitFor(() => expect(asked("project_theme_drawn")).toHaveLength(2));
    expect(asked("project_theme_drawn")[1]).toEqual({ plane: PLANE, workspace: "alpha" });
  });
});

/** The ignore check's sentence for a `charter.local.toml` git would commit, as the core says it
 *  (charter-app#308). */
const LEFT_OUT =
  "git would commit charter.local.toml, so purlis reads nothing in it until it is ignored — purlis doctor --fix local-ignore adds /charter.local.toml to .gitignore.";

describe("a charter.local.toml git would carry (charter-app#319)", () => {
  it("is said in each group that shows what is in force, once per old group", async () => {
    core(ALPHA, undefined, NO_THEME, LEFT_OUT);
    for (const [name, times] of [
      ["Extensions", 1],
      ["Appearance", 1],
    ] as const) {
      const group = await at(name);
      await waitFor(() => expect(within(group).getAllByText(LEFT_OUT)).toHaveLength(times));
      cleanup();
    }
  });

  it("is not said while charter reads the file", async () => {
    core();
    await at("Extensions");

    expect(screen.queryByText(/charter reads nothing in it/)).toBeNull();
  });
});

describe("Settings at a workspace's level, as a view", () => {
  it("is drawn in a view pane for its workspace, asking open_view nothing", async () => {
    const { asked } = core();
    render(
      <ViewPane
        plane={PLANE}
        view={workspaceSettingsView("alpha")}
        title={workspaceSettingsTitle("alpha")}
        workspace="alpha"
        waits={false}
        offered={[]}
        onOpenView={() => undefined}
        onAsk={() => undefined}
        onVaultChanged={() => undefined}
      />,
    );

    expect(await screen.findByRole("region", { name: "Settings · alpha" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Workspace" })).toBeChecked();
    expect(await within(nav()).findByRole("button", { name: "Live" })).toBeInTheDocument();
    expect(asked("open_view")).toEqual([]);
  });

  it("moves the pane it is in to the Project level", async () => {
    core();
    const moved: [string, string][] = [];
    render(
      <ViewPane
        plane={PLANE}
        view={workspaceSettingsView("alpha")}
        title={workspaceSettingsTitle("alpha")}
        workspace="alpha"
        waits={false}
        offered={[]}
        onOpenView={() => undefined}
        onAsk={() => undefined}
        onVaultChanged={() => undefined}
        onShowInstead={(_from, to, title) => moved.push([`${to.view}/${to.key}`, title])}
      />,
    );
    await within(await screen.findByRole("navigation", { name: "Groups" })).findByRole("button", {
      name: "Live",
    });

    await userEvent.click(screen.getByRole("radio", { name: "Project" }));

    expect(moved).toEqual([["settings/project", "Settings"]]);
  });

  it("moves a Project-level pane on a workspace's strip to that workspace's level", async () => {
    core();
    const moved: [string, string][] = [];
    render(
      <ViewPane
        plane={PLANE}
        view={settingsView("project")}
        title="Settings"
        workspace="alpha"
        waits={false}
        offered={[]}
        onOpenView={() => undefined}
        onAsk={() => undefined}
        onVaultChanged={() => undefined}
        onShowInstead={(_from, to, title) => moved.push([`${to.view}/${to.key}`, title])}
      />,
    );

    await userEvent.click(await screen.findByRole("radio", { name: "Workspace" }));

    expect(moved).toEqual([["workspace-settings/alpha", "Settings · alpha"]]);
  });
});

describe("Edit as JSON (NO-7, #1232)", () => {
  const editLink = () =>
    within(screen.getByRole("group", { name: "Edit as JSON" })).getByRole("button", {
      name: "workspace.json",
    });
  const box = () => screen.findByRole("textbox", { name: "workspace.json, as JSON" });
  /** What the manifest's text holds, once it holds `text`. */
  const holds = (text: string) => waitFor(async () => expect(textOf(await box())).toBe(text));

  it("sits at the foot of the nav and shows the whole manifest", async () => {
    core();
    await at();

    await userEvent.click(editLink());

    await holds(MANIFEST);
    expect(screen.queryByRole("group", { name: "Edit as TOML" })).toBeNull();
  });

  it("saves the whole text through the core, against the text the edit began from", async () => {
    const { sent } = core();
    await at();
    await userEvent.click(editLink());
    const typed = '{"name": "alpha", "description": "mended"}';

    retype(await box(), typed);
    await userEvent.click(screen.getByRole("button", { name: "Save workspace.json" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      plane: PLANE,
      workspace: "alpha",
      base: MANIFEST,
      change: { kind: "raw", text: typed },
    });
    await holds(typed);
  });

  it("says the core's refusal and keeps the edit", async () => {
    core(ALPHA, () => ({
      kind: "refused",
      reasons: ["workspaces/alpha/workspace.json would not be a JSON object, so nothing was saved"],
    }));
    await at();
    await userEvent.click(editLink());

    retype(await box(), "{");
    await userEvent.click(screen.getByRole("button", { name: "Save workspace.json" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "workspaces/alpha/workspace.json would not be a JSON object",
    );
    await holds("{");
  });

  it("is where a manifest that is not JSON is mended", async () => {
    core({
      ...ALPHA,
      text: '{"name": ',
      parsed: false,
      fields: [],
      refusals: [
        {
          why: "workspaces/alpha/workspace.json is not a JSON object, so purlis reads no settings from it — mend it by hand",
          key: null,
        },
      ],
    });
    await at();

    expect(
      screen.getByText("Open workspace.json under Edit as JSON to mend it."),
    ).toBeInTheDocument();
    await userEvent.click(editLink());
    await holds('{"name": ');
  });
});

describe("a standing refusal at the Workspace level (NO-7, #1232)", () => {
  it("links to the setting it is about, which is shown and focused", async () => {
    core({
      ...ALPHA,
      refusals: [
        {
          why: "settings.theme.icons in workspaces/alpha/workspace.json is 7, not an icon theme's pick",
          key: ["theme", "icons"],
        },
      ],
    });
    await at();

    await userEvent.click(screen.getByRole("button", { name: "Go to Appearance › Icons" }));

    expect(within(nav()).getByRole("button", { name: "Appearance" })).toHaveAttribute(
      "aria-current",
      "true",
    );
    await waitFor(() => expect(screen.getByLabelText("Icons")).toHaveFocus());
  });

  it("draws no link for a key no setting here holds", async () => {
    core({
      ...ALPHA,
      refusals: [
        { why: "settings.colour in workspaces/alpha/workspace.json is not read", key: ["colour"] },
      ],
    });
    await at();

    const row = screen.getByText("settings.colour in workspaces/alpha/workspace.json is not read");
    expect(within(row.closest("li") as HTMLElement).queryByRole("button")).toBeNull();
  });
});
