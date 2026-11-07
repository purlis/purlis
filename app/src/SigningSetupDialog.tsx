// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type ForgeDeviceCode, type SigningCheck, type SigningState } from "./bindings";
import { SettingActions } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { Dialog } from "./ui/dialog/Dialog";
import { StepList, type Step as RailStep } from "./ui/steps/Steps";

/** GitHub's page for a new signing key. */
export const NEW_SIGNING_KEY_PAGE = "https://github.com/settings/ssh/new";

/** How often the by-hand path asks GitHub whether the key is there yet. */
export const LOOK_EVERY_MS = 5000;

type Key = { at: "explain" } | { at: "making" } | { at: "refused"; why: string };

type Add =
  | { at: "adding" }
  | { at: "ask" }
  | { at: "asking" }
  | { at: "waiting"; code: ForgeDeviceCode }
  | { at: "declined"; why: string }
  | { at: "by-hand"; copied: boolean }
  | { at: "trouble"; why: string };

type Check =
  { at: "checking" } | { at: "found"; result: SigningCheck } | { at: "trouble"; why: string };

type Now =
  | { step: "reading" }
  | { step: "unread"; why: string }
  | { step: "key"; key: Key }
  | { step: "add"; add: Add }
  | { step: "check"; check: Check }
  | { step: "done"; email: string };

const STEP_IDS = ["key", "add", "check"] as const;

/**
 * **GitHub's reason a test commit isn't Verified, in the window's words**, with what to do. The
 * ✓ comes only from GitHub's own `valid` (ADR 0116 §6).
 */
export function reasonSaid(check: SigningCheck, login: string): string {
  switch (check.reason) {
    case "unverified_email":
    case "no_user":
    case "bad_email":
      return `GitHub checked the signature, but your commits use ${check.email}, which isn't a verified email on ${login}. GitHub only verifies a commit whose email is one of yours. Add and verify it in GitHub's email settings, then check again.`;
    case "unknown_key":
    case "not_signing_key":
      return `GitHub doesn't know this key as a signing key on ${login} yet. Add it, then check again.`;
    case "expired_key":
      return "GitHub says this key has expired. Remove it on GitHub, then set up signing again.";
    default:
      return `GitHub didn't verify the test commit. It says "${check.reason}". Check again in a moment.`;
  }
}

/**
 * **Set up signing** (#5628, spec #5607, ADR 0116 §4): one dialog, the same from every way in,
 * with a rail of three steps: *Make your key*, *Add it to GitHub* and *Check*. A step that needs
 * nothing from the person runs on by itself.
 *
 * - **Make your key**: the person is told just before macOS asks to let purlis use the Keychain,
 *   and to choose Always Allow. A no says the key couldn't be kept, with Try again. A device that
 *   has its key already goes straight on.
 * - **Add it to GitHub**: through the account's own sign-in. Where it may not add keys yet, the
 *   step says what the permission is for and asks for it on GitHub's page (a code, as a sign-in
 *   does), never at sign-in. **I'll add it myself** copies the key, opens GitHub's page for a new
 *   signing key and watches GitHub's list until the key is there.
 * - **Check** makes a signed test commit that GitHub keeps on no branch, and shows *Verified on
 *   GitHub ✓* only where GitHub said so.
 *
 * Closing at any step leaves nothing half-done that reads as on: on means this device has a key,
 * and the card reads it again (`onClose`).
 */
