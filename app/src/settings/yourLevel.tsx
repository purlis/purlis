// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  commands,
  type LevelWaiting,
  type PlainFileVault,
  type PlaneId,
  type SettingsStep,
} from "../bindings";
import { inTheWindow } from "../personaHostsAllow";
import type { RowIds } from "./components";
import { key, textAt, type Control } from "./fileControls";
import type { LiveSetting } from "./groups";

/** The group: Settings › Project › Sandbox › Your level (#2071). */
export const YOUR_LEVEL = "project.sandbox.yours";

/** Where the project's level is kept, in this machine's file. */
export const LEVEL_KEY = key("sandbox", "level");

/** The three levels, strictest first, as the file writes them. */
export const LEVELS = ["on", "floor", "off"] as const;

/** What each level is called here, as on a chat's tab. */
export const LEVEL_SAID: Readonly<Record<(typeof LEVELS)[number], string>> = {
  on: "Full sandbox",
  floor: "Floor",
  off: "No sandbox",
};

/** What the group says above its rows. */
export function yourLevelHelp(file: string): string {
  return `How much the sandbox holds for your chats in this project, on this machine only. Kept in ${file}, never in a committed file. A chat takes it at its next start; a chat already running keeps the level it started with. A level set here counts as yours; a lower one written into the file any other way waits for your Confirm below, and a chat starts at what you confirmed, or the floor, until then. Full sandbox is the project's hosts, folders and Allow asks. Floor reaches every host and folder, but never a vault's files or a file a program runs later. No sandbox holds nothing: a chat can read any file on disk, a plain-file vault's included.`;
}

/**
 * **Your level for the project, and for each persona** (#2071): a choice per row, kept in this
 * machine's `purlis.local.toml` (`[sandbox] level`, `[sandbox.personas]`). A persona's level
 * wins over the project's for a chat started as it; not set, it takes the project's, and the
 * project's not set is the floor.
 */
export function yourLevelControls(personas: readonly string[]): Control[] {
  const choice = (path: SettingsStep[], label: string, hint: string, unset: string): Control => ({
    ...textAt(path, label, { kind: "choice", hint }),
    choices: [...LEVELS],
    labels: LEVEL_SAID,
    unset,
  });
  return [
    choice(
      LEVEL_KEY,
      "This project",
      "Your level for every chat of this project, unless its persona has its own below.",
      "not set — the floor",
    ),
    ...personas.map((persona) =>
      choice(
        key("sandbox", "personas", persona),
        `As ${persona}`,
        `Your level for a chat started as ${persona}, over the project's.`,
        "not set — as this project",
      ),
    ),
  ];
}

/** Whether this page may move a vault: the window's alone. */
const mayMove = (): boolean => inTheWindow(commands, "moveVaultToKeychain");

/** What the row says of the vaults before anything is moved. */
export function plainFilesSaid(vaults: readonly PlainFileVault[]): string {
  const names = vaults.map((one) => one.name).join(", ");
  return `With no sandbox, a chat can read every file on disk, and the plain-file vaults ${names} are files. Move them into the keychain before you turn the sandbox off: declared tools and secret exec go on working.`;
}

