// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { listen } from "./here";
import { commands, type ForgeAccountRow } from "./bindings";
import { ForgeDeviceSignIn } from "./ForgeDeviceSignIn";
import { ForgeGitLabSignIn } from "./ForgeGitLabSignIn";
import { Notice } from "./Notice";

/** The event the core's refresh sends the window when a forge ended a sign-in. */
export const SIGN_IN_ENDED = "forge-sign-in-ended";

/** The event the core sends the window when a sign-in is not renewing, though no forge ended it. */
export const SIGN_IN_TROUBLE = "forge-sign-in-trouble";

/** What {@link SIGN_IN_TROUBLE} carries: the account's id and the kind of trouble. */
export type SignInTrouble = {
  account: string;
  trouble: "pin-changed" | "renewal-failing" | "everyday-holds-workflow" | "scopes-unknown";
};

/** The event a surface sends this window when it signed an account in or out. */
export const ACCOUNTS_CHANGED = "purlis:forge-accounts-changed";

/** Tells every surface of this window that lists forge accounts to read them again. */
export function forgeAccountsChanged(): void {
  window.dispatchEvent(new Event(ACCOUNTS_CHANGED));
}

/** The hosts this window has a browser sign-in on, by forge (ADR 0077 §1). */
const BROWSER_HOSTS: Readonly<Record<string, string>> = {
  github: "github.com",
  gitlab: "gitlab.com",
};

/** Whether `account` is a browser sign-in the forge ended, which this window can make again. */
function ended(account: ForgeAccountRow): boolean {
  return (
    !account.signed_in &&
    account.method === "browser" &&
    BROWSER_HOSTS[account.kind] === account.host
  );
}

/** What the Notice for an everyday token that holds `workflow` says. */
function WorkflowHeld({ account }: { account: ForgeAccountRow }) {
  return (
    <>
      GitHub added permission to push CI files (<code>workflow</code>) to purlis&apos;s sign-in to{" "}
      {account.host} as {account.login}, so purlis gives it to no chat. To fix it, revoke purlis at
      https://{account.host}/settings/applications, then sign in again without allowing CI files.
    </>
  );
}

/**
 * **A browser sign-in the forge ended, and the way back** (#729; ADR 0077 §3).
 *
 * A browser sign-in's token is renewed before it expires. When the forge refuses to renew it
 * (the sign-in was revoked there, or went six months unused), purlis marks the account signed
 * out and keeps it listed. This draws one Notice per such account, in the window's own list:
 * which sign-in ended, and **Sign in again**, which opens the same sign-in the Accounts page
 * does ({@link ForgeDeviceSignIn} for GitHub, {@link ForgeGitLabSignIn} for GitLab, #730). Dismiss hides it until the next launch; the account stays
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
  /** The troubles the core told of, until the account ends, is signed in again or one is dismissed. */
  const [troubles, setTroubles] = useState<readonly SignInTrouble[]>([]);
  /** The forge of the account being signed in to again. */
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
    // A surface here signed an account in or out: what was told of its trouble is the person's
    // to have acted on.
    const changed = () => {
      setTroubles([]);
      reread();
    };
    window.addEventListener(ACCOUNTS_CHANGED, changed);
    const listening = listen(SIGN_IN_ENDED, reread).catch(() => undefined);
    const hearing = listen<SignInTrouble>(SIGN_IN_TROUBLE, (heard) => {
      const { account, trouble } = heard.payload;
      setTroubles((was) =>
        was.some((t) => t.account === account && t.trouble === trouble)
          ? was
          : [...was, { account, trouble }],
      );
      reread();
    }).catch(() => undefined);
    return () => {
      window.removeEventListener(ACCOUNTS_CHANGED, changed);
      // The window may be gone before the listener is: nothing is left to stop then.
      void listening.then((stop) => stop?.()).catch(() => undefined);
      void hearing.then((stop) => stop?.()).catch(() => undefined);
    };
  }, [reread]);

  /** The accounts a trouble is drawn for: still listed, and still signed in. */
  const told = troubles.flatMap((trouble) => {
    const account = accounts.find((a) => a.id === trouble.account && a.signed_in);
    return account ? [{ account, trouble: trouble.trouble }] : [];
  });
  // An account whose everyday token holds `workflow` is read off the list, so the Notice outlives
  // the event and a restart: purlis serves such a token to no chat until it is signed in again
  // without it (ADR 0128 D1.1).
  const troubled = [
    ...told,
    ...accounts
      .filter(
        (a) =>
          a.signed_in &&
          a.kind === "github" &&
          (a.scopes ?? []).includes("workflow") &&
          !told.some((t) => t.account.id === a.id && t.trouble === "everyday-holds-workflow"),
      )
      .map((account) => ({ account, trouble: "everyday-holds-workflow" as const })),
  ];

  return (
    <>
      {accounts
        .filter((account) => ended(account) && !dismissed.includes(account.id))
        .map((account) => (
          <Notice
            key={account.id}
            cause={`forge-sign-in-ended:${account.id}`}
            tone="trouble"
            fixes={[{ label: "Sign in again", onPress: () => setAgain(account.kind) }]}
            onDismiss={() => setDismissed((was) => [...was, account.id])}
          >
            purlis&apos;s sign-in to {account.host} as {account.login} has ended, and {account.host}{" "}
            would not renew it. Until you sign in again, purlis cannot reach your repositories
            there.
          </Notice>
        ))}
      {troubled.map(({ account, trouble }) => (
        <Notice
          key={`${trouble}:${account.id}`}
          cause={`forge-sign-in-${trouble}:${account.id}`}
          tone="trouble"
          fixes={[{ label: "Sign in again", onPress: () => setAgain(account.kind) }]}
          onDismiss={() =>
            setTroubles((was) =>
              was.filter((t) => !(t.account === account.id && t.trouble === trouble)),
            )
          }
        >
          {trouble === "everyday-holds-workflow" ? (
            <WorkflowHeld account={account} />
          ) : trouble === "scopes-unknown" ? (
            <>
              {account.host} did not say what purlis&apos;s sign-in as {account.login} may do, so it
              may allow CI files (<code>workflow</code>) and purlis gives it to no chat. Sign in
              again to check it.
            </>
          ) : trouble === "pin-changed" ? (
            <>
              purlis stopped renewing its sign-in to {account.host} as {account.login}: the
              application pinned for {account.host} is not the one it signed in through. Until you
              check the application in Settings › You › Accounts and sign in again, the sign-in
              stops working when its token expires.
            </>
          ) : (
            <>
              purlis could not renew its sign-in to {account.host} as {account.login} several times
              in a row. It keeps trying and keeps the sign-in, which {account.host} has not ended.
              If it keeps failing, check your connection or sign in again.
            </>
          )}
        </Notice>
      ))}
      {again === "gitlab" && (
        <ForgeGitLabSignIn
          onCancel={() => setAgain(undefined)}
          onSignedIn={() => {
            setAgain(undefined);
            forgeAccountsChanged();
          }}
        />
      )}
      {again !== undefined && again !== "gitlab" && (
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