export function SigningSetupDialog({
  repo = null,
  onClose,
}: {
  /** The repository setup was opened for (`owner/name`), checked in; else the account's own. */
  repo?: string | null;
  /** Closed, at any step, or done. */
  onClose: () => void;
}) {
  const [now, setNow] = useState<Now>({ step: "reading" });
  const [state, setState] = useState<SigningState>();
  const [showing, setShowing] = useState<string | null>(null);
  /** The permission flow waiting on GitHub: stopped once it is no longer this one, or as the
   *  dialog closes. An answer that comes after a dialog closed changes nothing. */
  const [flow, setFlow] = useState<number>();
  useEffect(() => {
    if (flow === undefined) return;
    return () => void commands.forgeDeviceSignInCancel(flow).catch(() => undefined);
  }, [flow]);

  const go = setNow;

  const check = useCallback(() => {
    go({ step: "check", check: { at: "checking" } });
    void commands.signingCheck(repo).then((checked) => {
      if (checked.status === "error")
        go({ step: "check", check: { at: "trouble", why: checked.error } });
      else if (checked.data.verified) go({ step: "done", email: checked.data.email });
      else go({ step: "check", check: { at: "found", result: checked.data } });
    });
  }, [go, repo]);

  const add = useCallback(() => {
    go({ step: "add", add: { at: "adding" } });
    void commands.signingAddKey().then((added) => {
      if (added.status === "error") go({ step: "add", add: { at: "trouble", why: added.error } });
      else if (added.data === "added") check();
      else go({ step: "add", add: { at: "ask" } });
    });
  }, [check, go]);

  // As it opens: what this device and its GitHub account have, and the first step that needs
  // the person. A key there already, or an account that may add it, runs on by itself.
  useEffect(() => {
    void commands.signingState().then((read) => {
      if (read.status === "error") {
        go({ step: "unread", why: read.error });
        return;
      }
      setState(read.data);
      if (!read.data.on) go({ step: "key", key: { at: "explain" } });
      else if (read.data.account?.may_add) add();
      else go({ step: "add", add: { at: "ask" } });
    });
  }, [add, go]);

  const makeKey = () => {
    go({ step: "key", key: { at: "making" } });
    void commands.signingMakeKey().then((made) => {
      if (made.status === "error") {
        go({ step: "key", key: { at: "refused", why: made.error } });
        return;
      }
      void commands.signingState().then((read) => {
        if (read.status === "ok") setState(read.data);
        if (read.status === "ok" && read.data.account?.may_add) add();
        else go({ step: "add", add: { at: "ask" } });
      });
    });
  };

  const askPermission = () => {
    go({ step: "add", add: { at: "asking" } });
    void commands.forgeSigningPermissionStart().then((started) => {
      if (started.status === "error") {
        go({ step: "add", add: { at: "declined", why: started.error } });
        return;
      }
      const asked = started.data.flow;
      setFlow(asked);
      go({ step: "add", add: { at: "waiting", code: started.data } });
      void commands.forgeSigningPermissionWait(asked).then((ended) => {
        setFlow(undefined);
        if (ended.status === "ok") {
          add();
          return;
        }
        // Only while this flow is still the one waiting: I'll add it myself ended it.
        setNow((was) =>
          was.step === "add" && was.add.at === "waiting" && was.add.code.flow === asked
            ? { step: "add", add: { at: "declined", why: ended.error } }
            : was,
        );
      });
    });
  };

  const copyKey = useCallback(
    async () =>
      state?.public_key
        ? (navigator.clipboard?.writeText(state.public_key).then(
            () => true,
            () => false,
          ) ?? false)
        : false,
    [state],
  );

  const byHand = () => {
    setFlow(undefined);
    void copyKey().then((copied) => go({ step: "add", add: { at: "by-hand", copied } }));
    void openUrl(NEW_SIGNING_KEY_PAGE);
  };

  // By hand: GitHub's own list is asked until the key is there, then Check runs on.
  const watching = now.step === "add" && now.add.at === "by-hand";
  const lookNow = useCallback(() => {
    void commands.signingKeyListed().then((listed) => {
      if (listed.status === "ok" && listed.data) check();
    });
  }, [check]);
  useEffect(() => {
    if (!watching) return;
    const every = setInterval(lookNow, LOOK_EVERY_MS);
    return () => clearInterval(every);
  }, [watching, lookNow]);

  const login = state?.account?.login ?? "your account";
  const at = now.step === "key" ? 0 : now.step === "add" ? 1 : now.step === "check" ? 2 : 3;
  const rail: RailStep[] = [
    { id: "key", name: "Make your key", state: at > 0 ? "done" : "current" },
    {
      id: "add",
      name: "Add it to GitHub",
      state: at > 1 ? "done" : at === 1 ? "current" : "later",
      line: at > 1 ? `Added to ${login}` : null,
    },
    {
      id: "check",
      name: "Check",
      state: at > 2 ? "done" : at === 2 ? "current" : "later",
    },
  ];
  const current = STEP_IDS[Math.min(at, 2)];
  const lookingBack = showing !== null && showing !== current && at < 3;

  const main = (): {
    children: string;
    onPress: () => void;
    isDisabled?: boolean;
    reserve?: string[];
  } => {
    if (lookingBack) return { children: "Back to where you were", onPress: () => setShowing(null) };
    switch (now.step) {
      case "reading":
        return { children: "Make my key", onPress: () => undefined, isDisabled: true };
      case "unread":
        return { children: "Close", onPress: onClose };
      case "key":
        return {
          children: now.key.at === "refused" ? "Try again" : "Make my key",
          reserve: ["Make my key", "Try again"],
          isDisabled: now.key.at === "making",
          onPress: makeKey,
        };
      case "add":
        switch (now.add.at) {
          case "ask":
            return { children: "Continue to GitHub", onPress: askPermission };
          case "waiting": {
            const page = now.add.code.verification_uri;
            return { children: "Open GitHub", onPress: () => void openUrl(page) };
          }
          case "declined":
            return { children: "Try again", onPress: askPermission };
          case "by-hand":
            return { children: "I've added it", onPress: lookNow };
          case "trouble":
            return { children: "Try again", onPress: add };
          default:
            return { children: "Continue to GitHub", onPress: () => undefined, isDisabled: true };
        }
      case "check":
        return {
          children: "Check again",
          isDisabled: now.check.at === "checking",
          onPress: check,
        };
      case "done":
        return { children: "Done", onPress: onClose };
    }
  };

  const alternative =
    now.step === "add" && ["ask", "waiting", "declined"].includes(now.add.at) && !lookingBack
      ? { children: "I'll add it myself", onPress: byHand }
      : undefined;

  return (
    <Dialog
      onClose={onClose}
      title="Set up signing"
      subtitle="So GitHub shows the commits purlis makes for you as Verified."
      className="signing-setup"
      cancel={now.step === "done" ? false : undefined}
      summary={now.step === "done" ? undefined : `Step ${Math.min(at, 2) + 1} of 3`}
      alternative={alternative}
      main={main()}
    >
      {now.step !== "done" && now.step !== "reading" && now.step !== "unread" && (
        <StepList
          aria-label="Steps of Set up signing"
          steps={rail}
          showing={lookingBack ? showing : current}
          onGo={setShowing}
          className="signing-setup-rail"
        />
      )}
      {lookingBack ? (
        <LookBack showing={showing} state={state} />
      ) : (
        <Body
          now={now}
          state={state}
          login={login}
          onCopy={() =>
            void copyKey().then((copied) => go({ step: "add", add: { at: "by-hand", copied } }))
          }
        />
      )}
    </Dialog>
  );
}

