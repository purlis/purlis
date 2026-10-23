// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId, type RunnerWriteWhole } from "./bindings";
import { asksMoved } from "./asks";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { LoaderCircle, Server } from "lucide-react";
import { Button, ButtonRow } from "./ui/button/Button";

/**
 * **A runner chat's change to the project, whole, in its own tab** (#2046): one its row cannot
 * show whole is opened here, every text it carries as it would be written, with **Apply** and
 * **Dismiss**. Apply is offered only where what is shown is what would be written, and the app
 * judges it again before anything is written. Dismiss writes nothing.
 */
export function RunnerWriteTab({ plane, id }: { plane: PlaneId; id: number }) {
  const [read, setRead] = useState<RunnerWriteWhole | { gone: string } | undefined>();
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string>();
  const [done, setDone] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands
      .runnerWrite(plane, id)
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
    return <EmptyState mark={Server} headline={done} testid="runner-write-answered" />;
  if (read === undefined) return <EmptyState mark={LoaderCircle} headline="Reading the change…" />;
  if ("gone" in read)
    return <EmptyState mark={Server} headline={read.gone} testid="runner-write-gone" />;
  const answer = (apply: boolean) => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    const sent = apply
      ? commands.applyRunnerWrite(plane, id, "tab")
      : commands
          .dismissRunnerWrite(plane, id)
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
    <section className="runner-write" aria-label="The change a runner chat asked for">
      <p>
        Chat {read.session}, on pool {read.pool}, asks to change this project. It may not write here
        itself, so only you apply it; it is told what you did on its next turn.
      </p>
      <pre className="runner-write-whole" data-testid="runner-write-whole">
        {read.said}
      </pre>
      {read.why !== null && read.why !== undefined && (
        <p className="inbox-refused" data-testid="runner-write-not-applied">
          {read.why}
        </p>
      )}
      <ButtonRow>
        {read.applies && (
          // Apply grants what the change writes: drawn as every grant is (ADR 0097 §6).
          <Button variant="grants" aria-disabled={busy} onPress={() => answer(true)}>
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
