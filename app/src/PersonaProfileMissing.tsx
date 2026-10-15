// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useState } from "react";
import { commands, type PersonaProfile, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { Choice, SettingActions, SettingRow } from "./settings/components";
import { HARNESSES } from "./settings/profiles";

/**
 * **A persona whose profile this machine does not have** (#2039, spec #2030 story 21, ADR 0095
 * §1): a teammate opens a persona whose definition names a profile, `profile: qa-bot`, that this
 * project does not offer on their machine. Its tab offers to make a profile of that name here,
 * to sign in with their own account.
 *
 * **An offer on the persona's tab, not an Ask** (D-2039-1). Opening a persona is no wait:
 * nothing is held until the person answers, and a chat handed to the persona meanwhile starts on
 * the asking chat's profile and says so (D-1445-8). So it is a Notice on the tab with its fix,
 * and nothing is raised in the Inbox.
 *
 * **Nothing is made until Create is pressed** (D-2039-2). The fix opens a form under the line
 * with the name fixed to the one the definition names and a harness to pick, and Create runs
 * `machine_profile_create`, the command Settings › You › Profiles' Add profile runs: the same
 * folder, the same settings import, no sign-in copied. Signing it in is then the profile's own
 * Ask (#2035), raised by the first chat that starts on it.
 *
 * Only a `profile:` line counts, as `personaprofile::own` reads it: a `model:` names a model,
 * and is a profile only where one is called that.
 */
export function PersonaProfileMissing({
  plane,
  persona,
  changed,
  onMade,
}: {
  plane: PlaneId;
  persona: string;
  /** Moves when the project changed on disk: the definition is read again. */
  changed: number;
  /** A profile was made: the persona's view is asked again, so it says what this machine has. */
  onMade: () => void;
}) {
  const [read, setRead] = useState<PersonaProfile>();
  const [round, setRound] = useState(0);
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState(HARNESSES[0].value);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  /** What Create did, in a sentence, until it is dismissed. */
  const [made, setMade] = useState<string>();
  const form = useId();

  useEffect(() => {
    let gone = false;
    void commands
      .personaProfile(plane, persona)
      .catch(() => undefined)
      .then((answer) => {
        if (!gone) setRead(answer?.status === "ok" ? answer.data : undefined);
      });
    return () => {
      gone = true;
    };
  }, [plane, persona, changed, round]);

  const named = read?.named ?? null;
  if (named === null || read === undefined || (read.offered ?? []).includes(named))
    return made === undefined ? null : (
      <Notice cause={`persona-profile-made:${persona}`} onDismiss={() => setMade(undefined)}>
        {made}
      </Notice>
    );

  const create = async () => {
    if (busy) return;
    setBusy(true);
    setRefused(undefined);
    const done = await commands
      .machineProfileCreate(named, kind, false)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setBusy(false);
    if (done.status === "error") {
      setRefused(done.error);
      return;
    }
    setOpen(false);
    setMade(madeSays(named, done.data));
    setRound((n) => n + 1);
    onMade();
  };

  return (
    <Notice
      cause={`persona-profile-missing:${persona}`}
      label={`${persona} names the profile ${named}, which this machine does not have`}
      fixes={[
        {
          label: `Create profile ${named}…`,
          onPress: () => setOpen((was) => !was),
          opens: { id: form, open },
        },
      ]}
      under={
        open && (
          <form
            id={form}
            aria-label={`Create profile ${named}`}
            onSubmit={(event) => {
              event.preventDefault();
              void create();
            }}
          >
            <SettingRow
              label="Harness"
              help={`A profile of this machine's, called ${named}, available in every project here. This machine's harness settings are copied into it, never a sign-in; its first chat asks you to sign it in.`}
              control={(ids) => (
                <Choice
                  ids={ids}
                  kind="select"
                  options={HARNESSES}
                  value={kind}
                  disabled={busy}
                  onValueChange={setKind}
                />
              )}
            />
            {refused !== undefined && (
              <p className="trouble" role="alert">
                {refused}
              </p>
            )}
            <SettingActions>
              <button
                type="submit"
                className="ui-button ui-button-primary"
                tabIndex={0}
                disabled={busy}
              >
                {busy ? "Creating…" : "Create profile"}
              </button>
            </SettingActions>
          </form>
        )
      }
    >
      {`${persona} names the profile ${named}, which this machine does not have.`}
    </Notice>
  );
}

/** What Create says it made: what it imported, as Add profile says it, and what comes next. */
function madeSays(
  name: string,
  done: { copied: string[]; profile: { imported_from: string | null } },
) {
  const from = done.profile.imported_from ?? "this machine";
  const imported =
    done.copied.length === 0
      ? `found no settings to import in ${from}`
      : `imported ${done.copied.length === 1 ? "1 file" : `${done.copied.length} files`} of settings from ${from}`;
  return `Made the profile ${name} for this machine and ${imported}. Its first chat asks you to sign it in.`;
}