/** What a done step settled on, when the person goes back to it on the rail. */
function LookBack({ showing, state }: { showing: string | null; state?: SigningState }) {
  if (showing === "key")
    return (
      <p>
        This Mac&apos;s signing key is kept in its Keychain
        {state?.fingerprint ? <> as {state.fingerprint}</> : null}.
      </p>
    );
  return <p>The key is on {state?.account?.login ?? "your account"} on GitHub.</p>;
}

function Body({
  now,
  state,
  login,
  onCopy,
}: {
  now: Now;
  state?: SigningState;
  login: string;
  onCopy: () => void;
}) {
  switch (now.step) {
    case "reading":
      return (
        <p className="pending" role="status" aria-busy="true">
          Reading what this Mac has…
        </p>
      );
    case "unread":
      return (
        <p className="trouble said-in-full" role="alert">
          {now.why}
        </p>
      );
    case "key":
      return (
        <>
          <p>
            purlis makes a signing key on this Mac and keeps it in the macOS Keychain. It never
            leaves this Mac, and no chat can reach it.
          </p>
          <Card as="div" variant="ask">
            <p>
              <strong>macOS will ask next</strong> to let purlis use your Keychain. Choose{" "}
              <strong>Always Allow</strong>, so it doesn&apos;t ask again for every commit.
            </p>
          </Card>
          {now.key.at === "making" && (
            <p className="pending" role="status" aria-busy="true">
              Making your key…
            </p>
          )}
          {now.key.at === "refused" && (
            <p className="trouble said-in-full" role="alert">
              {now.key.why}
            </p>
          )}
        </>
      );
    case "add":
      return <AddBody add={now.add} state={state} login={login} onCopy={onCopy} />;
    case "check":
      return (
        <>
          <p>purlis makes a small signed test and asks GitHub whether it shows as Verified.</p>
          {now.check.at === "checking" && (
            <p className="pending" role="status" aria-busy="true">
              Asking GitHub…
            </p>
          )}
          {now.check.at === "found" && (
            <p className="trouble said-in-full" role="alert">
              {reasonSaid(now.check.result, login)}
            </p>
          )}
          {now.check.at === "trouble" && (
            <p className="trouble said-in-full" role="alert">
              {now.check.why}
            </p>
          )}
        </>
      );
    case "done":
      return (
        <div role="status">
          <p className="signing-verified">
            <strong>Verified on GitHub ✓</strong>
          </p>
          <p>Chat commits on this Mac are now signed in every repo, as {now.email}.</p>
        </div>
      );
  }
}

