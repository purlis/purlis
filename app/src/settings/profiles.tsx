// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState, useSyncExternalStore } from "react";
import { commands, type MachineProfileRemoval, type MachineProfileRow } from "../bindings";
import { HarnessIcon } from "../ui/harness-icon/HarnessIcon";
import { Choice, Field, SettingActions, SettingRow } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";
import { askProfileSignIn } from "./links";
import { Button } from "../ui/button/Button";
import { AlertDialog } from "../ui/dialog/Dialog";
import { Form } from "../ui/field/Field";
import { Badge } from "../ui/badge/Badge";

/**
 * **You › Profiles** (#2034, ADR 0095): this machine's own profiles, each available in every
 * project here. **Add profile** makes one, with a folder of its own in purlis's data home and
 * this machine's harness settings files copied in as they are, and no sign-in file; each made
 * profile says when they were imported and offers **Import again**. Each says the harness version it
 * is pinned to (#2038), and a profile whose next chat would not start on the version installed
 * (another major) offers **Update the pin**. **Remove…** asks first,
 * naming the conversations that go, and an adopted harness folder is never deleted: purlis only
 * forgets the profile, and the question says so. A profile whose sign-in the commands its chats
 * run can read says so plainly in its row, in the core's words (#2037, ADR 0095 §5).
 *
 * A profile is the person's, for the whole machine, so it is under You, beside This machine and
 * Accounts. The list and every act are the core's (`machine_profiles`, `machine_profile_*`), the
 * window's alone; a refusal is said in the row, in the core's words.
 *
 * **Each row says how its profile signs in and where** (#2035, ADR 0095 §3), and whether its
 * harness says it is signed in on this device. **Sign in** on a made profile asks the window of
 * `plane` for a shell tab running the harness's own sign-in in the profile's folder; with no
 * project behind Settings, the row says the command that does the same.
 */
export function profilesGroup(plane?: string): SettingsGroup {
  return {
    id: "you.profiles",
    label: "Profiles",
    help: "The profiles this machine keeps for every project: each a harness with its own settings, sign-in and conversations, in a folder of its own. A profile a project declares under the same name in its own local settings is that project's.",
    settings: [listed(plane), add],
  };
}

/** The harnesses a profile can be made of, as the core names them. */
export const HARNESSES = [
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
    : `${name}: imported ${done.copied.length === 1 ? "1 file" : `${done.copied.length} files`} of settings from ${from}. Sign it in through the harness's own sign-in with Sign in.`;
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
  if (profile.sandbox_only) return `${profile.harness}, for sandboxed chats only`;
  if (profile.imported_at === null) return profile.harness;
  const when = new Date(profile.imported_at * 1000).toLocaleDateString();
  return `${profile.harness}, settings imported ${when}`;
}

/** What a profile's row says of its sign-in: how and where, then what its harness says. */
function signedIn(profile: MachineProfileRow): string {
  switch (profile.standing) {
    case "signed-in":
      return `Signs in ${profile.signs_in}. Signed in.`;
    case "signed-out":
      return `Signs in ${profile.signs_in}. Not signed in: a chat on it does not start until it is.`;
    case "cannot-tell":
      return `Signs in ${profile.signs_in}. ${profile.harness} cannot say whether it is signed in.`;
    default:
      return `Signs in with ${profile.signs_in}.`;
  }
}

/** What a profile's row says about its harness version (#2038): the one it is pinned to, and the
 *  one installed where that is another: one its next chat follows, or one only Update the pin
 *  moves it to. */
function pinNote(profile: MachineProfileRow): string {
  if (profile.pinned === null) return "Pinned by its first chat";
  if (profile.installed === null || profile.installed === profile.pinned) {
    return `Pinned to ${profile.pinned}`;
  }
  return profile.offered === null
    ? `Pinned to ${profile.pinned}, its next chat follows ${profile.installed}`
    : `Pinned to ${profile.pinned}, ${profile.installed} installed`;
}

/** The version a profile is offered to pin to: the one installed, where its next chat would not
 *  start on it. The core says which (another major, or an older version). */
function offeredVersion(profile: MachineProfileRow): string | undefined {
  return profile.offered ?? undefined;
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
    <Button
      aria-label={spoken}
      isDisabled={disabled}
      onPress={onClick}
      className="ui-setting-reset"
    >
      {label}
    </Button>
  );
}

