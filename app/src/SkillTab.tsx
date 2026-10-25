// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { LoaderCircle, Sparkles } from "lucide-react";
import {
  commands,
  type RunEnd,
  type RunStart,
  type SkillStats,
  type StatsGroup,
  type StatsRow,
  type StudioRun,
} from "./bindings";
import { counted, saidAt } from "./dispatches";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { SkillDetailsPanel } from "./SkillDetails";

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
    case "curation":
      return "A curation action";
    case "trigger":
      return "A trigger";
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

/** Which group a totals row is of, as it says it: what it ran on, or a persona or a workspace
 *  by name, with `none` said in the grouping's own words. What a run ran on is never none: a
 *  run says it, or its start does not. */
export function groupSaid(group: StatsGroup, none?: string): string {
  switch (group.kind) {
    case "named":
      return group.name;
    case "none":
      return none ?? "Not recorded";
    default:
      return "Not recorded";
  }
}

/** The groupings a skill's totals are shown by, each with what its `none` group is. */
const GROUPINGS: readonly {
  key: keyof SkillStats;
  heading: string;
  column: string;
  none?: string;
}[] = [
  { key: "by_harness", heading: "By what it ran on", column: "Ran on" },
  { key: "by_persona", heading: "By persona", column: "Persona", none: "No persona" },
  { key: "by_workspace", heading: "By workspace", column: "Workspace", none: "Project root" },
];

/**
 * **A skill's totals** (SK-20, #2783): its runs, their signals and their tokens, by what they ran
 * on, by persona and by workspace. Signals only, never a pass or a fail.
 */
function SkillTotals({ plane, name, changed }: { plane: string; name: string; changed: number }) {
  const [stats, setStats] = useState<SkillStats>();
  const [refused, setRefused] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands.skillStats(plane, name).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") {
        setStats(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, name, changed]);
  if (refused !== undefined)
    return (
      <EmptyState
        headline={`purlis could not total the runs of ${name}`}
        body={refused}
        testid="skill-totals-refused"
      />
    );
  if (stats === undefined || stats.by_persona.length === 0) return null;
  return (
    <section aria-labelledby="skill-totals-heading" data-testid="skill-totals">
      <h3 id="skill-totals-heading">Totals</h3>
      {GROUPINGS.map(({ key, heading, column, none }) => (
        <table
          key={key}
          className="ui-table"
          aria-label={`Runs of ${name}, ${heading.toLowerCase()}`}
          data-testid={`skill-totals-${key}`}
        >
          <caption>{heading}</caption>
          <thead>
            <tr>
              <th scope="col">{column}</th>
              <th scope="col">Runs</th>
              <th scope="col">Tool calls</th>
              <th scope="col">Errors</th>
              <th scope="col">Refused</th>
              <th scope="col">Asks</th>
              <th scope="col">Tokens</th>
              <th scope="col">Time</th>
            </tr>
          </thead>
          <tbody>
            {stats[key].map((row: StatsRow) => (
              <tr key={`${row.group.kind}:${row.group.kind === "named" ? row.group.name : ""}`}>
                <th scope="row">{groupSaid(row.group, none)}</th>
                <td>
                  {counted(row.runs, "run", "runs")}
                  {row.ended < row.runs ? `, ${row.ended} ended` : ""}
                </td>
                <td>{row.tool_calls}</td>
                <td>{row.tool_errors}</td>
                <td>{row.refusals}</td>
                <td>{row.asks}</td>
                <td>{row.tokens ?? "Not reported"}</td>
                <td>{timeSaid(row.ms)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ))}
    </section>
  );
}

/** A signal's cell: its count, and empty where the run has no end recorded to count from. */
function signal(n: number | null): string {
  return n === null ? "" : String(n);
}

/**
 * **One skill's tab** (SK-3, #2766), opened from a skill run's chip or the Skill studio. Its
 * state, owners, history and uses come first (SK-19, #2782, `SkillDetailsPanel`); its totals add
 * its runs up by what they ran on, by persona and by workspace (SK-20). Its Runs panel lists every
 * run of the skill, newest first, with what started it, what it ran on and its signals: tool
 * calls, tool errors, refusals, asks, tokens and time, and the skill run it nests in or holds.
 * Never a pass or a fail: those come from grading.
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
      {/* No heading with the skill's name: its tab says it (#3147). */}
      <SkillDetailsPanel plane={plane} name={name} changed={changed} />
      <SkillTotals plane={plane} name={name} changed={changed} />
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
                  <th scope="col">Ran on</th>
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
                      <td>{run.ran_on ?? "Not recorded"}</td>
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
