// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { commands, type ForgeAccountRow } from "../bindings";
import { ForgeBrowserSignIn } from "../ForgeBrowserSignIn";
import { ACCOUNTS_CHANGED, forgeAccountsChanged } from "../ForgeSignInEnded";
import { ForgeTokenSignIn } from "../ForgeTokenSignIn";
import { ImportCliLogin } from "../ImportCliLogin";
import type { LiveSetting, SettingsGroup } from "./groups";
import { Button } from "../ui/button/Button";
import { Badge } from "../ui/badge/Badge";

/**
 * **You › Accounts** (#1814, ADR 0077 §6): this machine's forge accounts, each with Sign out;
 * **Sign in with GitHub** and **Sign in with GitLab**, the browser sign-ins on github.com and
 * gitlab.com ({@link ForgeBrowserSignIn}, #729, #730);
 * and **Sign in with a token…** ({@link ForgeTokenSignIn}). Both open the same dialogs the
 * welcome's "Sign in" does.
 *
 * An account holds for the whole machine (ADR 0077), so it is under You, beside This machine,
 * and reaches no project's files. The list is the core's (`forge_accounts`), read when the group
 * comes on screen and again after every sign-in and sign-out. A refusal is said in the row, in
 * the core's words.
 *
 * **Sign out** deletes the token from the keychain and takes the account off this machine. A
 * token the person made stays valid at the forge until they delete it there, which the row says
 * once it is done. A browser sign-in to gitlab.com is ended there too, and the row says so, or
 * says where to end it when GitLab could not be asked. One to GitHub cannot be revoked from
 * here (GitHub's revocation needs a secret purlis does not have, ADR 0077 §3): the row says it
 * lapses within eight hours and where on GitHub to revoke it now.
 *
 * **An account whose sign-in the forge ended** stays listed, as signed out: the window's Notice
 * says so with Sign in again ({@link ForgeSignInEnded}), and signing in here as the same login
 * signs that same account in.
 *
 * **Use your existing gh or glab login** ({@link ImportCliLogin}, #1376) is offered under the
 * list for a forge tool signed in where purlis has no account: never imported unless pressed,
 * and its cost to chats said first.
 */
export function accountsGroup(): SettingsGroup {
  return {
    id: "you.accounts",
    label: "Accounts",
    help: "The GitHub and GitLab accounts signed in on this machine. Their tokens are kept in your system keychain, never in a project, and never reach a chat. The forge work you do in this window uses them: the repo picker, discover, saves and the requests they open, and each workspace's request and checks. A host with no account here uses gh or glab, and chats always do.",
    settings: [forgeAccounts],
  };
}

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; accounts: ForgeAccountRow[] };

type Held = {
  read: Read;
  /** Why the last act was refused. */
  refused: string | undefined;
  /** What the last sign-out leaves for the person to do. */
  said: string | undefined;
  /** Whether the sign-in dialog is open. */
  signingIn: boolean;
};

const START: Held = {
  read: { state: "reading" },
  refused: undefined,
  said: undefined,
  signingIn: false,
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
    .forgeAccounts()
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (mine !== reading) return;
  set((was) => ({
    ...was,
    read:
      said.status === "error"
        ? { state: "trouble", trouble: said.error }
        : Array.isArray(said.data)
          ? { state: "read", accounts: said.data }
          : { state: "trouble", trouble: "purlis did not say which accounts it holds." },
  }));
}

/** Read again when another surface of this window signed an account in or out. */
const rereadWhenChanged = () => void reread();

function subscribe(listener: () => void) {
  if (listeners.size === 0) {
    held = START;
    void reread();
    window.addEventListener(ACCOUNTS_CHANGED, rereadWhenChanged);
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) window.removeEventListener(ACCOUNTS_CHANGED, rereadWhenChanged);
  };
}

/** Opens the sign-in dialog: what Settings' button does. */
function openSignIn() {
  set((was) => ({ ...was, signingIn: true, refused: undefined, said: undefined }));
}

async function signOut(account: ForgeAccountRow) {
  set((was) => ({ ...was, refused: undefined, said: undefined }));
  const done = await commands
    .forgeSignOut(account.id)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  set((was) => ({
    ...was,
    refused: done.status === "error" ? done.error : undefined,
    said: done.status === "error" ? undefined : signedOutSays(account, done.data),
  }));
  await reread();
  forgeAccountsChanged();
}

/**
 * What a sign-out leaves for the person to do at the forge, which purlis cannot do for them.
 * `ended` is the core's word on a browser sign-in's token there: `true` when the forge ended
 * it, `false` when it was asked and did not, nothing when no forge was asked.
 */
function signedOutSays(account: ForgeAccountRow, ended: boolean | null): string {
  const gone = `Signed out of ${account.login} on ${account.host}.`;
  if (account.method !== "browser")
    return `${gone} The token still works on ${account.host} until you delete it there.`;
  const deleted = `${gone} purlis deleted its sign-in from your keychain`;
  if (account.kind === "github")
    return `${deleted}. GitHub lets it lapse within eight hours; to end it now, revoke purlis at https://${account.host}/settings/applications.`;
  if (ended === true) return `${deleted}, and ${account.host} ended its token.`;
  if (ended === false)
    return `${deleted}, but ${account.host} could not be asked to end its token. To end it now, revoke purlis at https://${account.host}/-/user_settings/applications.`;
  return `${deleted}.`;
}

/** What the row says about an account beside its name. */
function note(account: ForgeAccountRow): string {
  if (!account.signed_in) return "signed out";
  const how = account.method === "token" ? "token" : account.method;
  return account.wider.length > 0 ? `${how}; wider than needed: ${account.wider.join(", ")}` : how;
}

const forgeAccounts: LiveSetting = {
  id: "you.accounts.forges",
  label: "Forge accounts",
  help: "Sign in to github.com or gitlab.com through your browser, or with a token on either, on a GitHub Enterprise server or on a self-managed GitLab. Sign out deletes the sign-in from your keychain.",
  useControl: function useForgeAccounts() {
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
            (now.read.accounts.length === 0 ? (
              <p className="ui-setting-help">No forge account is signed in on this machine.</p>
            ) : (
              <ul className="ui-machine-list">
                {now.read.accounts.map((account) => (
                  <li key={account.id} className="ui-machine-entry">
                    <span className="ui-machine-name">{account.login}</span>
                    <span className="ui-machine-path">{account.host}</span>
                    <Badge variant="accent">{note(account)}</Badge>
                    <Button
                      size="sm"
                      aria-label={`Sign out of ${account.login} on ${account.host}`}
                      onPress={() => void signOut(account)}
                    >
                      Sign out
                    </Button>
                  </li>
                ))}
              </ul>
            ))}
          {now.said !== undefined && (
            <p className="ui-setting-help" role="status">
              {now.said}
            </p>
          )}
          <ForgeBrowserSignIn
            onSignedIn={() => {
              set((was) => ({ ...was, refused: undefined, said: undefined }));
              void reread();
              forgeAccountsChanged();
            }}
          />
          <Button onPress={openSignIn}>Sign in with a token…</Button>
          {/* An existing gh or glab login, offered and never taken (#1376, ADR 0077 §5). The
              list is read again once one is imported. */}
          <ImportCliLogin onImported={() => void reread()} />
          {now.signingIn && (
            <ForgeTokenSignIn
              onCancel={() => set((was) => ({ ...was, signingIn: false }))}
              onSignedIn={() => {
                set((was) => ({ ...was, signingIn: false }));
                void reread();
                forgeAccountsChanged();
              }}
            />
          )}
        </div>
      ),
    };
  },
};
