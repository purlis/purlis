// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { commands, type ForgeAccountRow } from "../bindings";
import { ForgeTokenSignIn } from "../ForgeTokenSignIn";
import { ImportCliLogin } from "../ImportCliLogin";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Accounts** (#1814, ADR 0077 §6): this machine's forge accounts, each with Sign out,
 * and **Sign in with a token…**, which opens the same dialog the welcome's "Sign in" does
 * ({@link ForgeTokenSignIn}).
 *
 * An account holds for the whole machine (ADR 0077), so it is under You, beside This machine,
 * and reaches no project's files. The list is the core's (`forge_accounts`), read when the group
 * comes on screen and again after every sign-in and sign-out. A refusal is said in the row, in
 * the core's words.
 *
 * **Sign out** deletes the token from the keychain and takes the account off this machine. A
 * token the person made stays valid at the forge until they delete it there, which the row says
 * once it is done.
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
    said:
      done.status === "error"
        ? undefined
        : `Signed out of ${account.login} on ${account.host}. The token still works on ${account.host} until you delete it there.`,
  }));
  await reread();
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
  help: "Sign in with a token on github.com, gitlab.com, a GitHub Enterprise server or a self-managed GitLab. Sign out deletes the token from your keychain.",
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
                    <span className="ui-setting-badge">{note(account)}</span>
                    <button
                      type="button"
                      className="ui-setting-reset"
                      // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
                      tabIndex={0}
                      aria-label={`Sign out of ${account.login} on ${account.host}`}
                      onClick={() => void signOut(account)}
                    >
                      Sign out
                    </button>
                  </li>
                ))}
              </ul>
            ))}
          {now.said !== undefined && (
            <p className="ui-setting-help" role="status">
              {now.said}
            </p>
          )}
          <button type="button" tabIndex={0} onClick={openSignIn}>
            Sign in with a token…
          </button>
          {/* An existing gh or glab login, offered and never taken (#1376, ADR 0077 §5). The
              list is read again once one is imported. */}
          <ImportCliLogin onImported={() => void reread()} />
          {now.signingIn && (
            <ForgeTokenSignIn
              onCancel={() => set((was) => ({ ...was, signingIn: false }))}
              onSignedIn={() => {
                set((was) => ({ ...was, signingIn: false }));
                void reread();
              }}
            />
          )}
        </div>
      ),
    };
  },
};
