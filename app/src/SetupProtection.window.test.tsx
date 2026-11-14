// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type {
  ProtectionWord,
  SetupProtection,
  SettingsEdit,
  SettingsField,
  SettingsFile,
  SettingsWhich,
} from "./bindings";
import { SettingsTab } from "./settings/SettingsTab";
import { forgetGroups } from "./settings/links";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";
import { pickIn, selectNamed } from "./ui/select/select.testkit";

/**
 * **Setup's Protection row, end to end in the window** (T1-24, R-064 and R-010; Q236): the
 * page's one press sends the chosen level with the real command (`open_repo`, `create_project`),
 * and the core keeps it as the person's own level, `[sandbox] level` in this machine's file,
 * which Settings › Project › Sandbox › Your level reads and changes. The core here keeps that
 * file as `chosen_at_setup` and Settings' save do; what they write on disk is
 * `the_setup_choice_is_the_value_settings_reads`'s.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const LOCAL = "/home/dev/.local/share/purlis/local-project";
const REPO = "/home/dev/widget";
const PLANE = "/home/dev/plane";
const LEVEL_PATH = [{ key: "sandbox" }, { key: "level" }];

/** This machine's file of the project, as the fake core keeps it. */
function file(which: SettingsWhich, fields: SettingsField[] = []): SettingsFile {
  return {
    which,
    file: which === "shared" ? "purlis.toml" : "purlis.local.toml",
    exists: true,
    text: which === "shared" ? "schema = 1\n" : "",
    refusals: [],
    parsed: true,
    fields,
  };
}

/** The level the local file holds, or none. */
function levelIn(local: SettingsFile): string | undefined {
  const field = local.fields.find((one) => JSON.stringify(one.path) === JSON.stringify(LEVEL_PATH));
  return field?.value.kind === "text" ? field.value.value : undefined;
}

/** Writes `level` into the local file, as the core's setup write and Settings' save both do. */
function withLevel(local: SettingsFile, level: string): SettingsFile {
  return {
    ...local,
    fields: [
      ...local.fields.filter((one) => JSON.stringify(one.path) !== JSON.stringify(LEVEL_PATH)),
      { path: LEVEL_PATH, value: { kind: "text", value: level } },
    ],
  };
}

/** A machine with no policy and no local project yet: every level, Basic sandbox first. */
const FRESH: SetupProtection = { lowest: "off", default: "floor", why: null, yours: null };
const FOLDER = "/home/dev/Projects/garden";

/** The core: the welcome on a machine through the guided steps once, and the project's files.
 *  `setup` is what the Protection row is told (Q241). */
function core(setup: SetupProtection = FRESH) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const files: Record<SettingsWhich, SettingsFile> = {
    shared: file("shared"),
    local: file("local"),
  };
  const keep = (protection: unknown) => {
    if (typeof protection === "string") files.local = withLevel(files.local, protection);
  };
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: given });
      switch (cmd) {
        case "plane_at_launch":
          return { plane: null, from: null, why: null };
        case "recent_planes":
          return { planes: [], dropped: [], forgetful: null };
        case "git_presence":
          return { kind: "present" };
        case "onboarding_progress":
          return {
            step: "welcome",
            door: null,
            signing_in: false,
            git_missing: false,
            made: null,
            finished_once: true,
          };
        case "first_run_found":
          return { harnesses: [], forges: [], templates: [] };
        case "setup_protection":
          return setup;
        case "new_project_folder":
          return FOLDER;
        case "forge_owners":
          return {
            host: "github.com",
            login: "octocat",
            owners: [{ name: "octocat", title: "The Octocat", mine: true }],
          };
        case "create_project_on_forge":
          return new Promise(() => undefined);
        case "create_project":
          keep(given.protection);
          return {
            opened: { plane: PLANE, ask: null },
            asks_forge: null,
            protection_said: null,
          };
        case "open_repo":
          keep(given.protection);
          return {
            opened: {
              opened: { plane: LOCAL, ask: null },
              workspace: "widget",
              cwd: `${LOCAL}/workspaces/widget/widget`,
              harness: null,
              instructions: 0,
            },
            asks_forge: null,
          };
        case "plane_sidebar":
          return { root: LOCAL, personas: [], persona: null, unfiled: [], workspaces: [] };
        // Settings › Project, reading the same files.
        case "project_settings":
          return { shared: files.shared, local: files.local };
        case "project_extensions":
          return { extensions: [], local_left_out: null };
        case "sandbox_state":
          return {
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
        case "start_options":
          return {
            profiles: [],
            refused: [],
            personas: [],
            persona: null,
            persona_profiles: {},
            ignore_fix: null,
            declares_none: false,
          };
        case "plain_file_vaults":
        case "sandbox_levels_waiting":
          return [];
        case "save_project_settings": {
          const which = given.which as SettingsWhich;
          const change = given.change as { kind: "edits"; edits: SettingsEdit[] };
          for (const edit of change.edits)
            if (
              JSON.stringify(edit.path) === JSON.stringify(LEVEL_PATH) &&
              edit.value?.kind === "text"
            )
              files[which] = withLevel(files[which], edit.value.value);
          return { kind: "saved", file: files[which] };
        }
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked, files, calls: (cmd: string) => asked.filter((one) => one.cmd === cmd) };
}

