// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useCallback, useEffect, useId, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  commands,
  type SigningAccountKeys,
  type SigningDone,
  type SigningForge,
  type SigningOtherKey,
  type SigningState,
} from "../bindings";
import { ACCOUNTS_CHANGED } from "../ForgeSignInEnded";
import { cantVerify, SigningSetupDialog } from "../SigningSetupDialog";
import { SIGNING_WAITING_CHANGED } from "../signingAsked";

/** The lanes and the chats waiting on signing read it again: it was set up, replaced or removed. */
const tellTheLanes = () => window.dispatchEvent(new Event(SIGNING_WAITING_CHANGED));
import type { LiveSetting } from "./groups";
import { SettingActions } from "./components";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Dialog } from "../ui/dialog/Dialog";
import { Disclosure } from "../ui/disclosure/Disclosure";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "../ui/table/Table";

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
 *   Check in this window found it so, **Check** where none has yet, or **Add to this account**
 *   where the account hasn't the key (#5632). Then the account's purlis keys from the person's
 *   other devices, under a heading and a line saying what removing one does (#5918): each by its
 *   device, the day it was added and *Older setup* where an older setup titled it, with a quiet
 *   **Remove…** that asks first, naming the device (or the forge's keys page, where the sign-in
 *   may not remove one); **Replace key**; and **Remove signing**, after one confirmation naming
 *   the accounts. There is no switch: on means this Mac has a key.
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
          {read.state === "reading" && (
            <p className="ui-setting-help ui-text-secondary">Reading…</p>
          )}
          {read.state === "trouble" && (
            <p className="ui-setting-help ui-text-secondary" role="alert">
              {read.why}
            </p>
          )}
          {read.state === "read" && (
            <Signing signing={read.signing} onSetUp={setOpen} onChanged={reread} />
          )}
          {open !== false && (
            <SigningSetupDialog
              forge={open}
              onClose={() => {
                setOpen(false);
                reread();
                tellTheLanes();
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
  onChanged,
}: {
  signing: SigningState;
  /** Opens setup for that account, or for the first that verifies (`null`). */
  onSetUp: (forge: SigningForge | null) => void;
  /** Read the card again: the key changed or went. */
  onChanged: () => void;
}) {
  const accounts = signing.accounts;
  const cannot = accounts.filter((account) => !account.verifies);
  const cannotSay = cannot.map((account) => (
    <p className="ui-setting-help ui-text-secondary" key={`${account.kind}:${account.host}`}>
      {cantVerify(account)}
    </p>
  ));
  if (!signing.on) {
    return (
      <>
        <p className="ui-setting-help ui-text-secondary">
          <Badge variant="neutral">Not set up</Badge>
        </p>
        <p className="ui-setting-help ui-text-secondary">
          Your signing key is made on this Mac and stays in its Keychain. No chat can reach it.
        </p>
        {accounts.length === 0 ? (
          <p className="ui-setting-help ui-text-secondary">
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
  return <On signing={signing} onSetUp={onSetUp} onChanged={onChanged} />;
}

/** What each account holds of the person's keys, as read once the card is on. */
type Keys =
  { at: "reading" } | { at: "read"; keys: readonly SigningAccountKeys[] } | { at: "unread" };

/** What the last Replace key or Remove signing came to. */
type Did = { what: "replaced" | "removed"; done: readonly SigningDone[] } | { why: string };

const keyOf = (one: { kind: string; host: string }) => `${one.kind}:${one.host}`;

/** **The card, on** (#5632): account rows, the other devices' keys, Replace key, Remove signing. */
function On({
  signing,
  onSetUp,
  onChanged,
}: {
  signing: SigningState;
  onSetUp: (forge: SigningForge | null) => void;
  onChanged: () => void;
}) {
  const [keys, setKeys] = useState<Keys>({ at: "reading" });
  const [asked, setAsked] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [did, setDid] = useState<Did>();
  useEffect(() => {
    let live = true;
    void commands
      .signingAccountsKeys()
      .then((read) => {
        if (!live) return;
        setKeys(
          read.status === "ok" && Array.isArray(read.data)
            ? { at: "read", keys: read.data }
            : { at: "unread" },
        );
      })
      .catch(() => live && setKeys({ at: "unread" }));
    return () => {
      live = false;
    };
  }, [asked, signing]);
  const keysOf = (one: { kind: string; host: string }) =>
    keys.at === "read" ? keys.keys.find((held) => keyOf(held) === keyOf(one)) : undefined;
  const accounts = signing.accounts;
  const others = accounts.flatMap((account) =>
    (keysOf(account)?.others ?? []).map((other) => ({ account, other })),
  );
  const holding = accounts.filter((account) => keysOf(account)?.has_key === true);
  const act = (what: "replaced" | "removed") => {
    setBusy(true);
    setConfirming(false);
    const run =
      what === "replaced" ? commands.signingReplaceKey() : commands.signingRemoveEverywhere();
    void run.then((done) => {
      setBusy(false);
      setDid(done.status === "ok" ? { what, done: done.data } : { why: done.error });
      setAsked((was) => was + 1);
      onChanged();
      tellTheLanes();
    });
  };
  return (
    <>
      <p className="ui-setting-help ui-text-secondary">
        <Badge variant="ok">On</Badge> Chat commits on this Mac are signed with its signing key.
      </p>
      {accounts.length > 0 ? (
        // **A table of the accounts' signing** (#6301, as Your hosts is, #2919 R65): a row an
        // account, its login heading it, its host, and where its signing stands.
        <Table className="ui-record-table" aria-label="Signing on your accounts" stacks>
          <TableHeader>
            <TableColumn id="login" isRowHeader>
              Account
            </TableColumn>
            <TableColumn id="host">Host</TableColumn>
            <TableColumn id="signing">Signing</TableColumn>
          </TableHeader>
          <TableBody>
            {accounts.map((account) => {
              const held = keysOf(account);
              const forge = { kind: account.kind, host: account.host };
              return (
                <TableRow key={keyOf(account)} id={keyOf(account)} textValue={account.login}>
                  <TableCell>{account.login}</TableCell>
                  <TableCell heading="Host">
                    <span className="ui-record-where ui-text-meta">{account.host}</span>
                  </TableCell>
                  <TableCell heading="Signing">
                    {!account.verifies ? (
                      <span className="ui-setting-help ui-text-secondary">
                        {cantVerify(account)}
                      </span>
                    ) : held?.has_key === false ? (
                      <Button
                        size="sm"
                        aria-label={`Add to this account: ${account.login} on ${account.forge}`}
                        onPress={() => onSetUp(forge)}
                      >
                        Add to this account
                      </Button>
                    ) : account.verified_as ? (
                      <span className="signing-verified">
                        <Badge variant="ok">Verified on {account.forge} ✓</Badge>
                        <span className="ui-setting-help ui-text-secondary">
                          Commits as {account.verified_as}
                        </span>
                      </span>
                    ) : (
                      <Button
                        size="sm"
                        aria-label={`Check signing on ${account.login} on ${account.forge}`}
                        onPress={() => onSetUp(forge)}
                      >
                        Check
                      </Button>
                    )}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      ) : (
        <p className="ui-setting-help ui-text-secondary">
          Sign in to GitHub or GitLab above to add the key to your account.
        </p>
      )}
      {others.length > 0 && (
        <OtherKeys>
          {others.map(({ account, other }) => (
            <OtherKey
              key={`${keyOf(account)}:${other.id}`}
              other={other}
              where={`${account.login} on ${account.forge}`}
              forge={{ kind: account.kind, host: account.host }}
              mayRemove={keysOf(account)?.may_remove === true}
              keysPage={keysOf(account)?.keys_page ?? ""}
              onRemoved={() => setAsked((was) => was + 1)}
            />
          ))}
        </OtherKeys>
      )}
      <SettingActions>
        <Button isDisabled={busy} onPress={() => act("replaced")}>
          Replace key
        </Button>
        {/* Opens the one confirmation, whose own Remove signing is the act. */}
        <Button isDisabled={busy} onPress={() => setConfirming(true)}>
          Remove signing…
        </Button>
      </SettingActions>
      {did !== undefined && <DidSaid did={did} />}
      <Details title={signing.title} />
      {confirming && (
        <RemoveSigningConfirm
          accounts={holding}
          onClose={() => setConfirming(false)}
          onRemove={() => act("removed")}
        />
      )}
    </>
  );
}

/**
 * **Remove signing's one confirmation** (ADR 0116 §7): the accounts it is removed from, and that
 * commits already signed stay signed while repos that require signatures refuse new ones.
 */
export function RemoveSigningConfirm({
  accounts,
  onClose,
  onRemove,
}: {
  accounts: readonly { kind: string; host: string; login: string; forge: string }[];
  onClose: () => void;
  onRemove: () => void;
}) {
  return (
    <Dialog
      onClose={onClose}
      title="Remove signing from this Mac?"
      subtitle="purlis deletes the signing key from this Mac's Keychain and from every account it was added to."
      main={{ children: "Remove signing", variant: "danger", onPress: onRemove }}
    >
      {accounts.length > 0 ? (
        <ul aria-label="Accounts it is removed from">
          {accounts.map((account) => (
            <li key={keyOf(account)}>
              {account.login} on {account.forge} ({account.host})
            </li>
          ))}
        </ul>
      ) : (
        <p>No account lists this Mac&apos;s key, so only this Mac&apos;s Keychain changes.</p>
      )}
      <p>
        Commits already signed stay as they are. New chat commits won&apos;t be signed, and repos
        that only accept signed commits will refuse them until you set signing up again.
      </p>
    </Dialog>
  );
}

/**
 * **The other devices' keys** (#5918): a heading, one line on what removing one does, and the
 * table it names (#6301, as Your hosts is, #2919 R65, R67): a row a key, its device heading it,
 * the account it is on, the day it was added, and its Remove… at the row's end.
 */
function OtherKeys({ children }: { children: ReactNode }) {
  const heading = useId();
  return (
    <section className="signing-others" aria-labelledby={heading}>
      <h3 id={heading} className="ui-text-heading">
        Signing keys of your other devices
      </h3>
      <p className="ui-setting-help ui-text-secondary">
        Each signs that device&apos;s chat commits. Removing one here stops that device&apos;s
        commits showing as Verified until it sets up signing again.
      </p>
      <Table aria-labelledby={heading} stacks>
        <TableHeader>
          <TableColumn id="device" isRowHeader>
            Device
          </TableColumn>
          <TableColumn id="account">Account</TableColumn>
          <TableColumn id="added">Added</TableColumn>
          <TableColumn id="remove" align="end" hideName>
            Remove
          </TableColumn>
        </TableHeader>
        <TableBody>{children}</TableBody>
      </Table>
    </section>
  );
}

/** The day a key was added (`yyyy-mm-dd`), in the reader's own words for a date. */
export function addedOn(day: string): string {
  const [year, month, date] = day.split("-").map(Number);
  return new Date(year, month - 1, date).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

/**
 * Another device's purlis key on an account (#5918): its device, *Older setup* where an older
 * setup titled it, the account and the day added, with a quiet **Remove…** that asks first, or
 * the forge's keys page.
 */
function OtherKey({
  other,
  where,
  forge,
  mayRemove,
  keysPage,
  onRemoved,
}: {
  other: SigningOtherKey;
  where: string;
  forge: SigningForge;
  mayRemove: boolean;
  keysPage: string;
  onRemoved: () => void;
}) {
  const [why, setWhy] = useState<string>();
  const [confirming, setConfirming] = useState(false);
  const device = other.device;
  return (
    <TableRow id={`${where}:${other.id}`} textValue={device}>
      <TableCell>
        <span className="signing-device">
          {device}
          {other.older_setup && <Badge variant="neutral">Older setup</Badge>}
        </span>
      </TableCell>
      <TableCell heading="Account">
        <span className="ui-record-where ui-text-meta">{where}</span>
      </TableCell>
      <TableCell heading="Added">{other.added ? addedOn(other.added) : ""}</TableCell>
      <TableCell align="end">
        {mayRemove ? (
          <Button
            size="sm"
            aria-label={`Remove ${device}'s signing key from ${where}…`}
            onPress={() => setConfirming(true)}
          >
            Remove…
          </Button>
        ) : (
          <Button size="sm" onPress={() => void openUrl(keysPage)}>
            Remove it on the forge
          </Button>
        )}
        {why !== undefined && (
          <span className="ui-setting-help ui-text-secondary" role="alert">
            {why}
          </span>
        )}
        {confirming && (
          <RemoveOtherKeyConfirm
            device={device}
            where={where}
            onClose={() => setConfirming(false)}
            onRemove={() => {
              setConfirming(false);
              void commands.signingRemoveOther(forge, other.id).then((done) => {
                if (done.status === "ok") onRemoved();
                else setWhy(done.error);
              });
            }}
          />
        )}
      </TableCell>
    </TableRow>
  );
}

/**
 * **Removing another device's key, confirmed** (#5918): the device and the account by name, what
 * it does to that device's commits, and that this Mac is not affected. Its Remove key is the one
 * red button.
 */
export function RemoveOtherKeyConfirm({
  device,
  where,
  onClose,
  onRemove,
}: {
  device: string;
  where: string;
  onClose: () => void;
  onRemove: () => void;
}) {
  return (
    <Dialog
      onClose={onClose}
      title={`Remove ${device}'s signing key from ${where}?`}
      main={{ children: "Remove key", variant: "danger", onPress: onRemove }}
    >
      <p>
        {device}&apos;s chat commits will show as Unverified until it sets up signing again. This
        Mac is not affected.
      </p>
    </Dialog>
  );
}

/** What Replace key or Remove signing came to, account by account. */
function DidSaid({ did }: { did: Did }) {
  if ("why" in did)
    return (
      <p className="ui-setting-help ui-text-secondary" role="alert">
        {did.why}
      </p>
    );
  const refused = did.done.filter((one) => one.outcome === "refused");
  return (
    <div className="ui-setting-help ui-text-secondary" role="status">
      <p>
        {did.what === "replaced"
          ? "A new key is in place of the old one, on every account that had it. Check each account again to see it Verified."
          : "Signing is off on this Mac: its key is gone from the Keychain."}
      </p>
      {refused.map((one) => (
        <p key={keyOf(one)}>
          {one.login} on {one.host}: {one.why}.{" "}
          <Button size="sm" onPress={() => void openUrl(one.keys_page)}>
            Open its keys page
          </Button>
        </p>
      ))}
    </div>
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
