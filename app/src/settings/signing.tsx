// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type SigningState } from "../bindings";
import { ACCOUNTS_CHANGED } from "../ForgeSignInEnded";
import { SigningSetupDialog } from "../SigningSetupDialog";
import type { LiveSetting } from "./groups";
import { SettingActions } from "./components";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Disclosure } from "../ui/disclosure/Disclosure";

type Read =
  | { state: "reading" }
  | { state: "trouble"; why: string }
  | { state: "read"; signing: SigningState };

/**
 * **Signed commits** in You › Accounts (#5628, spec #5607, ADR 0116): the lasting place to set
 * signing up and see where it holds.
 *
 * - **Not set up**: two sentences on what it is and that the key stays on this Mac, and
 *   **Set up signing**, which opens {@link SigningSetupDialog}. With no GitHub account signed in,
 *   it says to sign in first, above.
 * - **On**, exactly while this Mac has a signing key (ADR 0116 §1): the GitHub account's row,
 *   with *Verified on GitHub ✓* and the email it commits as once a Check in this window found it
 *   so, or **Check** where none has yet.
 * - **Details**: the key type, its fingerprint, where it is kept and how to use your own key
 *   from Terminal. The only place the card names SSH or Ed25519 (ADR 0116 §8).
 */
export const signedCommits: LiveSetting = {
  id: "you.accounts.signing",
  label: "Signed commits",
  help: "When it's on, the commits purlis makes for your chats carry a signature, and GitHub shows them as Verified. Some repos only accept signed commits.",
  useControl: function useSignedCommits() {
    const [read, setRead] = useState<Read>({ state: "reading" });
    const [open, setOpen] = useState(false);
    const reread = useCallback(() => {
      void commands.signingState().then((got) => {
        // A core with nothing to say yet (a test's, answering every command with nothing)
        // leaves the card reading.
        if (got.status === "ok" && !got.data) return;
        setRead(
          got.status === "ok"
            ? { state: "read", signing: got.data }
            : { state: "trouble", why: got.error },
        );
      });
    }, []);
    useEffect(() => {
      reread();
      window.addEventListener(ACCOUNTS_CHANGED, reread);
      return () => window.removeEventListener(ACCOUNTS_CHANGED, reread);
    }, [reread]);
    return {
      grouped: true,
      control: (ids) => (
        <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
          {read.state === "reading" && <p className="ui-setting-help">Reading…</p>}
          {read.state === "trouble" && (
            <p className="ui-setting-help" role="alert">
              {read.why}
            </p>
          )}
          {read.state === "read" && (
            <Signing signing={read.signing} onSetUp={() => setOpen(true)} />
          )}
          {open && (
            <SigningSetupDialog
              onClose={() => {
                setOpen(false);
                reread();
              }}
            />
          )}
        </div>
      ),
    };
  },
};

function Signing({ signing, onSetUp }: { signing: SigningState; onSetUp: () => void }) {
  const account = signing.account;
  if (!signing.on) {
    return (
      <>
        <p className="ui-setting-help">
          <Badge variant="neutral">Not set up</Badge>
        </p>
        <p className="ui-setting-help">
          Your signing key is made on this Mac and stays in its Keychain. No chat can reach it.
        </p>
        {account ? (
          <SettingActions>
            <Button variant="primary" onPress={onSetUp}>
              Set up signing
            </Button>
          </SettingActions>
        ) : (
          <p className="ui-setting-help">Sign in to GitHub above first, then set up signing.</p>
        )}
      </>
    );
  }
  return (
    <>
      <p className="ui-setting-help">
        <Badge variant="ok">On</Badge> Chat commits on this Mac are signed with its signing key.
      </p>
      {account ? (
        <ul className="ui-machine-list">
          <li className="ui-machine-entry">
            <span className="ui-machine-name">{account.login}</span>
            <span className="ui-machine-path">{account.host}</span>
            {account.verified_as ? (
              <>
                <Badge variant="ok">Verified on GitHub ✓</Badge>
                <span className="ui-setting-help">Commits as {account.verified_as}</span>
              </>
            ) : (
              <Button size="sm" aria-label={`Check signing on ${account.login}`} onPress={onSetUp}>
                Check
              </Button>
            )}
          </li>
        </ul>
      ) : (
        <p className="ui-setting-help">Sign in to GitHub above to add the key to your account.</p>
      )}
      <Disclosure label="Details">
        <dl className="signing-details">
          <dt>Key</dt>
          <dd>Ed25519 SSH key{signing.fingerprint ? `, ${signing.fingerprint}` : ""}</dd>
          <dt>Kept in</dt>
          <dd>This Mac&apos;s Keychain, as purlis/@commit-signing</dd>
          <dt>Named on GitHub</dt>
          <dd>{signing.title}</dd>
          <dt>Your own key</dt>
          <dd>
            To use a key you already have, run <code>purlis signing setup --key</code> in Terminal.
          </dd>
        </dl>
      </Disclosure>
    </>
  );
}
