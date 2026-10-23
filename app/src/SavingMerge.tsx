// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type PlaneId, type SaveMerge } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { FIX_STARTING_SIZE, PLANE_SAVED, tellFixStarted } from "./saving";
import { askSettingsLink, type SettingsLink } from "./settings/links";
import { AlertDialog } from "./ui/dialog/Dialog";

/** Settings › You › Accounts, where a forge is signed in to. */
const ACCOUNTS: SettingsLink = { group: "you.accounts" };

/** Whose request a Notice is about, and where its answers go. */
export type RequestOf = {
  plane: PlaneId;
  /** The workspace and the repo of a workspace repo's request; both `null` for the project's. */
  workspace: string | null;
  repo: string | null;
  /** Whose request it is, as a sentence names it: `this project`, or a repo's name. */
  name: string;
  /** `pull request` or `merge request`. */
  request: string;
  /** The request's page. */
  pr: string;
  /** The branch it goes into, where purlis can tell. */
  target: string | null;
  merge: SaveMerge;
  /** A workspace repo that purlis does not save by itself: its fix reaches the request only
   *  when the person saves the repo, and the red Notice says so. */
  needsSave?: boolean;
};

/** What the question shows: the commit and the standing a fresh look found. */
type Looked = { head: string; merge: SaveMerge };

/** The request as a sentence names it: `pull request #12`, or the noun alone. */
function numbered(of: RequestOf): string {
  return of.merge.number == null ? of.request : `${of.request} ${of.merge.number}`;
}

/**
 * **Where a save's request stands with merging** (ADR 0051, amended 2026-10-13 and 2026-10-14),
 * in the Saving view under its link, and in the Inbox for a red one.
 *
 * - **Red** is a Notice: a check failed, purlis did not merge it, and it stays open. It names
 *   each failed check, linked to its page where it has one, and offers **Fix in a chat** (a
 *   chat started at once on what failed, at the plane root or in the repo's workspace) and
 *   **Merge as it stands…**.
 * - **A wait nothing purlis reads will end** (`needs: "person"`) is a Notice with **Merge as it
 *   stands…**: no check where the repository's other requests have them, checks purlis cannot
 *   read, a required check that has not reported.
 * - **A look with no sign-in** (`needs: "sign-in"`) is a Notice with **Sign in…**.
 * - **One the forge will not let merge** says why and links to the request. It has no merge
 *   button: the person's merge passes over checks and nothing else.
 * - Anything else is a note: nothing is wrong, and purlis looks again by itself.
 *
 * **Merge as it stands looks, then asks.** The press reads the request afresh
 * (`look_at_save_request`), and the question is drawn from that look: the request, the branch
 * it goes into and, for a red one, the checks that failed, and that purlis merges without
 * waiting for checks. The answer hands back the commit and the standing the question showed,
 * and the core merges only those: if the request changed meanwhile it says so, and the
 * standing is read again. A look that finds nothing left to merge past asks nothing. What the
 * forge or purlis refuses is said here in their words, beside the link to the request.
 *
 * A check's name and link came from whoever wrote the check. The core cuts the name, makes it
 * printable and keeps a link only when it is a plain `https://` address
 * (`purlis_core::forge::checks::FailedCheck`); here both are text and an `href`, never markup.
 */
