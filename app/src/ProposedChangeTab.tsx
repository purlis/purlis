// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useState } from "react";
import { commands, type PlaneId, type ProposedWhole } from "./bindings";
import { asksMoved } from "./asks";
import { DRAWING } from "./AskWrites";
import { EmptyState } from "./EmptyState";
import { FileCode, LoaderCircle } from "lucide-react";

/** The merge view, from the editor's own chunk, loaded when a change is opened. */
const MergeViewer = lazy(() =>
  import("./editor/LightEditor").then((module) => ({ default: module.MergeViewer })),
);

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
      <p>
        Chat {read.session} proposes this change to <code>{read.path}</code>. It may not write the
        file itself, so only you apply it; it is told what you did on its next turn.
      </p>
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
      <div className="ui-button-row">
        {read.applies && (
          <button
            type="button"
            className="ui-button ui-button-primary"
            tabIndex={0}
            aria-disabled={busy || !drawn}
            onClick={() => answer(true)}
          >
            Apply
          </button>
        )}
        <button
          type="button"
          className="ui-button"
          tabIndex={0}
          aria-disabled={busy}
          onClick={() => answer(false)}
        >
          Dismiss
        </button>
      </div>
      {said !== undefined && <p className="inbox-refused">{said}</p>}
    </section>
  );
}
