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
import { useState } from "react";
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
      onClose={onKeep}
      title={`Save your changes to ${listed(names)}?`}
      titleId="unsaved-leave"
      description={
        blocked
          ? "You changed its text and haven't saved it, and it can't be saved as it is: the check found what stops it from loading. If you leave now, the changes are lost."
          : one
            ? "You changed its text and haven't saved it. If you leave now, the changes are lost."
            : "You changed their text and haven't saved it. If you leave now, the changes are lost."
      }
      cancel={{ children: "Keep editing" }}
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
 * **The window's question about `asked`'s work** (#3929). Discard and leave lets every work go
 * and goes on; Save and leave saves each, and goes on once all are saved: one that is not stays
 * asked about, with why, and those saved are no longer named. `onAnswered` takes the question
 * down, before the close goes on.
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
  const live = useUnsavedWork().filter((one) => keys.has(one.key));
  // Work let go of elsewhere meanwhile is still named, so the question never names nothing.
  const left = live.length > 0 ? live : asked.work;
  return (
    <LeaveQuestion
      names={left.map((one) => one.name)}
      busy={busy}
      refused={refused}
      onKeep={() => {
        onAnswered();
        asked.kept?.();
      }}
      onDiscard={() => {
        for (const one of left) one.discard();
        onAnswered();
        asked.then();
      }}
      onSave={() => {
        setBusy(true);
        setRefused(undefined);
        void (async () => {
          const why: string[] = [];
          for (const one of left) {
            const said = await one.save().catch((err: unknown) => String(err));
            if (said !== undefined) why.push(left.length === 1 ? said : `${one.name}: ${said}`);
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
