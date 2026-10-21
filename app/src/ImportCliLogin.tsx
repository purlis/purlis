// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type ForgeAccountRow, type ForgeCliLogin } from "./bindings";
import { SettingActions } from "./settings/components";
import { Button } from "./ui/button/Button";

/** The offers declined in this run of the window, by CLI and host: asked once, not again. */
const declined = new Set<string>();

const keyOf = (offer: ForgeCliLogin) => `${offer.cli}@${offer.host}`;

/**
 * **Use your existing `gh` or `glab` login** (#1376, ADR 0077 §5): offered, never taken.
 *
 * Drawn on the welcome's sign-in page and in Settings › You › Accounts, for each forge tool that
 * is signed in to its forge's own host where purlis has no account yet (`forge_cli_logins`, which
 * asks each tool only whether it is signed in). **The offer says what the import means before it
 * is pressed**: the tool's token is copied into purlis's own item in the system keychain, and
 * from then on chats on that host lose the forge access the tool gave them (ruling V22c), until
 * chats have sign-ins of their own. The tool keeps its own login.
 *
 * **It defaults to declining.** Nothing is imported until **Use my … login** is pressed; **Not
 * now** comes first, and declining leaves the tool, the keychain and chats as they were. A
 * declined offer is not drawn again in this run of the window. The token never crosses into the
 * window: the import is one call that names the forge and the host, and its answer is the
 * account as Settings lists it.
 */
export function ImportCliLogin({
  onImported,
}: {
  /** The account the import signed in to: the list it belongs to is read again. */
  onImported: (account: ForgeAccountRow) => void;
}) {
  const [offers, setOffers] = useState<ForgeCliLogin[]>([]);
  /** The offer being imported right now, so it is not pressed twice. */
  const [importing, setImporting] = useState<string>();
  /** Why the last import did not happen, by offer, in the core's words. */
  const [refused, setRefused] = useState<{ key: string; why: string }>();
  /** Bumped on a decline, so the drawn list drops it. */
  const [, setDeclines] = useState(0);

  useEffect(() => {
    let gone = false;
    void commands
      .forgeCliLogins()
      .then((answer) => {
        // An offer that cannot be read is no offer: the token sign-in is right beside it.
        if (!gone && answer.status === "ok" && Array.isArray(answer.data)) setOffers(answer.data);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, []);

  const shown = offers.filter((offer) => !declined.has(keyOf(offer)));
  if (shown.length === 0) return null;

  const decline = (offer: ForgeCliLogin) => {
    declined.add(keyOf(offer));
    setDeclines((n) => n + 1);
  };

  const accept = async (offer: ForgeCliLogin) => {
    const key = keyOf(offer);
    setImporting(key);
    setRefused(undefined);
    const answer = await commands
      .forgeImportCliLogin(offer.kind, offer.host)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setImporting(undefined);
    if (answer.status === "error") {
      setRefused({ key, why: answer.error });
      return;
    }
    setOffers((was) => was.filter((one) => keyOf(one) !== key));
    onImported(answer.data);
  };

  return (
    <>
      {shown.map((offer) => {
        const key = keyOf(offer);
        const id = `import-${offer.cli}-${offer.host}`.replace(/[^a-zA-Z0-9-]/g, "-");
        const busy = importing === key;
        return (
          <section key={key} className="import-cli-login" aria-labelledby={id}>
            <h2 id={id}>Use your existing {offer.cli} login</h2>
            <p>
              <code>{offer.cli}</code> is signed in to {offer.host} on this machine. purlis can use
              that login, so you need not sign in to {offer.title} again.
            </p>
            <p>
              <strong>What this does:</strong> purlis copies the token {offer.cli} keeps for{" "}
              {offer.host} into its own item in your system keychain, and never hands it to a chat.
              A chat that runs {offer.cli} itself can still use {offer.cli}&apos;s own stored login.
              Sign out of {offer.cli} if you don&apos;t want that.
            </p>
            <SettingActions>
              <Button isDisabled={busy} onPress={() => decline(offer)}>
                Not now
              </Button>
              <Button isDisabled={busy} onPress={() => void accept(offer)}>
                Use my {offer.cli} login
              </Button>
            </SettingActions>
            {busy && (
              <p className="came-back" role="status">
                Copying the {offer.cli} login into your keychain…
              </p>
            )}
            {/* Verbatim: the core's sentence names what stopped it, and never holds the token. */}
            {refused?.key === key && (
              <p className="trouble said-in-full" role="alert">
                {refused.why}
              </p>
            )}
          </section>
        );
      })}
    </>
  );
}

/** Forgets every declined offer. Only the tests call it, between cases. */
export function forgetDeclinedImports(): void {
  declined.clear();
}
