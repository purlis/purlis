// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useState } from "react";
import { commands, type PlaneId, type ProposedWhole } from "./bindings";
import { asksMoved } from "./asks";
import { DRAWING } from "./AskWrites";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { FileCode, LoaderCircle } from "lucide-react";
import { Button, ButtonRow } from "./ui/button/Button";

/** The merge view, from the editor's own chunk, loaded when a change is opened. */
const MergeViewer = lazy(() =>
  import("./editor/LightEditor").then((module) => ({ default: module.MergeViewer })),
);

/** The session of a change purlis itself proposes (#3467, `proposals::BY_PURLIS`): no chat's. */
export const BY_PURLIS = 0;

/** Tells `onDrawn` once it is drawn: inside the wait, so once the change is on the screen. */
function Drawn({ onDrawn }: { onDrawn: () => void }) {
  useEffect(() => onDrawn(), [onDrawn]);
  return null;
}

/**
 * **A proposed change, whole, in its own tab** (#1975, PC-7): one too long for a row is opened
 * here, the file as purlis reads it and as the change would make it, in the comparison view,
 * with **Apply** and **Dismiss**. Apply counts only once the change is on the screen, and the
 * app judges it again before anything is written: the file still as it was, the change still
 * shown whole, by this tab's rules. A change not shown for anything but its length (a
 * credential's shape) says why and offers Dismiss alone.
 */
export function ProposedChangeTab({ plane, id }: { plane: PlaneId; id: number }) {
  const [read, setRead] = useState<ProposedWhole | { gone: string } | undefined>();
  const [drawn, setDrawn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string>();
  const [done, setDone] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands
      .proposedChange(plane, id)
      .then((answer) => {
        if (!live) return;
        setRead(answer.status === "ok" ? answer.data : { gone: answer.error });
      })
      .catch((err: unknown) => live && setRead({ gone: String(err) }));
    return () => {
      live = false;
    };
  }, [plane, id]);
  if (done !== undefined)
    return <EmptyState mark={FileCode} headline={done} testid="proposal-answered" />;
  if (read === undefined) return <EmptyState mark={LoaderCircle} headline="Reading the change…" />;
  if ("gone" in read)
    return <EmptyState mark={FileCode} headline={read.gone} testid="proposal-gone" />;
  const answer = (apply: boolean) => {
    if (busy || (apply && !drawn)) return;
    setBusy(true);
    setSaid(undefined);
    const sent = apply
      ? commands.applyProposedChange(plane, id, "tab")
      : commands
          .dismissProposedChange(plane, id)
          .then((one) =>
            one.status === "ok"
              ? { status: "ok" as const, data: "Dismissed: nothing was written." }
              : one,
          );
    void sent
      .then((one) => (one.status === "ok" ? setDone(one.data) : setSaid(one.error)))
      .catch((err: unknown) => setSaid(String(err)))
      .finally(() => {
        setBusy(false);
        asksMoved(plane);
      });
  };
  return (
    <section className="proposed-change" aria-label={`The change proposed to ${read.path}`}>
      {read.session === BY_PURLIS ? (
        <p>
          purlis proposes this change to <code>{read.path}</code>: it takes out settings purlis no
          longer reads. Nothing is written until you apply it.
        </p>
      ) : (
        <p>
          Chat {read.session} proposes this change to <code>{read.path}</code>. It may not write the
          file itself, so only you apply it; it is told what you did on its next turn.
        </p>
      )}
      {read.why !== null && read.why !== undefined ? (
        <p className="inbox-refused" data-testid="proposal-not-shown">
          {read.why}
        </p>
      ) : (
        <Suspense fallback={<p className="inbox-refused">{DRAWING}</p>}>
          <MergeViewer
            path={read.path}
            base={read.before}
            head={read.after}
            hunks={read.hunks}
            label={`The file as it is, and as the change would make it, in ${read.path}`}
          />
          <Drawn onDrawn={() => setDrawn(true)} />
        </Suspense>
      )}
      <ButtonRow>
        {read.applies && (
          <Button variant="grants" aria-disabled={busy || !drawn} onPress={() => answer(true)}>
            Apply
          </Button>
        )}
        <Button aria-disabled={busy} onPress={() => answer(false)}>
          Dismiss
        </Button>
      </ButtonRow>
      {said !== undefined && <p className="inbox-refused">{said}</p>}
    </section>
  );
}
