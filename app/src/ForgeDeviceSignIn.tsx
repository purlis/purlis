// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import * as Dialog from "./overlays/dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type ForgeAccountRow, type ForgeDeviceCode } from "./bindings";
import { SettingActions } from "./settings/components";

/** What each scope the sign-in asks for lets purlis do, in the window's words (FI2). */
const SCOPE_SAYS: Record<string, string> = {
  repo: "your repositories, private ones too: listing them, creating one, pushing to them, and their pull requests and issues",
  "read:org": "the names of your organisations, so a project can be created in one",
};

type Step =
  { at: "asking" } | { at: "waiting"; code: ForgeDeviceCode } | { at: "ended"; why: string };

/**
 * **Sign in with GitHub** (#729; ADR 0077 §3, as amended 2026-10-12): the OAuth device flow, in
 * the window. One component, used by Settings › Accounts and by the welcome's "Sign in".
 *
 * - **A code, and GitHub's own page to type it on.** Opening the dialog asks github.com for a
 *   code (`forge_device_sign_in_start`). The dialog shows it, with Copy and a button that opens
 *   GitHub's page in the system browser. The code is shown for a flow the person started here,
 *   and nowhere else.
 * - **Then it waits** (`forge_device_sign_in_wait`) until the code is confirmed in the browser,
 *   and answers the account. The window never holds a token: the core trades the code for one
 *   and keeps it in the keychain.
 * - **Every way it can end says what to do next**, in the core's sentence: a code that expired,
 *   a sign-in refused on GitHub, a forge that could not be reached. **Try again** asks for a new
 *   code; **Cancel** stops the waiting (`forge_device_sign_in_cancel`) and leaves nothing behind.
 */
export function ForgeDeviceSignIn({
  onSignedIn,
  onCancel,
}: {
  /** The account the sign-in made, as Settings lists it. */
  onSignedIn: (account: ForgeAccountRow) => void;
  onCancel: () => void;
}) {
  const [step, setStep] = useState<Step>({ at: "asking" });
  const [copied, setCopied] = useState<boolean>();
  /** Which try this is: Try again asks for a new code. */
  const [attempt, setAttempt] = useState(0);
  const signedIn = useRef(onSignedIn);
  useEffect(() => {
    signedIn.current = onSignedIn;
  }, [onSignedIn]);

  // One try: ask for a code, show it, and wait on it. Closing the dialog, or trying again,
  // leaves this try: its later answers are not this dialog's, and its polling is stopped.
  useEffect(() => {
    let left = false;
    let flow: number | undefined;
    /** Stops a flow's polling. A window that is going away has nothing left to stop. */
    const stop = (which: number) =>
      void commands.forgeDeviceSignInCancel(which).catch(() => undefined);
    const refused = (err: unknown) => ({ status: "error" as const, error: String(err) });
    void commands
      .forgeDeviceSignInStart("github", "github.com")
      .catch(refused)
      .then((started) => {
        if (started.status === "error") {
          if (!left) setStep({ at: "ended", why: started.error });
          return;
        }
        if (left) {
          stop(started.data.flow);
          return;
        }
        flow = started.data.flow;
        setStep({ at: "waiting", code: started.data });
        return commands
          .forgeDeviceSignInWait(started.data.flow)
          .catch(refused)
          .then((ended) => {
            flow = undefined;
            if (left) return;
            if (ended.status === "ok") signedIn.current(ended.data);
            else setStep({ at: "ended", why: ended.error });
          });
      })
      // An answer that is not what the core sends: said, never thrown past the dialog.
      .catch((err: unknown) => {
        if (!left) setStep({ at: "ended", why: String(err) });
      });
    return () => {
      left = true;
      if (flow !== undefined) stop(flow);
    };
  }, [attempt]);

  const tryAgain = () => {
    setStep({ at: "asking" });
    setCopied(undefined);
    setAttempt((n) => n + 1);
  };

  const copy = (code: string) => {
    const put = navigator.clipboard?.writeText(code);
    if (!put) {
      setCopied(false);
      return;
    }
    void put.then(
      () => setCopied(true),
      () => setCopied(false),
    );
  };

  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="ui-scrim" />
        <Dialog.Content
          className="ui-dialog warning"
          aria-describedby={undefined}
          // A click outside answers nothing (`docs/ui-primitives.md`). Escape is Cancel.
          onInteractOutside={(e) => e.preventDefault()}
        >
          <Dialog.Title>Sign in with GitHub</Dialog.Title>
          <p className="where">
            on github.com, in your browser; purlis keeps the sign-in in your system keychain
          </p>

          {step.at === "asking" && (
            <p className="pending" role="status" aria-busy="true">
              Asking github.com for a code…
            </p>
          )}

          {step.at === "waiting" && (
            <>
              <p>
                Enter this code on GitHub&apos;s page, then approve purlis there. The code is for
                this sign-in only.
              </p>
              <p className="ui-inline-surface ui-inline-surface-waiting">
                <strong>
                  <code aria-label="The code to enter on GitHub">{step.code.user_code}</code>
                </strong>
              </p>
              <SettingActions>
                <button
                  type="button"
                  className="ui-button ui-button-primary"
                  // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
                  tabIndex={0}
                  onClick={() => void openUrl(step.code.verification_uri)}
                >
                  Open GitHub in your browser
                </button>
                <button
                  type="button"
                  className="ui-button"
                  tabIndex={0}
                  onClick={() => copy(step.code.user_code)}
                >
                  Copy code
                </button>
              </SettingActions>
              {copied !== undefined && (
                <p className="ui-setting-help" role="status">
                  {copied ? "The code is on the clipboard." : "purlis could not copy the code."}
                </p>
              )}
              <p className="ui-setting-help">
                Or go to <code>{step.code.verification_uri}</code> on any device.
              </p>
              <div className="ui-setting-help">
                <p>GitHub will ask you to let purlis reach:</p>
                <ul>
                  {step.code.scopes.map((scope) => (
                    <li key={scope}>
                      <code>{scope}</code>
                      {SCOPE_SAYS[scope] ? `: ${SCOPE_SAYS[scope]}` : ""}
                    </li>
                  ))}
                </ul>
                <p>The sign-in is yours alone: purlis never hands it to a chat.</p>
              </div>
              <p className="pending" role="status" aria-busy="true">
                Waiting for you to approve it on GitHub…
              </p>
            </>
          )}

          {/* Verbatim: the core's sentence says how it ended and what to do next. */}
          {step.at === "ended" && (
            <p className="trouble said-in-full" role="alert">
              {step.why}
            </p>
          )}

          <SettingActions>
            {step.at === "ended" && (
              <button
                type="button"
                className="ui-button ui-button-primary"
                tabIndex={0}
                onClick={tryAgain}
              >
                Try again
              </button>
            )}
            <button type="button" className="ui-button" tabIndex={0} onClick={onCancel}>
              Cancel
            </button>
          </SettingActions>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
