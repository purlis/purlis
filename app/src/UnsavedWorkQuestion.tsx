// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The question a leave asks while work is unsaved** (#3683, #3929; spec #3679, the amendment's
 * S4): Keep editing, Discard and leave, or Save and leave.
 *
 * One question for every leave: a step inside a view that holds the work (the Skill studio's own,
 * `SkillEditInPlace.tsx`), and every close the window asks about through the registry
 * (`unsavedWork.ts`): a tab's, a pane's, a project's, a workspace's removal and a quit.
 */
import { useEffect, useState } from "react";
import { AlertDialog } from "./ui/dialog/Dialog";
import { listed } from "./taskAsks";
import { useUnsavedWork, type UnsavedWork } from "./unsavedWork";

/**
 * **The question, as it is drawn**: about the work named `names`; Save and leave waits while what
 * the view's check found blocks the save (`blocked`), or the check is still to run (`checking`),
 * and `refused` says why the last save did not go through.
 */
export function LeaveQuestion({
  names,
  blocked = false,
  checking = false,
  busy,
  refused,
  onKeep,
  onDiscard,
  onSave,
}: {
  names: readonly string[];
  /** Whether the view's check finds what keeps the work from being saved. */
  blocked?: boolean;
  /** Whether the view's check is still to look at the work as it is: Save and leave waits. */
  checking?: boolean;
  busy: boolean;
  /** Why the last Save and leave did not save, in the core's words. */
  refused?: string;
  onKeep: () => void;
  onDiscard: () => void;
  onSave: () => void;
}) {
  const one = names.length === 1;
  return (
    <AlertDialog
      // A save under way is waited for: an answer given meanwhile would race it.
      onClose={() => {
        if (!busy) onKeep();
      }}
      title={`Save your changes to ${listed(names)}?`}
      titleId="unsaved-leave"
      description={
        blocked
          ? "You changed its text and haven't saved it, and it can't be saved as it is: the check found what stops it from loading. If you leave now, the changes are lost."
          : one
            ? "You changed its text and haven't saved it. If you leave now, the changes are lost."
            : "You changed their text and haven't saved it. If you leave now, the changes are lost."
      }
      cancel={{ children: "Keep editing", isDisabled: busy }}
      alternative={{
        children: "Discard and leave",
        variant: "danger",
        isDisabled: busy,
        onPress: onDiscard,
      }}
      main={{
        children: "Save and leave",
        isDisabled: busy || blocked || checking,
        onPress: onSave,
      }}
    >
      {refused !== undefined && (
        <p className="trouble" role="alert" data-testid="unsaved-refused">
          {refused}
        </p>
      )}
    </AlertDialog>
  );
}

/**
 * **A close waiting on the person's answer about the work it would lose**: `then` carries the
 * close out, and `kept` is told where the answer was Keep editing.
 */
export type UnsavedAsk = {
  work: readonly UnsavedWork[];
  then: () => void;
  kept?: () => void;
};

/**
 * **The window's question about `asked`'s work** (#3929). Discard and leave goes on, and the work
 * goes with what holds it once the close is carried out (`unsavedWork.ts`), so a close whose own
 * question is then cancelled loses nothing. Save and leave saves each, and goes on once all are
 * saved: one that is not stays asked about, with why, and those saved are no longer named. Save
 * and leave waits while the views check whether the work could be saved as it is.
 * `onAnswered` takes the question down, before the close goes on.
 */
export function UnsavedWorkQuestion({
  asked,
  onAnswered,
}: {
  asked: UnsavedAsk;
  onAnswered: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  const keys = new Set(asked.work.map((one) => one.key));
  // Only work still registered is saved: work saved or let go of elsewhere meanwhile is not
  // written again. It is still named, so the question never names nothing.
  const live = useUnsavedWork().filter((one) => keys.has(one.key));
  const named = live.length > 0 ? live : asked.work;
  const checks = useChecks(live);
  return (
    <LeaveQuestion
      names={named.map((one) => one.name)}
      busy={busy}
      checking={checks === undefined}
      blocked={checks === false}
      refused={refused}
      onKeep={() => {
        onAnswered();
        asked.kept?.();
      }}
      onDiscard={() => {
        onAnswered();
        asked.then();
      }}
      onSave={() => {
        setBusy(true);
        setRefused(undefined);
        void (async () => {
          const why: string[] = [];
          for (const one of live) {
            const said = await one.save().catch((err: unknown) => String(err));
            if (said !== undefined) why.push(live.length === 1 ? said : `${one.name}: ${said}`);
          }
          setBusy(false);
          if (why.length > 0) return setRefused(why.join(" "));
          onAnswered();
          asked.then();
        })();
      }}
    />
  );
}

/**
 * **Whether every work in `work` could be saved as it is**, as each view checks it: `undefined`
 * while any is still checking, and asked again as the work changes. A view that cannot check
 * is taken at its word: its save says why where it is refused.
 */
function useChecks(work: readonly UnsavedWork[]): boolean | undefined {
  const [checked, setChecked] = useState<{ of: readonly UnsavedWork[]; can: boolean }>();
  // The list is a new one each render; what it holds changes only as the registry does.
  const [asked, setAsked] = useState(work);
  if (asked.length !== work.length || asked.some((one, at) => one !== work[at])) setAsked(work);
  useEffect(() => {
    let live = true;
    void Promise.all(
      asked.map((one) => (one.check ?? (() => Promise.resolve(true)))().catch(() => true)),
    ).then((each) => {
      if (live) setChecked({ of: asked, can: each.every(Boolean) });
    });
    return () => {
      live = false;
    };
  }, [asked]);
  return checked?.of === asked ? checked.can : undefined;
}
