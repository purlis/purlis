// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { LoaderCircle, Sparkles } from "lucide-react";
import { commands, type RunEnd, type RunStart, type StudioRun } from "./bindings";
import { saidAt } from "./dispatches";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { SkillLife } from "./SkillLife";

/** What started a run, as its row says it. */
export function startedBySaid(started: RunStart | null): string {
  switch (started) {
    case "skill-tool":
      return "The chat's Skill tool";
    case "read":
      return "The chat read the skill";
    case "mcp-load":
      return "The chat loaded the skill";
    case "use-skill":
      return "You, with Use skill…";
    case null:
      return "From before runs were recorded";
    default:
      return "Not known";
  }
}

/** How a run ended, as its row says it. */
export function endedSaid(ended: RunEnd | null, fromBefore: boolean): string {
  switch (ended) {
    case "turn":
      return "With its turn";
    case "interrupted":
      return "Interrupted";
    case "session":
      return "With the chat";
    case null:
      return fromBefore ? "Not recorded" : "Running, or not recorded";
    default:
      return "Not known";
  }
}

/** A run's time, as a person reads it: `850 ms`, `12 s`, `3 min 4 s`. */
export function timeSaid(ms: number | null): string {
  if (ms === null) return "";
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${Math.floor(ms / 1000)} s`;
  return `${Math.floor(ms / 60_000)} min ${Math.floor((ms % 60_000) / 1000)} s`;
}

/** Where a run nests: the skill whose run it started inside, and the skills that ran inside it. */
export function nestingSaid(run: StudioRun): string {
  const parts: string[] = [];
  if (run.inside !== null) parts.push(`Inside ${run.inside}`);
  if (run.ran.length > 0) parts.push(`Ran ${run.ran.join(", ")}`);
  return parts.join("; ");
}

/** A signal's cell: its count, and empty where the run has no end recorded to count from. */
function signal(n: number | null): string {
  return n === null ? "" : String(n);
}

/**
 * **One skill's tab** (SK-3, #2766), opened from a skill run's chip or the Skill studio. Its
 * state, owners, history and uses come first (SK-19, #2782, `SkillLife`). Its Runs panel lists
 * every run of the skill, newest first, with what started it and its signals: tool calls, tool
 * errors, refusals, asks, tokens and time, and the skill run it nests in or holds. Never a pass
 * or a fail: those come from grading.
 */
export function SkillTab({
  plane,
  name,
  run: wanted,
  changed,
}: {
  plane: string;
  name: string;
  /** The run the tab was opened on, from a run's chip: marked in the Runs panel. */
  run?: string;
  changed: number;
}) {
  const [runs, setRuns] = useState<StudioRun[]>();
  const [refused, setRefused] = useState<string>();
  const shown = useRef<HTMLTableRowElement>(null);
  useEffect(() => {
    let live = true;
    void commands.skillRuns(plane, name).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") {
        setRuns(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, name, changed]);
  useEffect(() => {
    shown.current?.scrollIntoView?.({ block: "nearest" });
  }, [runs]);

  return (
    <div className="skill-tab" data-testid="skill-tab">
      <h2 className="skill-tab-name">
        <code>{name}</code>
      </h2>
      <SkillLife plane={plane} name={name} changed={changed} />
      <section aria-labelledby="skill-runs-heading" data-testid="skill-runs">
        <h3 id="skill-runs-heading">Runs</h3>
        {refused !== undefined ? (
          <EmptyState
            headline={`purlis could not read the runs of ${name}`}
            body={refused}
            testid="skill-runs-refused"
          />
        ) : runs === undefined ? (
          <p className="pending" aria-busy="true">
            <LoaderCircle className="node-icon spinning" />
            Reading the skill's runs…
          </p>
        ) : runs.length === 0 ? (
          <EmptyState
            headline={`No runs of ${name} yet`}
            body="A run starts when a chat uses the skill, and ends with its turn."
            mark={Sparkles}
            testid="skill-runs-empty"
          />
        ) : (
          <>
            <p className="honest">
              Each time a chat used the skill, from then until its turn ended. What happened inside
              it, never a verdict.
            </p>
            <table className="ui-table" aria-label={`Runs of ${name}`}>
              <thead>
                <tr>
                  <th scope="col">Started</th>
                  <th scope="col">Persona</th>
                  <th scope="col">Started by</th>
                  <th scope="col">Ended</th>
                  <th scope="col">Tool calls</th>
                  <th scope="col">Errors</th>
                  <th scope="col">Refused</th>
                  <th scope="col">Asks</th>
                  <th scope="col">Tokens</th>
                  <th scope="col">Time</th>
                  <th scope="col">Nested</th>
                </tr>
              </thead>
              <tbody>
                {runs.map((run, at) => {
                  const isWanted = run.id !== null && run.id === wanted;
                  return (
                    <tr
                      key={run.id ?? `before-${at}`}
                      ref={isWanted ? shown : undefined}
                      data-testid={run.id === null ? "skill-run" : `skill-run-${run.id}`}
                      data-wanted={isWanted || undefined}
                      aria-current={isWanted || undefined}
                    >
                      <td>{saidAt(run.started)}</td>
                      <td>{run.persona ?? "None"}</td>
                      <td>{startedBySaid(run.started_by)}</td>
                      <td>{endedSaid(run.ended, run.id === null)}</td>
                      <td>{signal(run.tool_calls)}</td>
                      <td>{signal(run.tool_errors)}</td>
                      <td>{signal(run.refusals)}</td>
                      <td>{signal(run.asks)}</td>
                      <td>{run.ended === null ? "" : (run.tokens ?? "Not reported")}</td>
                      <td>{timeSaid(run.ms)}</td>
                      <td>{nestingSaid(run)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </>
        )}
      </section>
    </div>
  );
}