export function RequestMerge(of: RequestOf) {
  const { plane, workspace, repo, name, request, pr, merge } = of;
  /** The confirmation, while it is open, with what the fresh look it is drawn from found. */
  const [confirming, setConfirming] = useState<Looked>();
  /** Which press is on its way: one at a time, and a second press sends nothing. */
  const [busy, setBusy] = useState<"look" | "merge" | "fix">();
  const sending = useRef(false);
  /** What the last press came to, when it is worth saying: a refusal, or what happened. */
  const [said, setSaid] = useState<string>();

  const open: NoticeAction = {
    label: `Open the ${request}`,
    onPress: () => void openUrl(pr).catch(() => undefined),
  };

  /** Every reader of the save standing reads again. */
  const readAgain = () => window.dispatchEvent(new CustomEvent(PLANE_SAVED));

  /** The press that asks: a fresh look first, and the question only for what that look says
   *  can still be merged past. */
  const lookThenAsk = () => {
    if (sending.current) return;
    sending.current = true;
    setBusy("look");
    setSaid(undefined);
    void commands
      .lookAtSaveRequest(plane, workspace, repo)
      .then((answer) => {
        if (answer.status !== "ok") {
          setSaid(`purlis could not look at the ${numbered(of)}: ${answer.error}`);
          return;
        }
        const { head, merge: now } = answer.data;
        readAgain();
        if (head === null || now == null || now.state === "merged" || now.state === "queued")
          setSaid(
            `The ${numbered(of)} is no longer open for purlis to merge: it merged or was closed.`,
          );
        else if (now.state === "red" || (now.state === "waiting" && now.needs === "person"))
          setConfirming({ head, merge: now });
        else setSaid(`The ${numbered(of)} changed since purlis last looked: ${now.why}.`);
      })
      .catch((err: unknown) =>
        setSaid(`purlis could not look at the ${numbered(of)}: ${String(err)}`),
      )
      .finally(() => {
        sending.current = false;
        setBusy(undefined);
      });
  };

  const mergeIt = (shown: Looked) => {
    setConfirming(undefined);
    if (sending.current) return;
    sending.current = true;
    setBusy("merge");
    setSaid(undefined);
    void commands
      .mergeSaveRequest(plane, workspace, repo, shown.head, shown.merge.shown)
      .then((answer) => {
        if (answer.status === "ok") setSaid(answer.data);
        else setSaid(`purlis did not merge the ${numbered(of)}: ${answer.error}`);
        // The request is gone or queued, or it changed and the core kept what it found.
        readAgain();
      })
      .catch((err: unknown) => setSaid(`purlis did not merge the ${numbered(of)}: ${String(err)}`))
      .finally(() => {
        sending.current = false;
        setBusy(undefined);
      });
  };

  const fixIt = () => {
    if (sending.current) return;
    sending.current = true;
    setBusy("fix");
    setSaid(undefined);
    void commands
      .fixSaveRequest(plane, workspace, repo, FIX_STARTING_SIZE.columns, FIX_STARTING_SIZE.rows)
      .then((answer) => {
        if (answer.status === "ok") tellFixStarted(plane, answer.data);
        else setSaid(`purlis could not start a chat to fix it: ${answer.error}`);
      })
      .catch((err: unknown) => setSaid(`purlis could not start a chat to fix it: ${String(err)}`))
      .finally(() => {
        sending.current = false;
        setBusy(undefined);
      });
  };

  const merges: NoticeAction = {
    label: "Merge as it stands…",
    onPress: lookThenAsk,
    busy: busy === "look" || busy === "merge",
  };
  const fixes: NoticeAction = { label: "Fix in a chat", onPress: fixIt, busy: busy === "fix" };
  const outcome = said !== undefined && <span className="alert-said">{said}</span>;
  const confirm = confirming !== undefined && (
    <ConfirmMerge
      of={{ ...of, merge: confirming.merge }}
      onMerge={() => mergeIt(confirming)}
      onCancel={() => setConfirming(undefined)}
    />
  );
  const nextSave =
    repo === null
      ? "It stays open, and the next save goes onto it."
      : of.needsSave === true
        ? `It stays open, and the next save of ${name} goes onto it: a fix made in its clone reaches the ${request} when you save the repo.`
        : `It stays open, and the next save of ${name} goes onto it.`;

  if (merge.state === "red")
    return (
      <>
        <Notice
          cause={`save-request-red:${name}`}
          tone="trouble"
          fixes={[fixes, merges]}
          link={open}
        >
          {`The ${numbered(of)} of ${name} is red, so purlis did not merge it: ${merge.why}. ${nextSave}`}
          {merge.checks.length > 0 && <FailedChecks merge={merge} />}
          {outcome}
        </Notice>
        {confirm}
      </>
    );
  if (merge.state === "cannot-merge")
    return (
      <Notice cause={`save-request-cannot-merge:${name}`} tone="trouble" link={open}>
        {`purlis cannot merge the ${numbered(of)} of ${name} as it stands: ${merge.why}. It stays open.`}
      </Notice>
    );
  if (merge.state === "waiting" && merge.needs === "person")
    return (
      <>
        <Notice cause={`save-request-waits:${name}`} fixes={[merges]} link={open}>
          {`The ${numbered(of)} of ${name} waits, and nothing says for how long: ${merge.why}. purlis keeps looking, and merges it once every check on it has passed.`}
          {outcome}
        </Notice>
        {confirm}
      </>
    );
  if (merge.state === "waiting" && merge.needs === "sign-in")
    return (
      <Notice
        cause={`save-request-sign-in:${name}`}
        fixes={[{ label: "Sign in…", onPress: () => askSettingsLink(plane, ACCOUNTS) }]}
        link={open}
      >
        {`purlis could not look at the ${numbered(of)} of ${name}: ${merge.why}.`}
      </Notice>
    );
  const note =
    merge.state === "waiting"
      ? `purlis merges the ${request} of ${name} itself once every check on it has passed: ${merge.why}. It looks again by itself.`
      : merge.state === "merged"
        ? `purlis merged the ${request} of ${name}; the project moves onto what merged with the next look.`
        : merge.why;
  return (
    <p className="saving-note" data-merge={merge.state}>
      {said ?? note}
    </p>
  );
}

/** The checks that failed, each linked to its page where it has one. */
function FailedChecks({ merge }: { merge: SaveMerge }) {
  return (
    <ul className="saving-files" aria-label="Checks that failed">
      {merge.checks.map((check, n) => (
        <li key={`${check.name}-${n}`}>
          {check.url == null ? (
            check.name
          ) : (
            <a href={check.url} target="_blank" rel="noreferrer">
              {check.name}
            </a>
          )}
        </li>
      ))}
    </ul>
  );
}

/**
 * **Merge as it stands, asked first** (the operator's ruling P1, 2026-10-13): the request by
 * its name, the branch it goes into, the checks that failed when it is red, and that purlis
 * merges without waiting for checks. Nothing is merged until the person says so here.
 *
 * The family's `AlertDialog`, as Save all's confirmation is: an act that reaches past this machine
 * and has no undo is one the person confirms, and focus starts on Cancel.
 */
function ConfirmMerge({
  of,
  onMerge,
  onCancel,
}: {
  of: RequestOf;
  onMerge: () => void;
  onCancel: () => void;
}) {
  const into = of.target === null ? "its target branch" : of.target;
  return (
    <AlertDialog
      onClose={onCancel}
      title={`Merge the ${numbered(of)} of ${of.name} as it stands?`}
      description={
        <p className="came-back">
          {`purlis merges it into ${into} now, without waiting for its checks. `}
          {of.merge.state === "red"
            ? `${of.merge.why}. `
            : `No check has passed on it: ${of.merge.why}. `}
          A conflict, a review its branch requires and every other rule of the forge still stop it.
          There is no undo.
        </p>
      }
      // `danger`: a merge lands on the target branch, and purlis has no undo for it (D-1719-3,
      // `docs/ui-primitives.md`).
      main={{ children: "Merge as it stands", variant: "danger", onPress: onMerge }}
    >
      {of.merge.state === "red" && of.merge.checks.length > 0 && <FailedChecks merge={of.merge} />}
    </AlertDialog>
  );
}
