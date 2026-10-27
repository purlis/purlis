// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsTab } from "./SettingsTab";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";
import { forgetGroups } from "./links";
import type {
  YourLevel,
  LevelWaiting,
  PlainFileVault,
  SettingsEdit,
  SettingsField,
  SettingsFile,
  SettingsWhich,
} from "../bindings";
import { offeredIn, pickIn, selectNamed } from "../ui/select/select.testkit";

/**
 * **Your own sandbox level** (#2071): Settings › Project › Sandbox › Your level, a choice for the
 * project and one per persona, written to this machine's `purlis.local.toml` alone; and before
 * the sandbox is turned off, the project's plain-file vaults, each offered a move into the
 * keychain. What the core accepts and how a vault moves are `purlis_core`'s to test.
 */

const PLANE = "/home/dev/plane";

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

function applied(was: SettingsFile, edits: readonly SettingsEdit[]): SettingsFile {
  let fields = [...was.fields];
  for (const edit of edits) {
    const at = JSON.stringify(edit.path);
    fields = fields.filter((one) => JSON.stringify(one.path) !== at);
    if (edit.value !== null) fields.push({ path: edit.path, value: edit.value });
  }
  return { ...was, fields, text: `${was.text}# written\n` };
}

function core({
  vaults = [] as PlainFileVault[],
  move = (name: string) => `Moved ${name}.`,
  waiting = [] as LevelWaiting[],
  yours = undefined as YourLevel | undefined,
  modeOn = false,
} = {}) {
  const files: Record<SettingsWhich, SettingsFile> = {
    shared: file(
      "shared",
      modeOn
        ? [{ path: [{ key: "sandbox" }, { key: "mode" }], value: { kind: "text", value: "on" } }]
        : [],
    ),
    local: file("local"),
  };
  const sent: { which: SettingsWhich; edits: SettingsEdit[] }[] = [];
  const moved: string[] = [];
  let held = [...vaults];
  let waits = [...waiting];
  const confirmed: string[] = [];
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "project_settings":
          return { shared: files.shared, local: files.local };
        case "project_extensions":
          return { extensions: [], local_left_out: null };
        case "sandbox_state":
          return {
            on: true,
            offer: false,
            said: null,
            never: [],
            hosts_changed: null,
            presets_changed: null,
            presets: [],
            persona_hosts: [],
            besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
            policy: null,
            ...(yours === undefined ? {} : { yours }),
          };
        case "start_options":
          return {
            profiles: [],
            refused: [],
            personas: ["steward", "scribe"],
            persona: null,
            persona_profiles: {},
            ignore_fix: null,
            declares_none: false,
          };
        case "plain_file_vaults":
          return held;
        case "sandbox_levels_waiting":
          return waits;
        case "confirm_sandbox_level": {
          const persona = (given.persona ?? null) as string | null;
          confirmed.push(`${persona ?? "project"}:${given.level as string}`);
          waits = waits.filter((one) => one.persona !== persona);
          return `Confirmed for ${persona ?? "this project"}.`;
        }
        case "move_vault_to_keychain": {
          const name = given.name as string;
          moved.push(name);
          held = held.filter((one) => one.name !== name);
          return move(name);
        }
        case "save_project_settings": {
          const which = given.which as SettingsWhich;
          const change = given.change as { kind: "edits"; edits: SettingsEdit[] };
          sent.push({ which, edits: change.edits });
          files[which] = applied(files[which], change.edits);
          return { kind: "saved", file: files[which] };
        }
      }
      return undefined;
    },
    { shouldMockEvents: true },
  );
  return { sent, moved, confirmed };
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

async function atYourLevel() {
  render(<SettingsTab plane={PLANE} level="project" />);
  await userEvent.click(
    await within(await screen.findByRole("navigation", { name: "Groups" })).findByRole("button", {
      name: "Your level",
    }),
  );
}

const picker = (label: string) => selectNamed(label);