const listed = (plane: string | undefined): LiveSetting => ({
  id: "you.profiles.list",
  label: "This machine's profiles",
  help: "Import again takes the harness's settings from this machine once more; its conversations stay. A profile's chats run on the harness version it is pinned to and follow its minor and patch updates; Update the pin moves them, and the conversations they resume, to another major version installed. Remove asks first and names what goes.",
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
                    <HarnessIcon icon={profile.icon} kind={profile.kind} />
                    <span className="ui-machine-name">{profile.name}</span>
                    <span className="ui-machine-path">{profile.folder}</span>
                    <Badge variant="accent">{note(profile)}</Badge>
                    <span className="ui-setting-help">{signedIn(profile)}</span>
                    {profile.origin === "made" &&
                      (plane === undefined ? (
                        <span className="ui-setting-help">
                          Open a project to sign in from its window, or run{" "}
                          <code>purlis profile sign-in {profile.name}</code> in your terminal.
                        </span>
                      ) : profile.plans.length > 0 ? (
                        // A harness that serves several plans is signed in to the one purlis
                        // names (opencode: ChatGPT or GitHub Copilot, never a Claude plan).
                        profile.plans.map((plan) => (
                          <EntryButton
                            key={plan.id}
                            label={`Sign in to ${plan.title}`}
                            spoken={`Sign ${profile.name} in to ${plan.title}`}
                            disabled={now.busy}
                            onClick={() => askProfileSignIn(plane, profile.name, plan.id)}
                          />
                        ))
                      ) : (
                        <EntryButton
                          label={profile.standing === "signed-in" ? "Sign in again" : "Sign in"}
                          spoken={`Sign ${profile.name} in`}
                          disabled={now.busy}
                          onClick={() => askProfileSignIn(plane, profile.name)}
                        />
                      ))}
                    <Badge variant="accent">{pinNote(profile)}</Badge>
                    {offeredVersion(profile) !== undefined && (
                      <EntryButton
                        label="Update the pin"
                        spoken={`Update the pin of ${profile.name} to ${offeredVersion(profile)}`}
                        disabled={now.busy}
                        onClick={() => {
                          const version = offeredVersion(profile) ?? "";
                          void acting(
                            () => commands.machineProfilePin(profile.name, version),
                            () =>
                              `${profile.name}: its chats run on ${profile.harness} ${version} from now on.`,
                          );
                        }}
                      />
                    )}
                    {profile.commands_can_read_sign_in !== null && (
                      <span className="ui-setting-help">{profile.commands_can_read_sign_in}</span>
                    )}
                    {profile.origin === "made" && !profile.sandbox_only && (
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
});

/**
 * **Remove a profile?** The family's `AlertDialog`, as every question whose answer destroys something
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
    <AlertDialog
      onClose={onCancel}
      title={removal.adopted ? `Forget ${removal.name}?` : `Remove ${removal.name}?`}
      description={<p className="came-back">{removal.says}</p>}
      main={{
        children: removal.adopted ? "Forget profile" : "Remove profile",
        variant: "danger",
        onPress: () => onRemove(removal),
      }}
    >
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
    </AlertDialog>
  );
}

const add: LiveSetting = {
  id: "you.profiles.add",
  label: "Add profile",
  help: "A new profile of a harness, for a second account or a client's. This machine's harness settings files are copied into it as they are, and Sign in signs it in through the harness's own sign-in.",
  writes: "this machine",
  useControl: function useAdd() {
    const now = useSyncExternalStore(subscribe, () => held);
    const [name, setName] = useState("");
    const [kind, setKind] = useState("claude");
    const [sandboxOnly, setSandboxOnly] = useState(false);
    const submit = () => {
      const named = name.trim();
      if (named === "" || now.busy) return;
      void acting(
        () => commands.machineProfileCreate(named, kind, sandboxOnly),
        (done) => {
          setName("");
          return sandboxOnly
            ? `${named}: made for sandboxed chats only, with nothing imported. Its first chat signs in through the harness's own sign-in.`
            : importedSays(named, done);
        },
      );
    };
    return {
      grouped: true,
      control: (ids) => (
        <Form
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
          <SettingRow
            label="For sandboxed chats only"
            help="Only a sandboxed chat starts on it, and a sandboxed Codex chat starts on no other profile. Nothing is imported into it."
            control={(inner) => (
              <Choice
                kind="switch"
                ids={inner}
                checked={sandboxOnly}
                disabled={now.busy}
                onCheckedChange={setSandboxOnly}
              />
            )}
          />
          <SettingActions>
            <Button type="submit" isDisabled={now.busy || name.trim() === ""}>
              Add profile
            </Button>
          </SettingActions>
        </Form>
      ),
    };
  },
};
