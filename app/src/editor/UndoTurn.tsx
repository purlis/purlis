// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Undo a turn** (#5411; ADR 0081 as amended 2026-11-05): on the file diff's latest turn not yet
 * undone, a button that asks once, listing every file the undo will change, before it puts them
 * back as they were before the turn. It starts on Cancel, so ⏎ changes nothing; nothing is
 * committed and no branch moves, which the question says.
 */
import { useState } from "react";
import { Button } from "../ui/button/Button";
import { AlertDialog } from "../ui/dialog/Dialog";

export function UndoTurn({
  number,
  files,
  undoing,
  onUndo,
}: {
  /** The turn's number. */
  number: number;
  /** Every path the undo writes, sorted, as the core lists them. */
  files: string[];
  /** An undo on its way: the button waits for it. */
  undoing: boolean;
  onUndo: () => void;
}) {
  const [asking, setAsking] = useState(false);
  const back =
    files.length === 1
      ? "This file goes back as it was before the turn."
      : `These ${files.length} files go back as they were before the turn.`;
  return (
    <>
      <Button size="sm" isDisabled={undoing} onPress={() => setAsking(true)}>
        {`Undo turn ${number}…`}
      </Button>
      {asking && (
        <AlertDialog
          onClose={() => setAsking(false)}
          title={`Undo turn ${number}?`}
          description={<p>{`${back} Nothing is committed.`}</p>}
          main={{
            children: `Undo turn ${number}`,
            variant: "danger",
            onPress: () => {
              setAsking(false);
              onUndo();
            },
          }}
        >
          <ul aria-label={`Files turn ${number} changed`}>
            {files.map((file) => (
              <li key={file}>{file}</li>
            ))}
          </ul>
        </AlertDialog>
      )}
    </>
  );
}