describe("Settings › Sandbox › Your level", () => {
  it("offers the three levels for the project and for each persona, none set", async () => {
    core();
    await atYourLevel();
    expect(screen.getByText(/on this machine only/)).toBeInTheDocument();
    // Each level starts as chosen, and a lower one written another way waits for Confirm.
    expect(screen.getByText(/Floor starts the floor even where the project/)).toBeInTheDocument();
    await waitFor(() => expect(picker("As scribe")).toBeInTheDocument());
    expect(await offeredIn(picker("This project"))).toEqual([
      "not set — the project's level (full sandbox)",
      "Full sandbox",
      "Floor",
      "No sandbox",
    ]);
    expect(await offeredIn(picker("As steward"))).toEqual([
      "not set — as this project",
      "Full sandbox",
      "Floor",
      "No sandbox",
    ]);
  });

  it("writes the project's level and a persona's to this machine's file alone", async () => {
    const { sent } = core();
    await atYourLevel();
    await pickIn(picker("This project"), "off");
    await waitFor(() =>
      expect(sent).toEqual([
        {
          which: "local",
          edits: [
            {
              path: [{ key: "sandbox" }, { key: "level" }],
              value: { kind: "text", value: "off" },
            },
          ],
        },
      ]),
    );
    await waitFor(() => expect(picker("As scribe")).toBeInTheDocument());
    await pickIn(picker("As scribe"), "on");
    await waitFor(() =>
      expect(sent.at(-1)).toEqual({
        which: "local",
        edits: [
          {
            path: [{ key: "sandbox" }, { key: "personas" }, { key: "scribe" }],
            value: { kind: "text", value: "on" },
          },
        ],
      }),
    );
  });

  it("warns of the plain-file vaults and moves one this machine registers into the keychain", async () => {
    const { moved } = core({
      vaults: [
        { name: "ops", movable: true },
        { name: "team", movable: false },
      ],
    });
    await atYourLevel();

    expect(await screen.findByText(/plain-file vaults ops, team are files/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Move team to the keychain" })).toBeNull();
    expect(screen.getByText(/the team moves it/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Move ops to the keychain" }));

    expect(await screen.findByText("Moved ops.")).toBeInTheDocument();
    expect(moved).toEqual(["ops"]);
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Move ops to the keychain" })).toBeNull(),
    );
  });

  it("says so when the project has no plain-file vault", async () => {
    core();
    await atYourLevel();
    expect(await screen.findByText("This project has no plain-file vault.")).toBeInTheDocument();
  });
  it("shows a lower level the file alone says, with Confirm, and confirms it", async () => {
    const { confirmed } = core({ waiting: [{ persona: "qa", level: "off" }] });
    await atYourLevel();

    expect(
      await screen.findByText(/As qa, the file says no sandbox, which you did not set here/),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Confirm no sandbox as qa" }));

    expect(await screen.findByText("Confirmed for qa.")).toBeInTheDocument();
    // The level the row showed goes with the Confirm, so a file changed since is refused.
    expect(confirmed).toEqual(["qa:off"]);
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: /^Confirm/ })).not.toBeInTheDocument(),
    );
  });
});

/**
 * **The Sandbox page's summary says the level your chats start at** (#2071): the project's,
 * unless your own level says otherwise, with Change your level straight to the page that sets it.
 */
describe("Settings › Sandbox's summary", () => {
  async function atSandbox() {
    render(<SettingsTab plane={PLANE} level="project" />);
    await userEvent.click(
      await within(await screen.findByRole("navigation", { name: "Groups" })).findByRole("button", {
        name: "Sandbox",
      }),
    );
  }

  it("says your level where it turns the sandbox off for you, and goes to it", async () => {
    core({ modeOn: true, yours: { level: "off", confirmed: true } });
    await atSandbox();
    expect(
      await screen.findByText(/Off for you: your level, confirmed on this machine\./),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Change your level" }));
    expect(await screen.findByRole("region", { name: "Your level" })).toBeInTheDocument();
  });

  it("says the floor where your level is the floor, and the project's own otherwise", async () => {
    core({ modeOn: true, yours: { level: "floor", confirmed: true } });
    await atSandbox();
    expect(await screen.findByText(/Floor for you: your level/)).toBeInTheDocument();
    cleanup();
    core({ modeOn: true });
    await atSandbox();
    expect(await screen.findByText(/^On for everyone in this project\./)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Change your level" })).toBeInTheDocument();
    expect(screen.queryByText(/use that chat's tab/)).toBeNull();
    // Where the core says no level of yours counts (a policy keeps it), none is said as yours.
    expect(screen.queryByText(/for you:/)).toBeNull();
  });
});
