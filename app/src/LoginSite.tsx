// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { LoginShown, Shown } from "./bindings";
import { TextField } from "./ui/field/Field";

/** The site as the notice first draws it: the domain, and its page where it has one. */
const siteOf = (login: LoginShown) => `${login.domain}${login.path === "/" ? "" : login.path}`;

/**
 * **The site the person typed over a login's** (#4595): `undefined` until they change it, and
 * again once the notice is drawn anew, so the field never shows what isn't being asked.
 */
export function useTypedSite(ask: Shown) {
  const shown = ask.answer.via === "browser-login" ? ask.answer.shown : undefined;
  const [held, setHeld] = useState<{ shown: string | undefined; text: string }>();
  const typed = held?.shown === shown ? held?.text : undefined;
  return [typed, (text: string) => setHeld({ shown, text })] as const;
}

/**
 * **What a login a chat suggests is filled on, and from** (#4595, the person's rulings): the
 * site large and editable, the vault and key names and never a value, then what they're warned
 * of, and the line Always writes. The site is never cut short: the part a lookalike hides in is
 * the part that would go.
 */
export function LoginSite({
  login,
  typed,
  onTyped,
}: {
  login: LoginShown;
  typed: string | undefined;
  onTyped: (typed: string) => void;
}) {
  return (
    <div className="login-site" data-testid="login-site">
      {login.replaces != null && (
        <div data-testid="login-site-replaces">
          <p className="ui-text-meta">Recorded now</p>
          <p className="login-site-domain ui-text-body">
            {login.replaces}, password typed on https://{login.replaces_form}
          </p>
          <p className="ui-text-meta">
            The chat asks to change it. Nothing changes until you choose.
          </p>
        </div>
      )}
      <p className="ui-text-meta">Opens</p>
      <p className="login-site-domain ui-text-title">
        https://{login.domain}
        <span className="ui-text-body">{login.path}</span>
      </p>
      <TextField
        aria-label="Change the site this login opens"
        value={typed ?? siteOf(login)}
        spellCheck={false}
        onChange={onTyped}
      />
      <p className="ui-text-meta">
        {login.form === login.domain
          ? "The password is typed on this site alone"
          : "That site sends the browser to sign in elsewhere. The password is typed only on"}
      </p>
      <p className="login-site-domain ui-text-title" data-testid="login-site-form">
        https://{login.form}
      </p>
      <p className="ui-text-meta">
        Fills{" "}
        {login.user_key != null && (
          <>
            <code>{login.user_key}</code> and{" "}
          </>
        )}
        <code>{login.key}</code> from vault <code>{login.vault}</code>, on https only.
      </p>
      {login.warnings.map((warning) => (
        <p key={warning} className="login-site-warning">
          {warning}
        </p>
      ))}
      {login.no_always != null ? (
        <p className="ui-text-meta">{login.no_always}.</p>
      ) : (
        <AlwaysLine
          lead={
            login.replaces != null
              ? "Always replace swaps its line in "
              : "Always allow adds this to "
          }
          under={login.under}
          vaultToo={login.vault_too}
          after={login.replaces != null ? " for" : ""}
          line={login.line}
        />
      )}
    </div>
  );
}

/**
 * **The line Always writes, shown before it's pressed** (#4595, #5116): where it goes, whether
 * the persona's vault is allowed with it, and the line itself, whole. One piece for every notice
 * whose Always writes a line in a persona's tools.
 */
export function AlwaysLine({
  lead,
  under,
  vaultToo,
  after = "",
  line,
}: {
  lead: string;
  under: string | null | undefined;
  vaultToo: boolean;
  after?: string;
  line: string;
}) {
  return (
    <p className="ui-text-meta">
      {lead}
      <code>{under}</code>
      {vaultToo ? ", and lets this persona use the vault" : ""}
      {after}: <code className="login-site-line">{line}</code>
    </p>
  );
}
