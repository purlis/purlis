// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type SigningForge, type SigningState } from "../bindings";
import { ACCOUNTS_CHANGED } from "../ForgeSignInEnded";
import { cantVerify, SigningSetupDialog } from "../SigningSetupDialog";
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
 * **Signed commits** in You › Accounts (#5628, #5629, spec #5607, ADR 0116): the lasting place
 * to set signing up and see where it holds.
 *
 * - **Not set up**: two sentences on what it is and that the key stays on this Mac, and
 *   **Set up signing**, which opens {@link SigningSetupDialog} for the first account whose forge
 *   verifies signed commits. With no forge account signed in, it says to sign in first, above. A
 *   GitLab too old to verify signed commits says so, and is offered no setup.
 * - **On**, exactly while this Mac has a signing key (ADR 0116 §1): a row per forge account,
 *   GitHub and GitLab alike, with *Verified on <forge> ✓* and the email it commits as once a
 *   Check in this window found it so, or **Check** where none has yet.
 * - **Details**: the key type, its fingerprint, where it is kept and how to use your own key
 *   from Terminal. The only place the card names SSH or Ed25519 (ADR 0116 §8).
 */
export const signedCommits: LiveSetting = {
  id: "you.accounts.signing",
  label: "Signed commits",
  help: "When it's on, the commits purlis makes for your chats carry a signature, and GitHub and GitLab show them as Verified. Some repos only accept signed commits.",
  useControl: function useSignedCommits() {
    const [read, setRead] = useState<Read>({ state: "reading" });
    /** The dialog, open for an account (or the first that verifies, `null`), or closed. */
    const [open, setOpen] = useState<SigningForge | null | false>(false);
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
          {read.state === "read" && <Signing signing={read.signing} onSetUp={setOpen} />}
          {open !== false && (
            <SigningSetupDialog
              forge={open}
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

function Signing({
  signing,
  onSetUp,
}: {
  signing: SigningState;
  /** Opens setup for that account, or for the first that verifies (`null`). */
  onSetUp: (forge: SigningForge | null) => void;
}) {
  const accounts = signing.accounts;
  const cannot = accounts.filter((account) => !account.verifies);
  const cannotSay = cannot.map((account) => (
    <p className="ui-setting-help" key={`${account.kind}:${account.host}`}>
      {cantVerify(account)}
    </p>
  ));
  if (!signing.on) {
    return (
      <>
        <p className="ui-setting-help">
          <Badge variant="neutral">Not set up</Badge>
        </p>
        <p className="ui-setting-help">
          Your signing key is made on this Mac and stays in its Keychain. No chat can reach it.
        </p>
        {accounts.length === 0 ? (
          <p className="ui-setting-help">
            Sign in to GitHub or GitLab above first, then set up signing.
          </p>
        ) : (
          <>
            {cannot.length < accounts.length && (
              <SettingActions>
                <Button variant="primary" onPress={() => onSetUp(null)}>
                  Set up signing
                </Button>
              </SettingActions>
            )}
            {cannotSay}
          </>
        )}
      </>
    );
  }
  return (
    <>
      <p className="ui-setting-help">
        <Badge variant="ok">On</Badge> Chat commits on this Mac are signed with its signing key.
      </p>
      {accounts.length > 0 ? (
        <ul className="ui-machine-list">
          {accounts.map((account) => (
            <li className="ui-machine-entry" key={`${account.kind}:${account.host}`}>
              <span className="ui-machine-name">{account.login}</span>
              <span className="ui-machine-path">{account.host}</span>
              {!account.verifies ? (
                <span className="ui-setting-help">{cantVerify(account)}</span>
              ) : account.verified_as ? (
                <>
                  <Badge variant="ok">Verified on {account.forge} ✓</Badge>
                  <span className="ui-setting-help">Commits as {account.verified_as}</span>
                </>
              ) : (
                <Button
                  size="sm"
                  aria-label={`Check signing on ${account.login} on ${account.forge}`}
                  onPress={() => onSetUp({ kind: account.kind, host: account.host })}
                >
                  Check
                </Button>
              )}
            </li>
          ))}
        </ul>
      ) : (
        <p className="ui-setting-help">
          Sign in to GitHub or GitLab above to add the key to your account.
        </p>
      )}
      <Details title={signing.title} />
    </>
  );
}

/**
 * **Details**: the only place the card names SSH or Ed25519. The key's fingerprint is read from
 * the Keychain only once Details is opened, so the card itself never makes macOS ask.
 */
function Details({ title }: { title: string }) {
  const [fingerprint, setFingerprint] = useState<string>();
  const opened = (open: boolean) => {
    if (!open || fingerprint !== undefined) return;
    void commands.signingKeyShown().then((shown) => {
      if (shown.status === "ok") setFingerprint(shown.data.fingerprint);
    });
  };
  return (
    <Disclosure label="Details" onExpandedChange={opened}>
      <dl className="signing-details">
        <dt>Key</dt>
        <dd>Ed25519 SSH key{fingerprint ? `, ${fingerprint}` : ""}</dd>
        <dt>Kept in</dt>
        <dd>This Mac&apos;s Keychain, as purlis/@commit-signing</dd>
        <dt>Named on the forge</dt>
        <dd>{title}</dd>
        <dt>Your own key</dt>
        <dd>
          To use a key you already have, run <code>purlis signing setup --key</code> in Terminal.
        </dd>
      </dl>
    </Disclosure>
  );
}
