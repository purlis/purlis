// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  commands,
  type ForgeDeviceCode,
  type SigningAccount,
  type SigningCheck,
  type SigningEmails,
  type SigningForge,
  type SigningKeyShown,
  type SigningState,
} from "./bindings";
import { SettingActions } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { Radio, RadioGroup } from "./ui/choice/Radio";
import { Dialog } from "./ui/dialog/Dialog";
import { StepList, type Step as RailStep } from "./ui/steps/Steps";

/** GitHub's page for a new signing key. */
export const NEW_SIGNING_KEY_PAGE = "https://github.com/settings/ssh/new";

/** How often the by-hand path asks the forge whether the key is there yet. */
export const LOOK_EVERY_MS = 5000;

/** The forge's page where the person adds a signing key by hand: GitHub's, or the GitLab's own. */
export function newKeyPage(account: Pick<SigningAccount, "kind" | "host">): string {
  return account.kind === "gitlab"
    ? `https://${account.host}/-/user_settings/ssh_keys`
    : NEW_SIGNING_KEY_PAGE;
}

/** The account setup is for: the one named, else the first whose forge verifies signed commits. */
export function accountFor(
  state: SigningState | undefined,
  forge: SigningForge | null | undefined,
): SigningAccount | undefined {
  const accounts = state?.accounts ?? [];
  if (forge) return accounts.find((one) => one.kind === forge.kind && one.host === forge.host);
  return accounts.find((one) => one.verifies);
}

/** What a forge that can't verify signed commits says, with no setup offered (#5629). */
export function cantVerify(account: Pick<SigningAccount, "forge" | "host">): string {
  return `This server can't verify signed commits: ${account.forge} on ${account.host} is too old to show them as Verified, so signing isn't set up for it.`;
}

type Key = { at: "explain" } | { at: "making" } | { at: "refused"; why: string };

type Add =
  | { at: "adding" }
  | { at: "ask" }
  | { at: "asking" }
  | { at: "waiting"; code: ForgeDeviceCode }
  | { at: "declined"; why: string }
  | { at: "by-hand"; copied: boolean }
  | { at: "trouble"; why: string };

/** The forge's reasons a commit's email is not one of the account's verified ones. */
export const EMAIL_REASONS = ["unverified_email", "no_user", "bad_email"];

type Check =
  | { at: "checking" }
  | { at: "found"; result: SigningCheck }
  /** The email isn't one the forge verifies (#5631): the account's verified ones, once read,
   *  and the one picked. */
  | {
      at: "email";
      result: SigningCheck;
      emails?: SigningEmails;
      picked?: string;
      why?: string;
    }
  | { at: "nothing-pushed" }
  | { at: "trouble"; why: string };

type Now =
  | { step: "reading" }
  | { step: "unread"; why: string }
  | { step: "key"; key: Key }
  | { step: "add"; add: Add }
  | { step: "check"; check: Check }
  | { step: "done"; email: string };

const STEP_IDS = ["key", "add", "check"] as const;

/**
 * **The forge's reason a commit isn't Verified, in the window's words**, with what to do. The ✓
 * comes only from the forge's own `valid` (ADR 0116 §6). GitLab's reasons come in GitHub's words.
 */
export function reasonSaid(check: SigningCheck, login: string, forge = "GitHub"): string {
  switch (check.reason) {
    case "unverified_email":
    case "no_user":
    case "bad_email":
      return `${forge} checked the signature, but your commits use ${check.email}, which isn't a verified email on ${login}. ${forge} only verifies a commit whose email is one of yours. Add and verify it in ${forge}'s email settings, then check again.`;
    case "unknown_key":
    case "not_signing_key":
      return `${forge} doesn't know this key as a signing key on ${login} yet. Add it, then check again.`;
    case "other_key":
      return `The commit ${forge} checked was signed with another key, not this Mac's. Check again once a chat on this Mac has pushed a signed commit there.`;
    case "expired_key":
      return `${forge} says this key has expired. Remove it on ${forge}, then set up signing again.`;
    default:
      return `${forge} didn't verify the commit. It says "${check.reason}". Check again in a moment.`;
  }
}

