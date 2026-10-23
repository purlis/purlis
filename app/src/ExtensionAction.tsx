// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { commands, type ActionAnswer, type PlaneId, type RowAction } from "./bindings";
import { AlertDialog } from "./ui/dialog/Dialog";

/**
 * **An extension's action, run** — from a row of its view or from a palette command
 * (charter-app#341, ADR 0053).
 *
 * The core decides everything that matters: the gate re-taken at the press, the one process,
 * and the refusal of an action that asks first without a yes (`purlis_core::executor::act`).
 * What the window does is ask — when the action's manifest says to, and always when it deletes
 * (`RowAction.asks_first`, which the core computed) — and say what came back.
 */

/** Where an action was pressed: the view and its persona, and the row — or none of them, for a
 *  palette command. */
export type Pressed = {
  view: string | null;
  key: string;
  row: string | null;
};

/** What running it came to: the core's answer, or its refusal in its own words. */
export type Outcome = { answer: ActionAnswer } | { refused: string };

/** Ask the core to run `action`, with the operator's yes or without it. */
export async function runExtensionAction(
  plane: PlaneId,
  extension: string,
  action: RowAction,
  pressed: Pressed,
  workspace: string | undefined,
  confirmed: boolean,
): Promise<Outcome> {
  try {
    const said = await commands.runAction(
      plane,
      extension,
      action.id,
      pressed.view,
      pressed.key,
      pressed.row,
      workspace ?? null,
      confirmed,
    );
    return said.status === "error" ? { refused: said.error } : { answer: said.data };
  } catch (err: unknown) {
    return { refused: String(err) };
  }
}

/** What pressing Run came to, for the dialog: a refusal to show and let the operator try
 *  again or cancel, a sentence about what it ran to show before it closes, or nothing — the
 *  caller closes it. */
export type Answered = { refused: string } | { seen: string } | undefined;

/**
 * The question charter asks before an action that asks first. **Cancel has the focus**, as in
 * every dialog here that can end something: an Enter pressed out of habit runs nothing.
 *
 * `onRun` answers what to show: a refusal, or — from a palette command, which has no view to
 * say it above — what changed outside the extension's declared paths, after which the only
 * button left is Close.
 */
export function AskFirst({
  extension,
  action,
  onRun,
  onCancel,
}: {
  extension: string;
  action: RowAction;
  onRun: () => Promise<Answered>;
  onCancel: () => void;
}) {
  const [trouble, setTrouble] = useState<string>();
  const [seen, setSeen] = useState<string>();
  const [running, setRunning] = useState(false);
  const run = () => {
    setRunning(true);
    void onRun().then((ran) => {
      if (ran === undefined) return;
      if ("refused" in ran) setTrouble(ran.refused);
      else {
        setTrouble(undefined);
        setSeen(ran.seen);
      }
      setRunning(false);
    });
  };
  const verb = action.deletes ? "Delete" : "Run";
  const doing = action.deletes ? "Deleting…" : "Running…";
  return (
    <AlertDialog
      onClose={() => {
        // Not while the core is running it: the answer would land on a dialog nobody sees.
        if (!running) onCancel();
      }}
      title={
        <>
          Run “{action.title}” from {extension}?
        </>
      }
      description={
        <p className="came-back">
          {action.deletes
            ? "This action deletes. purlis asks before every action that deletes. The extension decides what it deletes. purlis does not stop it."
            : `${extension} asks purlis to ask you before it runs this.`}
        </p>
      }
      cancel={{
        children: seen === undefined ? "Cancel" : "Close",
        reserve: ["Cancel", "Close"],
        isDisabled: running,
      }}
      main={{
        // Said while the core runs it (D-630-3): a button that only greys out says nothing
        // about whether the press was taken. Once it has run, it stays where it was, done.
        children: running ? doing : verb,
        reserve: [verb, doing],
        variant: action.deletes ? "danger" : "secondary",
        isDisabled: running || seen !== undefined,
        onPress: run,
      }}
    >
      {(trouble ?? seen) !== undefined && (
        <p className="trouble" role="alert">
          {trouble ?? seen}
        </p>
      )}
    </AlertDialog>
  );
}
