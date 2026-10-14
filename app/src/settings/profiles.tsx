// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState, useSyncExternalStore } from "react";
import { commands, type MachineProfileRemoval, type MachineProfileRow } from "../bindings";
import * as AlertDialog from "../overlays/alert-dialog";
import { AnswerBar } from "../AnswerBar";
import { Choice, Field, SettingActions, SettingRow } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Profiles** (#2034, ADR 0095): this machine's own profiles, each available in every
 * project here. **Add profile** makes one, with a folder of its own in purlis's data home and
 * its settings imported from this machine's harness settings, never its sign-in; each made
 * profile says when they were imported and offers **Import again**. **Remove…** asks first,
 * naming the conversations that go, and an adopted harness folder is never deleted: purlis only
 * forgets the profile, and the question says so.
 *
 * A profile is the person's, for the whole machine, so it is under You, beside This machine and
 * Accounts. The list and every act are the core's (`machine_profiles`, `machine_profile_*`), the
 * window's alone; a refusal is said in the row, in the core's words.
 */
export function profilesGroup(): SettingsGroup {
  return {
    id: "you.profiles",
    label: "Profiles",
    help: "The profiles this machine keeps for every project: each a harness with its own settings, sign-in and conversations, in a folder of its own. A profile a project declares in charter.local.toml under the same name is that project's.",
    settings: [list, add],
  };
}

/** The harnesses a profile can be made of, as the core names them. */
const HARNESSES = [
  { value: "claude", label: "Claude Code" },
  { value: "codex", label: "Codex" },
  { value: "opencode", label: "opencode" },
];

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; profiles: MachineProfileRow[] };

type Held = {
  read: Read;
  /** Why the last act was refused. */
  refused: string | undefined;
  /** What the last act did, in a sentence. */
  said: string | undefined;
  /** The removal being asked about, once the core has said what goes. */
  asking: MachineProfileRemoval | undefined;
  /** Whether an act is running, so it is not given twice. */
  busy: boolean;
};

const START: Held = {
  read: { state: "reading" },
  refused: undefined,
  said: undefined,
  asking: undefined,
  busy: false,
};
let held: Held = START;
const listeners = new Set<() => void>();
let reading = 0;

function set(change: (was: Held) => Held) {
  held = change(held);
  for (const one of listeners) one();
}

async function reread() {
  const mine = ++reading;
  const said = await commands
    .machineProfiles()
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (mine !== reading) return;
  set((was) => ({
    ...was,
    read:
      said.status === "error"
        ? { state: "trouble", trouble: said.error }
        : Array.isArray(said.data)
          ? { state: "read", profiles: said.data }
          : { state: "trouble", trouble: "purlis did not say which profiles this machine has." },
  }));
}