/**
 * **Set up signing** (#5628, #5629, spec #5607, ADR 0116 §4): one dialog, the same from every way
 * in and on every forge, with a rail of three steps: *Make your key*, *Add it to <forge>* and
 * *Check*. A step that needs nothing from the person runs on by itself.
 *
 * - **Make your key**: the person is told just before macOS asks to let purlis use the Keychain,
 *   and to choose Always Allow. A no says the key couldn't be kept, with Try again. A device that
 *   has its key already goes straight on.
 * - **Add it to <forge>**: through the account's own sign-in. On **GitHub**, where it may not add
 *   keys yet, the step says what the permission is for and asks for it on GitHub's page (a code,
 *   as a sign-in does), never at sign-in; **I'll add it myself** copies the key, opens GitHub's
 *   page for a new signing key and watches GitHub's list until the key is there. On **GitLab**,
 *   whose sign-in may add keys already, the key is added as a signing key only and the step runs
 *   on by itself.
 * - **Check**: on GitHub a signed test commit that GitHub keeps on no branch; on GitLab, which
 *   makes none, a signed commit a chat pushed there. *Verified on <forge> ✓* only where the forge
 *   said so.
 *
 * A GitLab too old to verify signed commits says so, and no step is offered. Closing at any step
 * leaves nothing half-done that reads as on: on means this device has a key, and the card reads
 * it again (`onClose`).
 */
