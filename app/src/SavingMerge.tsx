// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState } from "react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AnswerBar } from "./AnswerBar";
import { commands, type PlaneId, type SaveMerge } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { FIX_STARTING_SIZE, PLANE_SAVED, tellFixStarted } from "./saving";
import { askSettingsLink, type SettingsLink } from "./settings/links";

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
};

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
 * **Merge as it stands asks first**, naming the request, the branch it goes into and, for a
 * red one, the checks that failed, and saying purlis merges without waiting for checks. What
 * the forge or purlis then refuses is said here in their words, beside the link to the request.
 *
 * A check's name and link came from whoever wrote the check. The core cuts the name, makes it
 * printable and keeps a link only when it is a plain `https://` address
 * (`purlis_core::forge::checks::FailedCheck`); here both are text and an `href`, never markup.
 */
export function RequestMerge(of: RequestOf) {
  const { plane, workspace, repo, name, request, pr, merge } = of;
  /** The confirmation, while it is open. */
  const [confirming, setConfirming] = useState(false);
  /** Which press is on its way: one at a time, and a second press sends nothing. */
  const [busy, setBusy] = useState<"merge" | "fix">();
  const sending = useRef(false);
  /** What the last press came to, when it is worth saying: a refusal, or what happened. */
  const [said, setSaid] = useState<string>();

  const open: NoticeAction = {
    label: `Open the ${request}`,
    onPress: () => void openUrl(pr).catch(() => undefined),
  };

  const mergeIt = () => {
    setConfirming(false);
    if (sending.current) return;
    sending.current = true;
    setBusy("merge");
    setSaid(undefined);
    void commands
      .mergeSaveRequest(plane, workspace, repo)
      .then((answer) => {
        if (answer.status === "ok") {
          setSaid(answer.data);
          // Every reader of the save standing reads again: the request is gone, or queued.
          window.dispatchEvent(new CustomEvent(PLANE_SAVED));
        } else setSaid(`purlis did not merge the ${numbered(of)}: ${answer.error}`);
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
    onPress: () => setConfirming(true),
    busy: busy === "merge",
  };
  const fixes: NoticeAction = { label: "Fix in a chat", onPress: fixIt, busy: busy === "fix" };
  const outcome = said !== undefined && <span className="alert-said">{said}</span>;
  const confirm = confirming && (
    <ConfirmMerge of={of} onMerge={mergeIt} onCancel={() => setConfirming(false)} />
  );

  if (merge.state === "red")
    return (
      <>
        <Notice
          cause={`save-request-red:${name}`}
          tone="trouble"
          fixes={[fixes, merges]}
          link={open}
        >
          {`The ${numbered(of)} of ${name} is red, so purlis did not merge it: ${merge.why}. It stays open, and the next save goes onto it.`}
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
 * Radix's `AlertDialog`, as Save all's confirmation is: an act that reaches past this machine
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
  const cancel = useRef<HTMLButtonElement>(null);
  const into = of.target === null ? "its target branch" : of.target;
  return (
    <AlertDialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="ui-scrim" />
        <AlertDialog.Content
          className="ui-dialog warning"
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            cancel.current?.focus();
          }}
        >
          <AlertDialog.Title>{`Merge the ${numbered(of)} of ${of.name} as it stands?`}</AlertDialog.Title>
          <AlertDialog.Description className="came-back">
            {`purlis merges it into ${into} now, without waiting for its checks. `}
            {of.merge.state === "red"
              ? `${of.merge.why}. `
              : `No check has passed on it: ${of.merge.why}. `}
            A conflict, a review its branch requires and every other rule of the forge still stop
            it. There is no undo.
          </AlertDialog.Description>
          {of.merge.state === "red" && of.merge.checks.length > 0 && (
            <FailedChecks merge={of.merge} />
          )}
          <AnswerBar>
            <AlertDialog.Cancel asChild>
              <button className="ui-button" type="button" tabIndex={0} ref={cancel}>
                Cancel
              </button>
            </AlertDialog.Cancel>
            {/* `ends-it`: a merge lands on the target branch, and purlis has no undo for it
                (D-1719-3, `docs/ui-primitives.md`). */}
            <button
              type="button"
              className="ui-button ui-button-danger ends-it"
              tabIndex={0}
              onClick={onMerge}
            >
              Merge as it stands
            </button>
          </AnswerBar>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
