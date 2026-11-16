// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState } from "react";
import { commands, type KeySaved, type KeyTarget } from "./bindings";
import { SettingActions, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Reserved } from "./ui/dialog/Reserved";
import { TextField } from "./ui/field/Field";
import { windowCopy } from "./windowCopy";

/**
 * **The one key form** (T1a-11, V7, X3): the one place the window takes a pasted model key. It
 * has no container of its own: each host draws it in place, never as a dialog: Your keys under
 * its head for a new entry and under a row for *Replace the key…*, J0's Add profile form inside
 * itself (T1a-13), and New chat in its right pane (T1a-27).
 *
 * **The value is never React's.** The box is the field family's secret box (`inputRef`, no
 * `value`), read once at Save and emptied only once the key is saved, so W9's *Replace the
 * key…* uses the value still in hand and the person never pastes it again (X2).
 *
 * **Aimed at a target** (`KeyTarget`): a new entry, this entry (the new value under the same
 * id, marked), or this keyless profile (the saved entry's id written into it). Where a key for
 * the product is held, the form offers *Use the key you already have* (W2) and, unless it was
 * aimed at a missing entry (F-8), W9's *Replace the key…*, which says how many chats use the
 * held key (W5) before it replaces it in place. A save that fails says why (W89; W88 for this
 * entry) with *Try again*. Core runs the check at save (V10).
 */
export function KeyForm({
  target,
  onSaved,
  onCancel,
}: {
  target: KeyTarget;
  /** The key is saved under `id`. */
  onSaved: (id: string) => void;
  /** The host's way back, where it offers one. */
  onCancel?: () => void;
}) {
  const box = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [held, setHeld] = useState<{ held: string; mayReplace: boolean } | undefined>();
  /** W9's *Replace the key…* pressed: the chats the held key has, before it is replaced. */
  const [counting, setCounting] = useState<number | undefined>();
  const [trouble, setTrouble] = useState<string | undefined>();
  const replacing = target.kind === "this-entry";
  const profile = target.kind === "this-profile" ? target.name : null;

  /** Runs `act` with the value in the box, which is emptied once the key is saved. */
  const withTheValue = async (
    act: (
      value: string,
    ) => Promise<{ status: "ok"; data: KeySaved } | { status: "error"; error: string }>,
    says: (error: string) => string,
  ) => {
    if (busy) return;
    const value = box.current?.value ?? "";
    if (value.trim() === "") {
      box.current?.focus();
      return;
    }
    setBusy(true);
    setTrouble(undefined);
    const done = await act(value).catch((err: unknown) => ({
      status: "error" as const,
      error: String(err),
    }));
    setBusy(false);
    if (done.status === "error") {
      setTrouble(says(done.error));
      return;
    }
    if (done.data.kind === "held") {
      setHeld({ held: done.data.held, mayReplace: done.data.may_replace });
      return;
    }
    if (box.current) box.current.value = "";
    setHeld(undefined);
    setCounting(undefined);
    onSaved(done.data.id);
  };

  const save = () =>
    void withTheValue(
      (value) => commands.yourKeySave(target, value),
      (error) => (replacing ? windowCopy.W88({ error }) : windowCopy.W89({ error })),
    );

  const askReplaceHeld = async (heldId: string) => {
    setBusy(true);
    const said = await commands
      .yourKeyChats(heldId)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setBusy(false);
    if (said.status === "error") setTrouble(windowCopy.W88({ error: said.error }));
    else setCounting(said.data);
  };

  const replaceHeld = (heldId: string) =>
    void withTheValue(
      (value) => commands.yourKeyReplaceHeld(heldId, value, profile),
      (error) => windowCopy.W88({ error }),
    );

  const pointAtHeld = async (heldId: string) => {
    setBusy(true);
    setTrouble(undefined);
    const said = await commands
      .yourKeyUseHeld(heldId, profile)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setBusy(false);
    if (said.status === "error") {
      setTrouble(windowCopy.W89({ error: said.error }));
      return;
    }
    if (box.current) box.current.value = "";
    onSaved(heldId);
  };

  return (
    <div className="key-form">
      <SettingRow
        label={windowCopy.W1}
        help="It is kept in your system keychain, read only by purlis, and sent only to Anthropic."
        control={(ids) => (
          <TextField
            inputRef={box}
            id={ids.id}
            className="ui-setting-box"
            type="password"
            isDisabled={busy}
            aria-labelledby={ids.labelledBy}
            aria-describedby={ids.describedBy}
          />
        )}
      />
      {trouble !== undefined && (
        <p className="trouble ui-setting-help" role="alert">
          {trouble}
        </p>
      )}
      {held !== undefined && counting === undefined && (
        <div className="key-form-held" role="group" aria-label={windowCopy.W2}>
          {held.mayReplace && <p className="ui-setting-help">{windowCopy.W9}</p>}
          <SettingActions>
            {held.mayReplace && (
              <Button isDisabled={busy} onPress={() => void askReplaceHeld(held.held)}>
                Replace the key…
              </Button>
            )}
            <Button isDisabled={busy} onPress={() => void pointAtHeld(held.held)}>
              {windowCopy.W2}
            </Button>
          </SettingActions>
        </div>
      )}
      {held !== undefined && counting !== undefined && (
        <div className="key-form-held" role="group" aria-label="Replace the key">
          <p className="ui-setting-help">{windowCopy.W5({ count: counting })}</p>
          <SettingActions>
            <Button isDisabled={busy} onPress={() => setCounting(undefined)}>
              Cancel
            </Button>
            <Button variant="danger" isDisabled={busy} onPress={() => replaceHeld(held.held)}>
              Replace the key
            </Button>
          </SettingActions>
        </div>
      )}
      {held === undefined && (
        <SettingActions>
          {onCancel !== undefined && (
            <Button isDisabled={busy} onPress={onCancel}>
              Cancel
            </Button>
          )}
          <Button isDisabled={busy} onPress={save}>
            <Reserved texts={["Save", "Try again", "Saving…"]}>
              {busy ? "Saving…" : trouble === undefined ? "Save" : "Try again"}
            </Reserved>
          </Button>
        </SettingActions>
      )}
    </div>
  );
}