/** The plain-file vaults, each with Move to the keychain where this machine may move it. */
function PlainFiles({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const [vaults, setVaults] = useState<readonly PlainFileVault[]>();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const read = () =>
    void commands
      .plainFileVaults(plane)
      // A core that answers nothing lists nothing.
      .then((answer) =>
        setVaults(answer.status === "ok" ? ((answer.data as PlainFileVault[] | null) ?? []) : []),
      )
      .catch(() => setVaults([]));
  useEffect(read, [plane]);
  const move = (name: string) => {
    setBusy(true);
    setSaid(undefined);
    void commands
      .moveVaultToKeychain(plane, name)
      .then((answer) => setSaid(answer.status === "ok" ? answer.data : answer.error))
      .catch((err: unknown) => setSaid(`purlis could not move ${name}: ${String(err)}`))
      .finally(() => {
        setBusy(false);
        read();
      });
  };
  return (
    <div
      id={ids.id}
      role="group"
      className="ui-setting-status"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      {vaults === undefined ? (
        <p>Reading this project's vaults…</p>
      ) : vaults.length === 0 ? (
        <p>This project has no plain-file vault.</p>
      ) : (
        <>
          <p>{plainFilesSaid(vaults)}</p>
          <ul>
            {vaults.map((one) => (
              <li key={one.name}>
                <code>{one.name}</code>{" "}
                {!one.movable ? (
                  <span>— registered in the team's file: the team moves it.</span>
                ) : mayMove() ? (
                  <button
                    type="button"
                    className="ui-button"
                    disabled={busy}
                    onClick={() => move(one.name)}
                  >
                    {`Move ${one.name} to the keychain`}
                  </button>
                ) : null}
              </li>
            ))}
          </ul>
        </>
      )}
      {said !== undefined && <p>{said}</p>}
    </div>
  );
}

/** The row of the plain-file vaults, under Your level. None where this page cannot call. */
export function plainFilesRow(plane: PlaneId): LiveSetting[] {
  if (!inTheWindow(commands, "plainFileVaults")) return [];
  return [
    {
      id: `${YOUR_LEVEL}.plain-files`,
      label: "Plain-file vaults",
      // A vault's move is this machine's: its keychain and its own registration.
      writes: "this machine",
      help: "What a chat with no sandbox could read, and the move that keeps it out of reach.",
      useControl: () => ({
        grouped: true,
        control: (ids) => <PlainFiles plane={plane} ids={ids} />,
      }),
    },
  ];
}

/** What a waiting level says. */
function waitingSaid(one: LevelWaiting): string {
  const level = LEVEL_SAID[one.level as (typeof LEVELS)[number]].toLowerCase();
  const whose = one.persona === null ? "For this project" : `As ${one.persona}`;
  return `${whose}, the file says ${level}, which you did not set here. A chat starts at what you confirmed, or the floor, until you confirm it.`;
}

/** The levels the file says lower than you set or confirmed, each with Confirm. */
function Waiting({ plane, ids, text }: { plane: PlaneId; ids: RowIds; text: string }) {
  const [waiting, setWaiting] = useState<readonly LevelWaiting[]>();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const read = () =>
    void commands
      .sandboxLevelsWaiting(plane)
      // A core that answers nothing lists nothing.
      .then((answer) =>
        setWaiting(answer.status === "ok" ? ((answer.data as LevelWaiting[] | null) ?? []) : []),
      )
      .catch(() => setWaiting([]));
  // Read again whenever the file's text changes: a save, or an edit by hand Settings read.
  useEffect(read, [plane, text]);
  const confirm = (persona: string | null, level: string) => {
    setBusy(true);
    setSaid(undefined);
    void commands
      .confirmSandboxLevel(plane, persona, level)
      .then((answer) => setSaid(answer.status === "ok" ? answer.data : answer.error))
      .catch((err: unknown) => setSaid(`purlis could not confirm it: ${String(err)}`))
      .finally(() => {
        setBusy(false);
        read();
      });
  };
  return (
    <div
      id={ids.id}
      role="group"
      className="ui-setting-status"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      {waiting === undefined ? (
        <p>Reading your levels…</p>
      ) : waiting.length === 0 ? (
        <p>Every level here is one you set or confirmed.</p>
      ) : (
        <ul>
          {waiting.map((one) => {
            const level = LEVEL_SAID[one.level as (typeof LEVELS)[number]].toLowerCase();
            const whose = one.persona === null ? "for this project" : `as ${one.persona}`;
            return (
              <li key={one.persona ?? ""}>
                <span>{waitingSaid(one)}</span>{" "}
                <button
                  type="button"
                  className="ui-button"
                  disabled={busy}
                  onClick={() => confirm(one.persona, one.level)}
                >
                  {`Confirm ${level} ${whose}`}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {said !== undefined && <p>{said}</p>}
    </div>
  );
}

/** The row of the levels waiting for Confirm, under Your level. None where this page cannot
 *  confirm: the record it writes is the window's alone. */
export function levelsWaitingRow(plane: PlaneId, text: string): LiveSetting[] {
  if (!inTheWindow(commands, "confirmSandboxLevel")) return [];
  return [
    {
      id: `${YOUR_LEVEL}.waiting`,
      label: "Waiting for your Confirm",
      // The record of what you confirmed is this machine's, outside the project.
      writes: "this machine",
      help: "A chat can write the settings file, so a lower level it holds counts only once you confirm it here.",
      useControl: () => ({
        grouped: true,
        control: (ids) => <Waiting plane={plane} ids={ids} text={text} />,
      }),
    },
  ];
}