function AddBody({
  add,
  state,
  login,
  onCopy,
}: {
  add: Add;
  state?: SigningState;
  login: string;
  onCopy: () => void;
}) {
  switch (add.at) {
    case "adding":
      return (
        <p className="pending" role="status" aria-busy="true">
          Adding the key to {login} on GitHub…
        </p>
      );
    case "ask":
    case "asking":
      return (
        <>
          <p>
            GitHub needs to know this key is yours. purlis can add it to <strong>{login}</strong>{" "}
            for you.
          </p>
          <Card as="div" variant="ask">
            <p>
              GitHub will ask you to let purlis <strong>manage your signing keys</strong> and{" "}
              <strong>read your email addresses</strong>. That&apos;s all it&apos;s for: adding this
              key, checking it, and removing it if you ask.
            </p>
          </Card>
          {add.at === "asking" && (
            <p className="pending" role="status" aria-busy="true">
              Asking GitHub for a code…
            </p>
          )}
        </>
      );
    case "waiting":
      return (
        <>
          <p>Enter this code on GitHub&apos;s page, then allow it there.</p>
          <Card as="p" variant="ask" className="forge-device-code">
            <strong>
              <code aria-label="The code to enter on GitHub">{add.code.user_code}</code>
            </strong>
          </Card>
          <SettingActions>
            <Button onPress={() => void navigator.clipboard?.writeText(add.code.user_code)}>
              Copy code
            </Button>
          </SettingActions>
          <p className="pending" role="status" aria-busy="true">
            Waiting for you to allow it on GitHub…
          </p>
        </>
      );
    case "declined":
      return (
        <>
          <p className="trouble said-in-full" role="alert">
            {add.why}
          </p>
          <p>You can try again, or add the key on GitHub yourself.</p>
        </>
      );
    case "by-hand":
      return (
        <>
          <p>Add it on GitHub yourself. purlis watches and carries on once it&apos;s there.</p>
          <ol>
            <li>
              {add.copied
                ? "The key is copied."
                : "purlis couldn't copy the key: copy it from the box below."}{" "}
              <Button size="sm" onPress={onCopy}>
                Copy again
              </Button>
            </li>
            <li>
              On GitHub&apos;s page, choose Key type <strong>Signing Key</strong>.
            </li>
            <li>Paste it, name it &quot;{state?.title ?? "purlis"}&quot;, and save.</li>
          </ol>
          {state?.public_key && (
            <Card as="p" variant="ask" className="signing-public-key">
              <code aria-label="This Mac's public signing key">{state.public_key}</code>
            </Card>
          )}
          <p className="pending" role="status" aria-busy="true">
            Waiting for it to appear on {login}…
          </p>
        </>
      );
    case "trouble":
      return (
        <p className="trouble said-in-full" role="alert">
          {add.why}
        </p>
      );
  }
}
