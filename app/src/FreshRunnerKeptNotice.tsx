// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type FreshKept, type FreshRunnersKept, type PlaneId } from "./bindings";
import { listen } from "./here";
import { Notice } from "./Notice";

/** The event the core sends each time a project's kept fresh runners change. */
export const FRESH_RUNNERS_KEPT = "fresh-runners-kept";

/** How many of a runner's missing pieces the line names before it counts the rest. */
const NAMED_AT_MOST = 2;

/** What the core said is kept, as a list: a window test's stand-in may answer nothing. */
function listed(said: unknown): readonly FreshKept[] {
  return Array.isArray(said) ? (said as FreshKept[]) : [];
}

/** What is missing, in at most a few of the core's sentences. */
function missingSaid(why: readonly string[]): string {
  const named = why.slice(0, NAMED_AT_MOST).join(" ");
  const more = why.length - NAMED_AT_MOST;
  return more > 0 ? `${named} And ${more} more.` : named;
}

/**
 * **A task's runner kept, since its work is not all here** (#4569, the steward's decision of
 * 2026-11-02).
 *
 * A task on a cluster pool runs on a runner of its own, which goes only once purlis has proved
 * this project holds every commit and change it holds. Where that proof fails, the runner stays,
 * which costs money but keeps the work, and this says so: what is not here, with **Fetch
 * again**, and **Discard**, which asks once more before the runner goes with what it holds. A
 * runner its cluster says is gone already is said too, with **Forget the runner**. Each goes once
 * the core says its runner went (`fresh-runners-kept`).
 */
export function FreshRunnerKeptNotice({ plane }: { plane: PlaneId }) {
  const [kept, setKept] = useState<{ plane: PlaneId; kept: readonly FreshKept[] }>();
  // The runners whose Discard was pressed once, by their chat's id: the next press discards.
  const [confirming, setConfirming] = useState<ReadonlySet<string>>(new Set());
  // What a press was refused with, by the chat's id.
  const [said, setSaid] = useState<Readonly<Record<string, string>>>({});
  // The runners put away until the core says something new of them.
  const [putAway, setPutAway] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    // A word heard before the read answered is newer than the read, and wins.
    let heard = false;
    void (async () => {
      try {
        const unlisten = await listen<FreshRunnersKept>(FRESH_RUNNERS_KEPT, (event) => {
          if (event.payload.plane !== plane) return;
          heard = true;
          setKept({ plane, kept: listed(event.payload.kept) });
          setPutAway(new Set());
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
      try {
        const now = await commands.freshRunnersKept(plane);
        if (!gone && !heard && now.status === "ok") setKept({ plane, kept: listed(now.data) });
      } catch {
        // What cannot be read shows nothing.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);
  const runners = kept?.plane === plane ? kept.kept : [];
  const refused = (chat: string, why: string | undefined) =>
    setSaid((was) => {
      const now = { ...was };
      if (why === undefined) Reflect.deleteProperty(now, chat);
      else now[chat] = why;
      return now;
    });
  const fetchAgain = (chat: string) =>
    void commands
      .fetchFreshRunnerAgain(plane, chat)
      .then((done) => refused(chat, done.status === "error" ? done.error : undefined));
  const discard = (chat: string) => {
    if (!confirming.has(chat)) {
      setConfirming((was) => new Set([...was, chat]));
      return;
    }
    void commands
      .discardFreshRunner(plane, chat)
      .then((done) => refused(chat, done.status === "error" ? done.error : undefined));
  };
  return runners
    .filter((runner) => !putAway.has(runner.chat))
    .map((runner) => {
      const asked = confirming.has(runner.chat);
      const dismiss = () => setPutAway((was) => new Set([...was, runner.chat]));
      const refusal = said[runner.chat] !== undefined && (
        <> purlis could not do that: {said[runner.chat]}</>
      );
      if (runner.gone)
        return (
          <Notice
            key={runner.chat}
            cause={`fresh-runner-gone:${plane}:${runner.chat}`}
            tone="trouble"
            label="Runner gone"
            fixes={[{ label: "Forget the runner", onPress: () => discard(runner.chat) }]}
            onDismiss={dismiss}
          >
            A task's runner in workspace {runner.workspace} is gone from its cluster, and what it
            held that is not here went with it. {missingSaid(runner.why)}
            {asked && <> Press Forget the runner again to stop naming it.</>}
            {refusal}
          </Notice>
        );
      return (
        <Notice
          key={runner.chat}
          cause={`fresh-runner-kept:${plane}:${runner.chat}`}
          tone="attention"
          label="Runner kept"
          fixes={[
            { label: "Fetch again", onPress: () => fetchAgain(runner.chat) },
            {
              label: asked ? "Discard for good" : "Discard",
              onPress: () => discard(runner.chat),
            },
          ]}
          onDismiss={dismiss}
        >
          A task's runner in workspace {runner.workspace} is kept, since some of its work is not
          here. {missingSaid(runner.why)}
          {asked && <> Its work that is not here is lost once you discard it.</>}
          {refusal}
        </Notice>
      );
    });
}
