// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { listen } from "./here";
import { commands, type ForgeAccountRow } from "./bindings";
import { ForgeDeviceSignIn } from "./ForgeDeviceSignIn";
import { Notice } from "./Notice";

/** The event the core's refresh sends the window when a forge ended a sign-in. */
export const SIGN_IN_ENDED = "forge-sign-in-ended";

/** The event a surface sends this window when it signed an account in or out. */
export const ACCOUNTS_CHANGED = "purlis:forge-accounts-changed";

/** Tells every surface of this window that lists forge accounts to read them again. */
export function forgeAccountsChanged(): void {
  window.dispatchEvent(new Event(ACCOUNTS_CHANGED));
}

/** Whether `account` is a browser sign-in the forge ended, which this window can make again. */
function ended(account: ForgeAccountRow): boolean {
  return !account.signed_in && account.method === "browser" && account.kind === "github";
}

/**
 * **A browser sign-in the forge ended, and the way back** (#729; ADR 0077 §3).
 *
 * A browser sign-in's token is renewed before it expires. When the forge refuses to renew it
 * (the sign-in was revoked there, or went six months unused), purlis marks the account signed
 * out and keeps it listed. This draws one Notice per such account, in the window's own list:
 * which sign-in ended, and **Sign in again**, which opens the same sign-in the Accounts page
 * does ({@link ForgeDeviceSignIn}). Dismiss hides it until the next launch; the account stays
 * listed as signed out in Settings › You › Accounts either way.
 *
 * The accounts are the core's (`forge_accounts`), read when the window opens, when the core says
 * a sign-in ended ({@link SIGN_IN_ENDED}), and when a surface here signed one in or out
 * ({@link ACCOUNTS_CHANGED}). A list that cannot be read draws nothing: the Accounts page says
 * why.
 */
export function ForgeSignInEnded() {
  const [accounts, setAccounts] = useState<ForgeAccountRow[]>([]);
  const [dismissed, setDismissed] = useState<readonly string[]>([]);
  /** The account being signed in to again. */
  const [again, setAgain] = useState<string>();

  const reread = useCallback(() => {
    void commands
      .forgeAccounts()
      .then((said) => {
        if (said.status === "ok" && Array.isArray(said.data)) setAccounts(said.data);
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    reread();
    window.addEventListener(ACCOUNTS_CHANGED, reread);
    const listening = listen(SIGN_IN_ENDED, reread).catch(() => undefined);
    return () => {
      window.removeEventListener(ACCOUNTS_CHANGED, reread);
      // The window may be gone before the listener is: nothing is left to stop then.
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, [reread]);

  return (
    <>
      {accounts
        .filter((account) => ended(account) && !dismissed.includes(account.id))
        .map((account) => (
          <Notice
            key={account.id}
            cause={`forge-sign-in-ended:${account.id}`}
            tone="trouble"
            fixes={[{ label: "Sign in again", onPress: () => setAgain(account.id) }]}
            onDismiss={() => setDismissed((was) => [...was, account.id])}
          >
            purlis&apos;s sign-in to {account.host} as {account.login} has ended, and {account.host}{" "}
            would not renew it. Until you sign in again, purlis cannot reach your repositories
            there.
          </Notice>
        ))}
      {again !== undefined && (
        <ForgeDeviceSignIn
          onCancel={() => setAgain(undefined)}
          onSignedIn={() => {
            setAgain(undefined);
            forgeAccountsChanged();
          }}
        />
      )}
    </>
  );
}