function subscribe(listener: () => void) {
  if (listeners.size === 0) {
    held = START;
    void reread();
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** What a command answered. */
type Done<T> = { status: "ok"; data: T } | { status: "error"; error: string };

/** Runs `act`, says what it did or why not, and reads the list again. */
async function acting<T>(act: () => Promise<Done<T>>, says: (done: T) => string | undefined) {
  set((was) => ({ ...was, busy: true, refused: undefined, said: undefined }));
  const done = await act().catch((err: unknown) => ({
    status: "error" as const,
    error: String(err),
  }));
  set((was) => ({
    ...was,
    busy: false,
    refused: done.status === "error" ? done.error : undefined,
    said: done.status === "ok" ? says(done.data) : undefined,
  }));
  await reread();
}

/** What an import says it did. */
function importedSays(
  name: string,
  done: { copied: string[]; profile: MachineProfileRow },
): string {
  const from = done.profile.imported_from ?? "this machine";
  return done.copied.length === 0
    ? `${name}: found no settings to import in ${from}.`
    : `${name}: imported ${done.copied.length === 1 ? "1 file" : `${done.copied.length} files`} of settings from ${from}. Its first chat signs in through the harness's own sign-in.`;
}

/** Asks the core what removing `profile` takes away, and opens the question. */
async function askRemove(profile: MachineProfileRow) {
  set((was) => ({ ...was, busy: true, refused: undefined, said: undefined }));
  const done = await commands
    .machineProfileRemoval(profile.name)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  set((was) => ({
    ...was,
    busy: false,
    refused: done.status === "error" ? done.error : undefined,
    asking: done.status === "ok" ? done.data : undefined,
  }));
}

function remove(shown: MachineProfileRemoval) {
  set((was) => ({ ...was, asking: undefined }));
  void acting(
    () => commands.machineProfileRemove(shown.name, shown),
    () =>
      shown.adopted
        ? `Forgot ${shown.name}. ${shown.folder} stays.`
        : `Removed ${shown.name} and deleted ${shown.folder}.`,
  );
}

/** What a profile's row says beside its name. */
function note(profile: MachineProfileRow): string {
  if (profile.origin === "adopted") return `${profile.harness}, the harness's own folder`;
  if (profile.imported_at === null) return profile.harness;
  const when = new Date(profile.imported_at * 1000).toLocaleDateString();
  return `${profile.harness}, settings imported ${when}`;
}

/** An entry's button. `tabIndex={0}`: WebKit leaves a button out of the Tab order without it
 *  (#190). */
function EntryButton({
  label,
  spoken,
  disabled,
  onClick,
}: {
  label: string;
  spoken: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="ui-setting-reset ui-button"
      tabIndex={0}
      aria-label={spoken}
      disabled={disabled}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

const list: LiveSetting = {
  id: "you.profiles.list",
  label: "This machine's profiles",
  help: "Import again takes the harness's settings from this machine once more; its conversations stay. Remove asks first and names what goes.",
  writes: "this machine",
  useControl: function useProfiles() {
    const now = useSyncExternalStore(subscribe, () => held);
    return {
      grouped: true,
      error: now.refused === undefined ? undefined : [now.refused],
      control: (ids) => (
        <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
          {now.read.state === "reading" && <p className="ui-setting-help">Reading…</p>}
          {now.read.state === "trouble" && (
            <p className="ui-setting-help" role="alert">
              {now.read.trouble}
            </p>
          )}
          {now.read.state === "read" &&
            (now.read.profiles.length === 0 ? (
              <p className="ui-setting-help">This machine keeps no profile of its own yet.</p>
            ) : (
              <ul className="ui-machine-list">
                {now.read.profiles.map((profile) => (
                  <li key={profile.name} className="ui-machine-entry">
                    <span className="ui-machine-name">{profile.name}</span>
                    <span className="ui-machine-path">{profile.folder}</span>
                    <span className="ui-setting-badge">{note(profile)}</span>
                    {profile.origin === "made" && (
                      <EntryButton
                        label="Import again"
                        spoken={`Import the settings of ${profile.name} again`}
                        disabled={now.busy}
                        onClick={() =>
                          void acting(
                            () => commands.machineProfileImport(profile.name),
                            (done) => importedSays(profile.name, done),
                          )
                        }
                      />
                    )}
                    <EntryButton
                      label="Remove…"
                      spoken={`Remove ${profile.name}`}
                      disabled={now.busy}
                      onClick={() => void askRemove(profile)}
                    />
                  </li>
                ))}
              </ul>
            ))}
          {now.said !== undefined && (
            <p className="ui-setting-help" role="status">
              {now.said}
            </p>
          )}
          {now.asking !== undefined && (
            <RemoveProfile
              removal={now.asking}
              onRemove={remove}
              onCancel={() => set((was) => ({ ...was, asking: undefined }))}
            />
          )}
        </div>
      ),
    };
  },
};

/**
 * **Remove a profile?** Radix's `AlertDialog`, as every question whose answer destroys something
 * is: what goes, in the core's words and with each conversation named, then Cancel first and
 * the removal last at the trailing edge (D-1210-8). An adopted profile's question says its
 * folder stays.
 */
export function RemoveProfile({
  removal,
  onRemove,
  onCancel,
}: {
  removal: MachineProfileRemoval;
  onRemove: (shown: MachineProfileRemoval) => void;
  onCancel: () => void;
}) {
  const conversations = removal.conversations ?? [];
  return (
    <AlertDialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="ui-scrim" />
        <AlertDialog.Content className="ui-dialog warning">
          <AlertDialog.Title>
            {removal.adopted ? `Forget ${removal.name}?` : `Remove ${removal.name}?`}
          </AlertDialog.Title>
          <AlertDialog.Description className="came-back">{removal.says}</AlertDialog.Description>
          {!removal.adopted && conversations.length > 0 && (
            <>
              <h3>Conversations that go</h3>
              <ul className="at-risk" aria-label={`Conversations of ${removal.name}`}>
                {conversations.map((one) => (
                  <li key={one}>{one}</li>
                ))}
              </ul>
            </>
          )}
          <AnswerBar>
            <AlertDialog.Cancel asChild>
              <button className="ui-button" type="button" tabIndex={0} onClick={onCancel}>
                Cancel
              </button>
            </AlertDialog.Cancel>
            <button
              type="button"
              className="ui-button ui-button-danger ends-it"
              tabIndex={0}
              onClick={() => onRemove(removal)}
            >
              {removal.adopted ? "Forget profile" : "Remove profile"}
            </button>
          </AnswerBar>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

const add: LiveSetting = {
  id: "you.profiles.add",
  label: "Add profile",
  help: "A new profile of a harness, for a second account or a client's. Its settings are imported from this machine's harness settings, never its sign-in, and its first chat signs in through the harness's own sign-in.",
  writes: "this machine",
  useControl: function useAdd() {
    const now = useSyncExternalStore(subscribe, () => held);
    const [name, setName] = useState("");
    const [kind, setKind] = useState("claude");
    const submit = () => {
      const named = name.trim();
      if (named === "" || now.busy) return;
      void acting(
        () => commands.machineProfileCreate(named, kind),
        (done) => {
          setName("");
          return importedSays(named, done);
        },
      );
    };
    return {
      grouped: true,
      control: (ids) => (
        <form
          role="group"
          aria-labelledby={ids.labelledBy}
          aria-describedby={ids.describedBy}
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          <SettingRow
            label="Name"
            control={(inner) => (
              <Field
                ids={inner}
                kind="text"
                value={name}
                placeholder="claude-work"
                disabled={now.busy}
                onChange={setName}
              />
            )}
          />
          <SettingRow
            label="Harness"
            control={(inner) => (
              <Choice
                ids={inner}
                kind="select"
                options={HARNESSES}
                value={kind}
                disabled={now.busy}
                onValueChange={setKind}
              />
            )}
          />
          <SettingActions>
            <button
              type="submit"
              className="ui-button"
              tabIndex={0}
              disabled={now.busy || name.trim() === ""}
            >
              Add profile
            </button>
          </SettingActions>
        </form>
      ),
    };
  },
};
