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
      <p className="login-site-domain ui-text-title">
        https://{login.domain}
        <span className="ui-text-body">{login.path}</span>
      </p>
      <TextField
        aria-label="Change the site this login is filled on"
        value={typed ?? siteOf(login)}
        spellCheck={false}
        onChange={onTyped}
      />
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
        <p className="ui-text-meta">
          Always allow adds this to <code>{login.under}</code>
          {login.vault_too ? ", and lets this persona use the vault" : ""}:{" "}
          <code className="login-site-line">{login.line}</code>
        </p>
      )}
    </div>
  );
}