export function SigningSetupDialog({
  forge = null,
  repo = null,
  commit = null,
  finish,
  onClose,
}: {
  /** What the last step offers in place of Done, where setup was opened from work a forge
   *  refused: **Sign and retry**, or **Sign and retry all N** from the notice (ADR 0116 §4). */
  finish?: { label: string; onFinish: () => void };
  /** The forge account setup is for: the one bound to the repo that needed it. With none, the
   *  first account whose forge verifies signed commits. */
  forge?: SigningForge | null;
  /** The repository setup was opened for (`owner/name`, or a GitLab project's path), checked in;
   *  else the account's own. */
  repo?: string | null;
  /** The pushed commit setup was opened for: what GitLab's Check asks about. */
  commit?: string | null;
  /** Closed, at any step, or done. */
  onClose: () => void;
}) {
  const [now, setNow] = useState<Now>({ step: "reading" });
  const [state, setState] = useState<SigningState>();
  /** The key's public half, read from the Keychain only for the step that adds it by hand. */
  const [key, setKey] = useState<SigningKeyShown>();
  const [showing, setShowing] = useState<string | null>(null);
  /** The permission flow waiting on GitHub: stopped once it is no longer this one, or as the
   *  dialog closes. An answer that comes after a dialog closed changes nothing. */
  const [flow, setFlow] = useState<number>();
  useEffect(() => {
    if (flow === undefined) return;
    return () => void commands.forgeDeviceSignInCancel(flow).catch(() => undefined);
  }, [flow]);

  const go = setNow;
  const account = accountFor(state, forge);
  // Every step names the account by its forge and host, read once the state is.
  const kind = account?.kind ?? forge?.kind ?? "github";
  const host = account?.host ?? forge?.host ?? "github.com";
  const gitlab = kind === "gitlab";
  const forgeName = account?.forge ?? (gitlab ? "GitLab" : "GitHub");

  const check = useCallback(
    (on: SigningForge) => {
      go({ step: "check", check: { at: "checking" } });
      void commands.signingCheck(on, repo, commit).then((checked) => {
        if (checked.status === "error")
          go({ step: "check", check: { at: "trouble", why: checked.error } });
        else if (checked.data.verified) go({ step: "done", email: checked.data.email });
        else if (checked.data.reason === "nothing_pushed")
          go({ step: "check", check: { at: "nothing-pushed" } });
        else if (EMAIL_REASONS.includes(checked.data.reason)) {
          const result = checked.data;
          go({ step: "check", check: { at: "email", result } });
          void commands.signingEmails(on).then((read) =>
            go({
              step: "check",
              check:
                read.status === "ok" && read.data
                  ? { at: "email", result, emails: read.data, picked: read.data.emails[0]?.email }
                  : {
                      at: "email",
                      result,
                      why: read.status === "error" ? read.error : "purlis couldn't read them.",
                    },
            }),
          );
        } else go({ step: "check", check: { at: "found", result: checked.data } });
      });
    },
    [go, repo, commit],
  );

  /** Chat commits signed as the address picked, then Check again (#5631). */
  const takeEmail = (email: string) => {
    const on = { kind, host };
    void commands.signingUseEmail(on, email).then((used) => {
      if (used.status === "ok") check(on);
      else
        setNow((was) =>
          was.step === "check" && was.check.at === "email"
            ? { step: "check", check: { ...was.check, why: used.error } }
            : was,
        );
    });
  };

  /** The public key read (once), then copied: whether it reached the clipboard. */
  const copyKey = useCallback(async () => {
    let shown = key;
    if (!shown) {
      const read = await commands.signingKeyShown();
      if (read.status === "error") return false;
      shown = read.data;
      setKey(shown);
    }
    const put = navigator.clipboard?.writeText(shown.public_key);
    return put
      ? put.then(
          () => true,
          () => false,
        )
      : false;
  }, [key]);

  const byHand = useCallback(
    (on: SigningForge) => {
      setFlow(undefined);
      void copyKey().then((copied) => go({ step: "add", add: { at: "by-hand", copied } }));
      void openUrl(newKeyPage(on));
    },
    [copyKey, go],
  );

  /** Where the sign-in may not add the key: GitHub asks for the permission; a GitLab sign-in
   *  without `api` has none to ask for, so the key is added by hand. */
  const mayNotAdd = useCallback(
    (on: SigningForge) => {
      if (on.kind === "gitlab") byHand(on);
      else go({ step: "add", add: { at: "ask" } });
    },
    [byHand, go],
  );

  const add = useCallback(
    (on: SigningForge) => {
      go({ step: "add", add: { at: "adding" } });
      void commands.signingAddKey(on).then((added) => {
        if (added.status === "error") go({ step: "add", add: { at: "trouble", why: added.error } });
        else if (added.data === "added") check(on);
        else mayNotAdd(on);
      });
    },
    [check, go, mayNotAdd],
  );

  /** With the key made: add it where the sign-in may, else look whether it's there already
   *  (added by hand before), else ask for the permission or go by hand. */
  const onFromTheKey = useCallback(
    (read: SigningState) => {
      const found = accountFor(read, forge);
      const on: SigningForge = found
        ? { kind: found.kind, host: found.host }
        : (forge ?? { kind: "github", host: "github.com" });
      if (found?.may_add) {
        add(on);
        return;
      }
      // A key added by hand before is on the forge's public list already: Check runs on, and
      // no permission is asked for.
      void commands.signingKeyListed(on).then((listed) => {
        if (listed.status === "ok" && listed.data) check(on);
        else mayNotAdd(on);
      });
    },
    [add, check, forge, mayNotAdd],
  );

  // As it opens, once: what this device and its forge account have, and the first step that
  // needs the person. A key there already, or an account that may add it, runs on by itself.
  const opened = useRef(false);
  useEffect(() => {
    if (opened.current) return;
    opened.current = true;
    void commands.signingState().then((read) => {
      if (read.status === "error") {
        go({ step: "unread", why: read.error });
        return;
      }
      setState(read.data);
      const found = accountFor(read.data, forge);
      if (found && !found.verifies) {
        go({ step: "unread", why: cantVerify(found) });
        return;
      }
      if (!read.data.on) go({ step: "key", key: { at: "explain" } });
      else onFromTheKey(read.data);
    });
  }, [forge, go, onFromTheKey]);

  const makeKey = () => {
    go({ step: "key", key: { at: "making" } });
    void commands.signingMakeKey().then((made) => {
      if (made.status === "error") {
        go({ step: "key", key: { at: "refused", why: made.error } });
        return;
      }
      void commands.signingState().then((read) => {
        if (read.status === "ok") {
          setState(read.data);
          onFromTheKey(read.data);
        } else mayNotAdd({ kind, host });
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
          add({ kind, host });
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

  // By hand: the forge's own list is asked until the key is there, then Check runs on.
  const watching = now.step === "add" && now.add.at === "by-hand";
  const lookNow = useCallback(() => {
    const on = { kind, host };
    void commands.signingKeyListed(on).then((listed) => {
      if (listed.status === "ok" && listed.data) check(on);
    });
  }, [check, kind, host]);
  useEffect(() => {
    if (!watching) return;
    const every = setInterval(lookNow, LOOK_EVERY_MS);
    return () => clearInterval(every);
  }, [watching, lookNow]);

  const login = account?.login ?? "your account";
  const at = now.step === "key" ? 0 : now.step === "add" ? 1 : now.step === "check" ? 2 : 3;
  const rail: RailStep[] = [
    { id: "key", name: "Make your key", state: at > 0 ? "done" : "current" },
    {
      id: "add",
      name: `Add it to ${forgeName}`,
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
            return { children: `Continue to ${forgeName}`, onPress: askPermission };
          case "waiting": {
            const page = now.add.code.verification_uri;
            return { children: `Open ${forgeName}`, onPress: () => void openUrl(page) };
          }
          case "declined":
            return { children: "Try again", onPress: askPermission };
          case "by-hand":
            return { children: "I've added it", onPress: lookNow };
          case "trouble":
            return { children: "Try again", onPress: () => add({ kind, host }) };
          default:
            return {
              children: `Continue to ${forgeName}`,
              onPress: () => undefined,
              isDisabled: true,
            };
        }
      case "check":
        if (now.check.at === "nothing-pushed") return { children: "Done", onPress: onClose };
        if (now.check.at === "email" && now.check.picked !== undefined) {
          const picked = now.check.picked;
          return { children: "Use this email", onPress: () => takeEmail(picked) };
        }
        return {
          children: "Check again",
          isDisabled: now.check.at === "checking",
          onPress: () => check({ kind, host }),
        };
      case "done":
        return finish
          ? {
              children: finish.label,
              onPress: () => {
                finish.onFinish();
                onClose();
              },
            }
          : { children: "Done", onPress: onClose };
    }
  };

  const emailSettings =
    now.step === "check" &&
    now.check.at === "email" &&
    now.check.emails !== undefined &&
    now.check.emails.emails.length === 0
      ? now.check.emails.settings
      : undefined;
  const alternative =
    now.step === "add" && ["ask", "waiting", "declined"].includes(now.add.at) && !lookingBack
      ? { children: "I'll add it myself", onPress: () => byHand({ kind, host }) }
      : emailSettings !== undefined && !lookingBack
        ? {
            children: `Open ${forgeName}'s email settings`,
            onPress: () => void openUrl(emailSettings),
          }
        : undefined;

  return (
    <Dialog
      onClose={onClose}
      title="Set up signing"
      subtitle={`So ${forgeName} shows the commits purlis makes for you as Verified.`}
      className="signing-setup"
      cancel={now.step === "done" && !finish ? false : undefined}
      summary={
        now.step === "done" || now.step === "unread"
          ? undefined
          : `Step ${Math.min(at, 2) + 1} of 3`
      }
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
        <LookBack showing={showing} login={login} forge={forgeName} />
      ) : (
        <Body
          now={now}
          state={state}
          shownKey={key}
          login={login}
          forge={forgeName}
          gitlab={gitlab}
          page={newKeyPage({ kind, host })}
          onCopy={() =>
            void copyKey().then((copied) => go({ step: "add", add: { at: "by-hand", copied } }))
          }
          onPick={(email) =>
            setNow((was) =>
              was.step === "check" && was.check.at === "email"
                ? { step: "check", check: { ...was.check, picked: email, why: undefined } }
                : was,
            )
          }
        />
      )}
    </Dialog>
  );
}

/** What a done step settled on, when the person goes back to it on the rail. */
function LookBack({
  showing,
  login,
  forge,
}: {
  showing: string | null;
  login: string;
  forge: string;
}) {
  if (showing === "key") return <p>This Mac&apos;s signing key is kept in its Keychain.</p>;
  return (
    <p>
      The key is on {login} on {forge}.
    </p>
  );
}

function Body({
  now,
  state,
  shownKey,
  login,
  forge,
  gitlab,
  page,
  onCopy,
  onPick,
}: {
  now: Now;
  state?: SigningState;
  shownKey?: SigningKeyShown;
  login: string;
  forge: string;
  gitlab: boolean;
  page: string;
  onCopy: () => void;
  onPick: (email: string) => void;
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
      return (
        <AddBody
          add={now.add}
          state={state}
          shownKey={shownKey}
          login={login}
          forge={forge}
          gitlab={gitlab}
          page={page}
          onCopy={onCopy}
        />
      );
    case "check":
      return (
        <>
          <p>
            {gitlab
              ? `purlis asks ${forge} whether a signed commit a chat pushed there shows as Verified.`
              : `purlis makes a small signed test and asks ${forge} whether it shows as Verified.`}
          </p>
          {now.check.at === "checking" && (
            <p className="pending" role="status" aria-busy="true">
              Asking {forge}…
            </p>
          )}
          {now.check.at === "found" && (
            <p className="trouble said-in-full" role="alert">
              {reasonSaid(now.check.result, login, forge)}
            </p>
          )}
          {now.check.at === "email" && (
            <EmailChoice check={now.check} login={login} forge={forge} onPick={onPick} />
          )}
          {now.check.at === "nothing-pushed" && (
            <p role="status">
              Signing is on: chat commits on this Mac are signed from now on. {forge} can only check
              a commit that&apos;s there, so the ✓ shows once a chat pushes its first signed commit
              to {login}&apos;s repos.
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
            <strong>Verified on {forge} ✓</strong>
          </p>
          <p>Chat commits on this Mac are now signed in every repo, as {now.email}.</p>
        </div>
      );
  }
}

function AddBody({
  add,
  state,
  shownKey,
  login,
  forge,
  gitlab,
  page,
  onCopy,
}: {
  add: Add;
  state?: SigningState;
  shownKey?: SigningKeyShown;
  login: string;
  forge: string;
  gitlab: boolean;
  page: string;
  onCopy: () => void;
}) {
  switch (add.at) {
    case "adding":
      return (
        <p className="pending" role="status" aria-busy="true">
          {gitlab
            ? `Adding the key to ${login} on ${forge} as a signing key, with the sign-in purlis already has…`
            : `Adding the key to ${login} on ${forge}…`}
        </p>
      );
    case "ask":
    case "asking":
      return (
        <>
          <p>
            {forge} needs to know this key is yours. purlis can add it to <strong>{login}</strong>{" "}
            for you.
          </p>
          <Card as="div" variant="ask">
            <p>
              {forge} will ask you to let purlis <strong>manage your signing keys</strong> and{" "}
              <strong>read your email addresses</strong>. That&apos;s all it&apos;s for: adding this
              key, checking it, and removing it if you ask.
            </p>
          </Card>
          {add.at === "asking" && (
            <p className="pending" role="status" aria-busy="true">
              Asking {forge} for a code…
            </p>
          )}
        </>
      );
    case "waiting":
      return (
        <>
          <p>Enter this code on {forge}&apos;s page, then allow it there.</p>
          <Card as="p" variant="ask" className="forge-device-code">
            <strong>
              <code aria-label={`The code to enter on ${forge}`}>{add.code.user_code}</code>
            </strong>
          </Card>
          <SettingActions>
            <Button onPress={() => void navigator.clipboard?.writeText(add.code.user_code)}>
              Copy code
            </Button>
          </SettingActions>
          <p className="pending" role="status" aria-busy="true">
            Waiting for you to allow it on {forge}…
          </p>
        </>
      );
    case "declined":
      return (
        <>
          <p className="trouble said-in-full" role="alert">
            {add.why}
          </p>
          <p>You can try again, or add the key on {forge} yourself.</p>
        </>
      );
    case "by-hand":
      return (
        <>
          <p>Add it on {forge} yourself. purlis watches and carries on once it&apos;s there.</p>
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
              On {forge}&apos;s page ({page.replace("https://", "")}), choose{" "}
              {gitlab ? (
                <>
                  Usage type <strong>Signing</strong>
                </>
              ) : (
                <>
                  Key type <strong>Signing Key</strong>
                </>
              )}
              .
            </li>
            <li>Paste it, name it &quot;{state?.title ?? "purlis"}&quot;, and save.</li>
          </ol>
          {shownKey && (
            <Card as="p" variant="ask" className="signing-public-key">
              <code aria-label="This Mac's public signing key">{shownKey.public_key}</code>
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

/**
 * **The Check step's email choice** (#5631, ADR 0116 §6): why the email matters, in one
 * sentence, and the account's verified addresses to sign chat commits as; with none, what to do
 * on the forge, and no ✓.
 */
function EmailChoice({
  check,
  login,
  forge,
  onPick,
}: {
  check: { result: SigningCheck; emails?: SigningEmails; picked?: string; why?: string };
  login: string;
  forge: string;
  onPick: (email: string) => void;
}) {
  const emails = check.emails?.emails;
  return (
    <>
      <p className="said-in-full">
        The key is on {forge}, but your chat commits use <code>{check.result.email}</code>, which
        isn&apos;t a verified email on {login}. {forge} only verifies a commit whose email is one of
        yours.
      </p>
      {emails === undefined && check.why === undefined && (
        <p className="pending" role="status" aria-busy="true">
          Reading your verified emails on {forge}…
        </p>
      )}
      {emails !== undefined && emails.length > 0 && <p>Sign chat commits as:</p>}
      {emails !== undefined && emails.length > 0 && (
        <RadioGroup
          aria-label="Sign chat commits as"
          value={check.picked ?? null}
          onChange={onPick}
          className="signing-emails"
        >
          {emails.map((one) => (
            <Radio
              key={one.email}
              value={one.email}
              description={
                one.primary
                  ? "Verified · primary"
                  : one.private
                    ? "Verified · keeps your address private"
                    : "Verified"
              }
            >
              {one.email}
            </Radio>
          ))}
        </RadioGroup>
      )}
      {emails !== undefined && emails.length === 0 && (
        <p className="trouble said-in-full" role="alert">
          {login} has no verified email on {forge} yet. Add one in {forge}&apos;s email settings and
          verify it, then check again: {forge} shows a commit as Verified only then.
        </p>
      )}
      {check.why !== undefined && (
        <p className="trouble said-in-full" role="alert">
          {check.why}
        </p>
      )}
    </>
  );
}
