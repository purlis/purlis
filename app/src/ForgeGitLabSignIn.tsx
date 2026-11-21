// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type ForgeAccountRow } from "./bindings";
import { Dialog } from "./ui/dialog/Dialog";

/** The one host purlis has a GitLab application on (ADR 0077 §1). */
const DEFAULT_HOST = "gitlab.com";

/**
 * **Sign in with GitLab** (#730, ADR 0077 §1 and §3): the browser sign-in to gitlab.com. One
 * component, opened by the browser sign-in row on the welcome's sign-in page and in Settings ›
 * You › Accounts.
 *
 * - **It starts as it opens, on gitlab.com.** The button that opened it was the person's ask, so
 *   the dialog does not ask again: purlis opens gitlab.com in the person's own browser (never a
 *   page inside this window) and the dialog says it is waiting. **On a self-managed host** (an
 *   application an owner made, so `consent` is given) **it first shows what to check on the
 *   forge's page and waits for a press** (ADR 0128 D1.3 rule 1): the browser does not open until
 *   the person has read it.
 * - **Nothing of the sign-in is in the window.** The call names the forge and the host and
 *   answers the account as Settings lists it. The address the browser is sent to, the code that
 *   comes back and the token are the host process's alone.
 * - **It always has a way out.** Cancel stops the wait and closes the listener at once. A
 *   refusal is the core's sentence, with Try again beside it; the sentence names the token
 *   sign-in where that is the fix.
 */
export function ForgeGitLabSignIn({
  host: HOST = DEFAULT_HOST,
  consent,
  onSignedIn,
  onCancel,
}: {
  /** The host to sign in on: gitlab.com, or a self-managed GitLab whose application the person
   *  pinned (ADR 0128 D1.3). */
  host?: string;
  /** What to check on the forge's own page before approving: the pinned owner, said. */
  consent?: string;
  /** The account the sign-in made, as Settings lists it. */
  onSignedIn: (account: ForgeAccountRow) => void;
  onCancel: () => void;
}) {
  /** Why the last try did not sign in, in the core's words; `undefined` while one is running. */
  const [trouble, setTrouble] = useState<string | undefined>();
  /** Whether the browser may be opened: at once on gitlab.com, on a press where a consent sentence
   *  is to be read first. */
  const [pressed, setPressed] = useState(consent === undefined);
  /** The try whose answer is still wanted, by the name the host knows it by: a later try, or a
   *  cancel, outdates the ones before. */
  const wanted = useRef<string | undefined>(undefined);
  const signedIn = useRef(onSignedIn);
  useEffect(() => {
    signedIn.current = onSignedIn;
  }, [onSignedIn]);

  /** Stops the try that is running, if one is: its listener closes, and its answer is dropped. */
  const stop = useCallback(() => {
    const running = wanted.current;
    wanted.current = undefined;
    if (running !== undefined) void commands.forgePkceSignInCancel(running).catch(() => undefined);
  }, []);

  const start = useCallback(() => {
    stop();
    // Named here, so a cancel that overtakes its own start still finds it.
    const mine = crypto.randomUUID();
    wanted.current = mine;
    void commands
      .forgePkceSignIn("gitlab", HOST, mine)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((said) => {
        if (mine !== wanted.current) return;
        wanted.current = undefined;
        if (said.status === "ok") signedIn.current(said.data);
        else setTrouble(said.error);
      });
  }, [stop, HOST]);

  useEffect(() => {
    if (pressed) start();
    // Closed any other way than Cancel: the wait still stops.
    return stop;
  }, [pressed, start, stop]);

  const tryAgain = () => {
    setTrouble(undefined);
    start();
  };

  const cancel = () => {
    stop();
    onCancel();
  };

  const waiting = pressed && trouble === undefined;

  // **Placed as every dialog is** (#2910): it had no place of its own and sat wherever the
  // window drew it.
  return (
    <Dialog
      onClose={cancel}
      title="Sign in with GitLab"
      subtitle="for this machine; the sign-in is kept in your system keychain"
      // Its answer stays where it is while the browser is asked, so nothing moves when it fails.
      main={{
        children: !pressed ? `Open ${HOST} in your browser` : waiting ? "Waiting…" : "Try again",
        reserve: [`Open ${HOST} in your browser`, "Waiting…", "Try again"],
        isDisabled: waiting,
        onPress: pressed ? tryAgain : () => setPressed(true),
      }}
    >
      {!pressed ? (
        <p>Read this first. When you open {HOST}, its page shows who owns the application.</p>
      ) : waiting ? (
        <p className="pending" role="status" aria-busy="true">
          Waiting for your browser. purlis opened {HOST} there: approve purlis on that page, and the
          sign-in finishes here by itself.
        </p>
      ) : (
        // Verbatim: the core's sentence says what stopped it and what to do.
        <p className="trouble said-in-full" role="alert">
          {trouble}
        </p>
      )}
      {consent !== undefined && <p role="note">{consent}</p>}
      <p className="ui-setting-help ui-text-secondary">
        purlis asks {HOST} for the <code>api</code> scope, the only one that lets it open merge
        requests and push for you. It never hands the sign-in to a chat.
      </p>
    </Dialog>
  );
}