beforeEach(() => {
  forgetThisLaunch();
  forgetGroups();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/dev/layout.json", found: false, document: null, trouble: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

const SAID: Record<ProtectionWord, RegExp> = {
  floor: /^Basic sandbox/,
  on: /^Full sandbox/,
  off: /^No sandbox/,
};

/** The welcome's Create a project, the Protection row at `level`, and the repo opened. */
async function setUpAt(level: ProtectionWord) {
  const person = userEvent.setup();
  await person.click(await screen.findByRole("button", { name: "Create a project" }));
  const row = await screen.findByRole("radiogroup", { name: "Protection" });
  await person.click(within(row).getByRole("radio", { name: SAID[level] }));
  await person.type(screen.getByLabelText("Or type the repo's path"), REPO);
  await person.click(screen.getByRole("button", { name: "Open repo" }));
}

describe("setup's Protection row, in the window", () => {
  it("one press finishes setup at every Protection choice (R-064)", async () => {
    for (const level of ["floor", "on", "off"] as const) {
      const { calls, files } = core();
      render(<App />);
      await setUpAt(level);
      await waitFor(() => expect(calls("open_repo")).toHaveLength(1));
      expect(calls("open_repo")[0].args).toMatchObject({ path: REPO, protection: level });
      // Setup is done: the project is in front, and the level is written.
      await waitFor(() =>
        expect(screen.queryByRole("radiogroup", { name: "Protection" })).not.toBeInTheDocument(),
      );
      expect(levelIn(files.local)).toBe(level);
      cleanup();
      clearMocks();
    }
  });

  it("the setup row and Settings › Project › Sandbox change the same level (R-010)", async () => {
    const { files } = core();
    render(<App />);
    await setUpAt("on");
    await waitFor(() => expect(levelIn(files.local)).toBe("on"));
    cleanup();

    // Settings › Project › Sandbox › Your level reads it.
    render(<SettingsTab plane={PLANE} level="project" />);
    await userEvent.click(
      await within(await screen.findByRole("navigation", { name: "Groups" })).findByRole("button", {
        name: "Your level",
      }),
    );
    const picker = await waitFor(() => selectNamed("This project"));
    await waitFor(() => expect(picker).toHaveTextContent("Full sandbox"));

    // Changed there, the same value reads back changed.
    await pickIn(picker, "off");
    await waitFor(() => expect(levelIn(files.local)).toBe("off"));
    fireEvent.blur(picker);
    await waitFor(() => expect(selectNamed("This project")).toHaveTextContent("No sandbox"));
  });

  it("First run's local create finishes in one press and writes the level", async () => {
    const { calls, files } = core();
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    const row = await screen.findByRole("radiogroup", { name: "Protection" });
    await person.click(within(row).getByRole("radio", { name: SAID.off }));
    await person.type(screen.getByLabelText("Name"), "garden");
    const create = await screen.findByRole("button", { name: "Create project" });
    await waitFor(() => expect(create).toBeEnabled());
    await person.click(create);
    await waitFor(() => expect(calls("create_project")).toHaveLength(1));
    expect(calls("create_project")[0].args).toMatchObject({ path: FOLDER, protection: "off" });
    await waitFor(() =>
      expect(screen.queryByRole("radiogroup", { name: "Protection" })).not.toBeInTheDocument(),
    );
    expect(levelIn(files.local)).toBe("off");
  });

  it("a local project already there shows its level read-only, and Open repo writes none (Q241)", async () => {
    const { calls, files } = core({ ...FRESH, yours: "on" });
    files.local = withLevel(files.local, "on");
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    expect(
      await screen.findByText("Your level: Full sandbox · change it in Settings"),
    ).toBeInTheDocument();
    await person.type(screen.getByLabelText("Or type the repo's path"), REPO);
    await person.click(screen.getByRole("button", { name: "Open repo" }));
    await waitFor(() => expect(calls("open_repo")).toHaveLength(1));
    expect(calls("open_repo")[0].args).toMatchObject({ path: REPO, protection: null });
    expect(levelIn(files.local)).toBe("on");
  });

  it("options below a required tier are out of reach, with the reason, and its default is chosen (Q241)", async () => {
    const why = "Policy forbids starting a chat without the sandbox. Set by Platform team.";
    const { calls, files } = core({ lowest: "on", default: "on", why, yours: null });
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    const row = await screen.findByRole("radiogroup", { name: "Protection" });
    await waitFor(() => expect(within(row).getByRole("radio", { name: SAID.on })).toBeChecked());
    expect(within(row).getByRole("radio", { name: SAID.floor })).toBeDisabled();
    expect(within(row).getByRole("radio", { name: SAID.off })).toBeDisabled();
    expect(screen.getByText(why, { exact: false })).toBeInTheDocument();
    // Still one press.
    await person.type(screen.getByLabelText("Or type the repo's path"), REPO);
    await person.click(screen.getByRole("button", { name: "Open repo" }));
    await waitFor(() => expect(calls("open_repo")).toHaveLength(1));
    expect(calls("open_repo")[0].args).toMatchObject({ protection: "on" });
    expect(levelIn(files.local)).toBe("on");
  });

  it("the forge-hosted create shows the row and sends its answer (review 3)", async () => {
    const { calls } = core();
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await person.click(await screen.findByRole("radio", { name: /^GitHub/ }));
    const row = await screen.findByRole("radiogroup", { name: "Protection" });
    await person.click(within(row).getByRole("radio", { name: SAID.on }));
    await person.type(await screen.findByLabelText("Name"), "garden");
    const create = await screen.findByRole("button", { name: "Create project" });
    await waitFor(() => expect(create).toBeEnabled());
    await person.click(create);
    await waitFor(() => expect(calls("create_project_on_forge")).toHaveLength(1));
    expect(calls("create_project_on_forge")[0].args.ask).toMatchObject({
      name: "garden",
      protection: "on",
    });
  });
});
