// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type ProposedPin as Offered } from "./bindings";
import { asksMoved } from "./asks";
import { PinToWorkspace } from "./PinToWorkspace";
import { projectName } from "./projectName";
import { REVIEW_PIN_ASK, type ReviewPinAsk } from "./proposedPinOpen";
import { AlertDialog } from "./ui/dialog/Dialog";

/**
 * **Review… on a chat's proposed pin** (#5963, ADR 0122 §1): a chat or skill may only propose a
 * pin, held on its tab and in the Inbox with Review… and Dismiss. Review… opens #5953's Pin to
 * workspace on it, which says who will see it and names the chat and the command it proposed;
 * only that dialog's Pin pins it. Cancel leaves the proposal waiting; Dismiss on the row pins
 * nothing.
 *
 * This is the project's window's half: it draws Pin to workspace for each proposal a row asks
 * about, by an event (`proposedPinOpen.ts`).
 */
export function ProposedPins({ plane }: { plane: string }) {
  const [id, setId] = useState<number>();
  useEffect(() => {
    const heard = (event: Event) => {
      const ask = (event as CustomEvent<ReviewPinAsk>).detail;
      if (ask?.plane === plane && typeof ask.id === "number") setId(ask.id);
    };
    window.addEventListener(REVIEW_PIN_ASK, heard);
    return () => window.removeEventListener(REVIEW_PIN_ASK, heard);
  }, [plane]);
  if (id === undefined) return null;
  return <ProposedPin key={id} plane={plane} id={id} onClose={() => setId(undefined)} />;
}

/** **Pin to workspace, on proposal `id`**: asks the core what was proposed, and pins only on Pin. */
export function ProposedPin({
  plane,
  id,
  onClose,
}: {
  plane: string;
  id: number;
  onClose: () => void;
}) {
  const [offered, setOffered] = useState<{ offered?: Offered; failed?: string }>({});
  const [pinning, setPinning] = useState(false);
  const [trouble, setTrouble] = useState<string>();
  useEffect(() => {
    let gone = false;
    commands.proposedPin(plane, id).then(
      (said) => {
        if (gone) return;
        setOffered(said.status === "ok" ? { offered: said.data } : { failed: said.error });
      },
      (why: unknown) => {
        if (!gone) setOffered({ failed: String(why) });
      },
    );
    return () => {
      gone = true;
    };
  }, [plane, id]);
  if (offered.failed !== undefined)
    return (
      <AlertDialog
        onClose={onClose}
        title="This pin cannot be made"
        description={offered.failed}
        cancel={{ children: "Close" }}
        data-testid="proposed-pin-refused"
      />
    );
  if (offered.offered === undefined) return null;
  const shown = offered.offered;
  return (
    <PinToWorkspace
      project={projectName(plane)}
      workspace={shown.workspace}
      ask={shown.ask}
      pinning={pinning}
      trouble={trouble}
      proposed={{ by: shown.by, command: shown.command, cwd: shown.cwd }}
      onCancel={onClose}
      onPin={(name, icon) => {
        setPinning(true);
        setTrouble(undefined);
        commands
          .pinProposed(plane, id, name, icon)
          .then(
            (said) => {
              if (said.status === "error") {
                setTrouble(said.error);
                return;
              }
              onClose();
            },
            (why: unknown) => setTrouble(String(why)),
          )
          .finally(() => {
            setPinning(false);
            asksMoved(plane);
          });
      }}
    />
  );
}
