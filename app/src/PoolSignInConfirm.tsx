// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PoolSignInAsk } from "./bindings";
import { listen } from "./here";
import { AlertDialog } from "./ui/dialog/Dialog";

/** The event the core sends each time the question changes. */
export const POOL_SIGN_IN_CONFIRM = "pool-sign-in-confirm";

/** The question waiting, read as the window comes up and each time the core says it changed. */
function useAsked(): PoolSignInAsk | null {
  const [asked, setAsked] = useState<PoolSignInAsk | null>(null);
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    const read = async () => {
      try {
        const now = await commands.poolSignInAsked();
        if (!gone) setAsked(now ?? null);
      } catch {
        // What cannot be read asks nothing.
      }
    };
    void (async () => {
      try {
        const unlisten = await listen(POOL_SIGN_IN_CONFIRM, () => {
          void read();
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
      await read();
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, []);
  return asked;
}

/**
 * **A pool sign-in started from the command is confirmed here**: `purlis profile sign-in --pool`
 * waits for this answer. It starts on Cancel, so a Return meant for something else declines it,
 * and its yes is pressed or not taken (`variant: "grants"`). Escape declines too. Each window
 * draws it, and the question going takes it from all of them.
 */
export function PoolSignInConfirm() {
  const asked = useAsked();
  if (!asked) return null;
  const answer = (yes: boolean) => {
    void commands.answerPoolSignIn(asked.id, yes).catch(() => {
      // Taken back already: the core says so, and the dialog goes with it.
    });
  };
  return (
    <AlertDialog
      onClose={() => answer(false)}
      title={`Sign ${asked.profile} in on ${asked.pool}?`}
      titleId="pool-sign-in-confirm"
      aria-describedby="pool-sign-in-confirm-said"
      initialFocus="cancel"
      main={{
        children: "Sign in",
        variant: "grants",
        onPress: () => answer(true),
      }}
    >
      <p id="pool-sign-in-confirm-said">
        The purlis profile sign-in command asks to sign this profile in on this pool&apos;s runner.
        Sign in only if you ran it. Otherwise, cancel and nothing starts.
      </p>
    </AlertDialog>
  );
}
