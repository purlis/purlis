// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId, type RunnerDispatchWhole } from "./bindings";
import { asksMoved, onAsksMoved } from "./asks";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { LoaderCircle, Server } from "lucide-react";
import { Button, ButtonRow } from "./ui/button/Button";

/**
 * **A runner chat's held open or dispatch, whole, in its own tab**: everything Approve would
 * start on this machine, the brief whole, and what the runner chat can do with it after. Approve
 * is pressed only here, where nothing is cut; the app decides it then as a dispatch of a chat
 * here is decided. Refuse starts nothing.
 */
export function RunnerDispatchTab({ plane, id }: { plane: PlaneId; id: number }) {
  const [read, setRead] = useState<RunnerDispatchWhole | { gone: string } | undefined>();
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string>();
  const [done, setDone] = useState<string>();
  useEffect(() => {
    let live = true;
    // Read again whenever the window answers an ask of this project: one refused from its row
    // or gone with its chat says so here, rather than offering an Approve that starts nothing.
    const read = () =>
      void commands
        .runnerDispatch(plane, id)
        .then((answer) => {
          if (!live) return;
          setRead(answer.status === "ok" ? answer.data : { gone: answer.error });
        })
        .catch((err: unknown) => live && setRead({ gone: String(err) }));
    read();
    const off = onAsksMoved((moved) => moved === plane && read());
    return () => {
      live = false;
      off();
    };
  }, [plane, id]);
  if (done !== undefined)
    return <EmptyState mark={Server} headline={done} testid="runner-dispatch-answered" />;
  if (read === undefined) return <EmptyState mark={LoaderCircle} headline="Reading the ask…" />;
  if ("gone" in read)
    return <EmptyState mark={Server} headline={read.gone} testid="runner-dispatch-gone" />;
  const answer = (approve: boolean) => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    const sent = approve
      ? commands.approveRunnerDispatch(plane, id)
      : commands
          .refuseRunnerDispatch(plane, id)
          .then((one) =>
            one.status === "ok"
              ? { status: "ok" as const, data: "Refused: nothing was started." }
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
  const persona = read.persona ?? "no persona";
  return (
    <section className="runner-write" aria-label="The dispatch a runner chat asked for">
      <p>
        {read.chat} (chat {read.session}), on pool {read.pool}, asks to{" "}
        {read.handoff ? "hand its work off to" : "start a task as"} a chat on this machine. Only you
        start it; it is told what you did on its next turn.
      </p>
      <dl className="runner-dispatch-facts" data-testid="runner-dispatch-facts">
        <dt>Runs as</dt>
        <dd>{persona}</dd>
        {read.name !== "" && (
          <>
            <dt>Task</dt>
            <dd>{read.name}</dd>
          </>
        )}
        <dt>Works in</dt>
        <dd>{read.works}</dd>
        <dt>Profile</dt>
        <dd>{read.profile ?? "none this machine offers"}</dd>
        {read.long && (
          <>
            <dt>Runs</dt>
            <dd>long-running</dd>
          </>
        )}
      </dl>
      <pre className="runner-write-whole" data-testid="runner-dispatch-brief">
        {read.brief}
      </pre>
      <p>
        {read.handoff
          ? "Once it starts, the chat runs with this persona's tools and access here."
          : "Once it starts, the chat runs with this persona's tools and access here, and the runner chat can send it more instructions, read what it says, and get its report."}
      </p>
      <ButtonRow>
        {/* Approve grants a start here: drawn as every grant is (ADR 0097 §6). */}
        <Button variant="grants" aria-disabled={busy} onPress={() => answer(true)}>
          Approve
        </Button>
        <Button aria-disabled={busy} onPress={() => answer(false)}>
          Refuse
        </Button>
      </ButtonRow>
      {said !== undefined && <p className="inbox-refused">{said}</p>}
    </section>
  );
}
