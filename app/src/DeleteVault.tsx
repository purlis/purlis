// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Field, SettingRow } from "./settings/components";
import { AlertDialog } from "./ui/dialog/Dialog";

/** What a vault holds, as `vault_open` said it: its provider and its secrets' NAMES. */
export type VaultHolds = { provider: string; secrets: readonly string[] };

/**
 * Deleting a vault (SI-3): what it holds, what happens to each secret, and the vault's name
 * typed back before anything is deleted.
 *
 * **The operator's ruling, in its three parts.** The dialog lists the secrets the vault holds; it
 * says plainly that a keychain vault's entries are destroyed and cannot be recovered; and the
 * delete is gated on the vault's name typed exactly. What deletes is `vault_remove`, which is
 * `vaultcmd::destroy` in the core: every keyring entry, then the keys index, then the
 * registration — and an entry the keyring refuses stops it with the vault still registered.
 *
 * **It says only what is true of the provider.** A keychain is charter's store, so its entries
 * go. A plain file, a references file and a 1Password item are the operator's, and the delete
 * leaves them where they are — the dialog says so rather than promise a destruction that does
 * not happen.
 *
 * **The names are read before, never the values.** `vault_open` answers with names and sizes
 * only (`vaults.rs`), and a name is the one thing the operator needs to recognise what is about
 * to go.
 *
 * Radix's `AlertDialog`, for `DeleteWorkspace`'s reason: it is the primitive for a question whose
 * answer destroys something, its content is `role="alertdialog"`, and Escape is Cancel. The
 * keyboard lands in the name box, because typing is the answer this dialog asks for, and a stray
 * Return there deletes nothing until the name is right.
 *
 * **It is a question, so it ends in the answer bar** (D-1210-8). Its one field is the typed name
 * of what it ends: that confirms an answer, it makes or changes nothing, so the dialog ends as
 * `DeleteWorkspace` does — Cancel first, the delete last at the trailing edge — and not in a
 * form's `SettingActions` (V89j), which ends the forms that make or change something.
 * `dialogAnswers.guard.test.ts` keeps a `danger` act out of any dialog's form row.
 */
export function DeleteVault({
  vault,
  holds,
  unreadable,
  trouble,
  deleting,
  onDelete,
  onCancel,
}: {
  vault: string;
  /** What it holds, or `undefined` while charter is still reading it. Drawn as "reading", never
   *  as an empty vault: "nothing to lose" is a claim, and charter has not made it yet. */
  holds?: VaultHolds;
  /** Why charter could not read what it holds. The delete is still offered: the core reads
   *  the vault again when it deletes, and refuses on its own reading. */
  unreadable?: string;
  /** The last delete's refusal, in the core's words. */
  trouble?: string;
  /** Whether a delete is running right now, so the answer cannot be given twice. */
  deleting: boolean;
  onDelete: () => void;
  onCancel: () => void;
}) {
  const [typed, setTyped] = useState("");
  const ready = typed === vault && !deleting;
  // The keyboard starts in the box the name is typed back into. A question's footer (D-1210-8):
  // the way out first, the delete last at the trailing edge. Return in the box deletes once the
  // name is typed exactly, and not before (D-1210-9).
  return (
    <AlertDialog
      onClose={() => {
        if (!deleting) onCancel();
      }}
      title={`Delete vault ${vault}?`}
      description={<p className="came-back">{fate(holds)}</p>}
      initialFocus="field"
      cancel={{ isDisabled: deleting, onPress: onCancel }}
      main={{
        children: deleting ? "Deleting…" : "Delete vault",
        reserve: ["Delete vault", "Deleting…"],
        variant: "danger",
        isDisabled: !ready,
        byReturn: true,
        onPress: onDelete,
      }}
    >
      {holds === undefined && unreadable === undefined && (
        <p className="pending">Reading what it holds…</p>
      )}
      {holds === undefined && unreadable !== undefined && (
        <p className="trouble" role="alert">
          purlis could not read what it holds: {unreadable}. Delete reads it again, and a keychain
          vault's secrets are still destroyed.
        </p>
      )}

      {holds !== undefined && holds.secrets.length > 0 && (
        <>
          <h3>What it holds</h3>
          <ul className="at-risk" aria-label={`Secrets in ${vault}`}>
            {holds.secrets.map((key) => (
              <li key={key}>{key}</li>
            ))}
          </ul>
        </>
      )}

      <SettingRow
        label={`Type ${vault} to confirm`}
        control={(ids) => <Field ids={ids} kind="text" value={typed} onChange={setTyped} />}
      />

      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </AlertDialog>
  );
}

/** What deleting does to the secrets, for the provider this vault is kept by. */
function fate(holds: VaultHolds | undefined): string {
  if (holds === undefined) {
    return "purlis forgets this vault. A keychain vault's secrets are destroyed with it and cannot be recovered.";
  }
  const n = holds.secrets.length;
  const secrets = `${n} ${n === 1 ? "secret" : "secrets"}`;
  switch (holds.provider) {
    case "keyring":
      return n === 0
        ? "purlis forgets this vault. It holds no secrets in your system keychain."
        : `Its ${secrets} are destroyed in your system keychain and cannot be recovered. Then purlis forgets the vault.`;
    case "1password":
      return "purlis forgets this vault. Its item in 1Password is left alone — delete it there if it should go.";
    default:
      return "purlis forgets this vault. Its file is left on disk, where you can delete it yourself.";
  }
}
