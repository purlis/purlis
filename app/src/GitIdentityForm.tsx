// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { Field, SettingActions, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { DialogAnswers, useInADialog } from "./ui/dialog/Dialog";

/**
 * **The git identity form** (FX-3): the fix for the doctor's `git identity` row, which needs a
 * name and an email before it can be applied.
 *
 * The fix registry's ids are plain (`charter doctor --fix <id>`), and a fix that takes input is
 * marked in the core (`FixId::takes_input`). The window draws this form wherever such a fix is
 * offered — the Doctor row's Fix button today, and a Notice carrying the same id — and hands
 * what is typed to the core, which checks both fields and answers each one's refusal. The
 * refusal is drawn under its field, by the design system's `SettingRow`, and nothing is
 * written until both are right.
 *
 * **It fills in, never replaces** (D-FX3-8). A key git's global config already holds is shown
 * prefilled and locked, and is not sent: the core writes only the unset key(s), and reads the
 * identity again just before it writes, so a form opened before the identity was set from a
 * terminal cannot replace it.
 */

/** The fix ids whose Fix opens a form instead of applying at once. */
export const FIXES_WITH_A_FORM: ReadonlySet<string> = new Set(["git-identity"]);

/** Each field's refusal, in the core's words; `undefined` when the input was taken. */
export type IdentityRefused = { name: string[]; email: string[] };

/** git's global identity as it stands: each value one display line, empty when unset. */
export type IdentityNow = { name: string; email: string };

export function GitIdentityForm({
  id,
  current,
  submit,
  onCancel,
  busy,
}: {
  /** The form's id, for the control that opened it (`aria-controls`). */
  id?: string;
  /** What is set already: those fields are shown locked. */
  current: IdentityNow;
  /** Hand the input to the core. Resolves with each field's refusal, or `undefined` once the
   *  fix was applied (its outcome is the caller's to say). A locked field is sent empty. */
  submit: (name: string, email: string) => Promise<IdentityRefused | undefined>;
  onCancel: () => void;
  /** Out of reach while something else is being fixed or checked. */
  busy?: boolean;
}) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [refused, setRefused] = useState<IdentityRefused>();
  const [sending, setSending] = useState(false);
  const form = useRef<HTMLDivElement>(null);
  const inADialog = useInADialog();
  const nameSet = current.name !== "";
  const emailSet = current.email !== "";

  /** The `at`th of the form's two fields: 0 the name, 1 the email. */
  const field = (at: number) => form.current?.querySelectorAll("input")[at];

  // The keyboard starts in the first field there is to fill.
  useEffect(() => {
    field(nameSet ? 1 : 0)?.focus();
  }, [nameSet]);

  // And goes back to the first field the core refused.
  useEffect(() => {
    if (refused === undefined) return;
    if (refused.name.length > 0) field(0)?.focus();
    else if (refused.email.length > 0) field(1)?.focus();
  }, [refused]);

  const locked = "Already set in git's global config; this fix leaves it as it is.";

  const send = () => {
    if (busy || sending) return;
    setSending(true);
    void submit(nameSet ? "" : name, emailSet ? "" : email)
      .then(setRefused)
      .finally(() => setSending(false));
  };

  // **A form by its role, not a `<form>` element** (#2910): it is drawn in the Doctor dialog's
  // body, which is the dialog's own form, and a form element is never inside another. Return in
  // its fields sets the identity, as its own form's Return did, and goes no further.
  return (
    <div
      id={id}
      ref={form}
      className="doctor-form"
      role="form"
      aria-label="Git identity"
      onKeyDown={(event) => {
        if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
        if (!(event.target instanceof HTMLInputElement)) return;
        event.preventDefault();
        event.stopPropagation();
        send();
      }}
    >
      <SettingRow
        label="Name"
        help={
          nameSet
            ? locked
            : "The name your commits are made under: written to git's global user.name."
        }
        error={refused?.name}
        control={(ids) => (
          <Field
            ids={ids}
            kind="text"
            value={nameSet ? current.name : name}
            onChange={setName}
            disabled={nameSet}
            maxLength={256}
            placeholder="Ann Example"
          />
        )}
      />
      <SettingRow
        label="Email"
        help={emailSet ? locked : "Written to git's global user.email."}
        error={refused?.email}
        control={(ids) => (
          <Field
            ids={ids}
            kind="text"
            value={emailSet ? current.email : email}
            onChange={setEmail}
            disabled={emailSet}
            maxLength={256}
            placeholder="ann@example.com"
          />
        )}
      />
      {/* In a dialog its answers are the dialog's, in its footer, so the main action is where
        it is in every dialog (#2910); elsewhere (a Notice) they end the form, Cancel first. */}
      {inADialog ? (
        <DialogAnswers
          cancel={{ children: "Cancel", onPress: onCancel }}
          main={{
            children: sending ? "Setting…" : "Set identity",
            reserve: ["Set identity", "Setting…"],
            isDisabled: busy || sending,
            onPress: send,
          }}
        />
      ) : (
        <SettingActions>
          <Button onPress={onCancel}>Cancel</Button>
          <Button variant="primary" isDisabled={busy || sending} onPress={send}>
            {sending ? "Setting…" : "Set identity"}
          </Button>
        </SettingActions>
      )}
    </div>
  );
}
