// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useId, useState } from "react";
import { LoaderCircle } from "lucide-react";
import {
  commands,
  SKILL_RUNS_PANEL,
  type SkillLevel,
  type SkillPanel,
  type SkillPanelKey,
  type SkillStats,
  type StatsGroup,
  type StatsRow,
} from "./bindings";
import type { Offer } from "./actions";
import { counted } from "./dispatches";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { SkillDetailsPanel } from "./SkillDetails";
import { AnsweredBlocks } from "./Views";

/** A run's time, as a person reads it: `850 ms`, `12 s`, `3 min 4 s`. */
export function timeSaid(ms: number | null): string {
  if (ms === null) return "";
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${Math.floor(ms / 1000)} s`;
  return `${Math.floor(ms / 60_000)} min ${Math.floor((ms % 60_000) / 1000)} s`;
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
      <h2 id="skill-totals-heading">Totals</h2>
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

/** Whether two levels are one: the same place, shared or this machine's alike. */
function sameLevel(a: SkillLevel, b: SkillLevel): boolean {
  return (
    a.at === b.at &&
    a.local === b.local &&
    ("name" in a ? a.name : null) === ("name" in b ? b.name : null)
  );
}

/**
 * **One skill's tab** (SK-3, #2766), opened from a skill run's chip or the Skill studio. Its
 * state, owners, history and uses come first (SK-19, #2782, `SkillDetailsPanel`), then its
 * totals by what it ran on, by persona and by workspace (SK-20). Its other panels are studio
 * panels (ADR 0101 §6), each answered in panel blocks by the core and drawn
 * here as any view's answer is: purlis's Runs panel first, which lists every run of the skill,
 * newest first, with what started it and its signals (tool calls, tool errors, refusals, asks,
 * tokens and time, never a pass or a fail), then each panel an extension adds.
 */
export function SkillTab({
  plane,
  name,
  run: wanted,
  changed,
  offerFor,
  onPress,
}: {
  plane: string;
  name: string;
  /** The run the tab was opened on, from a run's chip: marked in the Runs panel. */
  run?: string;
  changed: number;
  /** The catalogue, for the ways out purlis's own panels offer. */
  offerFor?: (id: string) => Offer | undefined;
  onPress?: (offer: Offer) => void;
}) {
  const [panels, setPanels] = useState<SkillPanel[]>();
  const [refused, setRefused] = useState<string>();
  // A grade that ended (SK-16): its scorecard and the history's grade are read again.
  const [graded, setGraded] = useState(0);
  const regraded = useCallback(() => setGraded((n) => n + 1), []);
  // The copy the details show (#3353): the Grades panel is of that copy alone. Until the details
  // say, the core reads the copy they show first.
  const [level, setLevel] = useState<SkillLevel | null>(null);
  const shownCopy = useCallback(
    (next: SkillLevel) => setLevel((was) => (was !== null && sameLevel(was, next) ? was : next)),
    [],
  );
  useEffect(() => {
    let live = true;
    void commands.skillPanels(plane, name, level).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") {
        setPanels(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, name, changed, graded, level]);

  return (
    <div className="skill-tab" data-testid="skill-tab">
      {/* No heading with the skill's name: its tab says it (#3147). */}
      <SkillDetailsPanel
        plane={plane}
        name={name}
        changed={changed + graded}
        onGraded={regraded}
        onCopy={shownCopy}
      />
      <SkillTotals plane={plane} name={name} changed={changed} />
      {refused !== undefined ? (
        <EmptyState
          headline={`purlis could not read the panels of ${name}`}
          body={refused}
          testid="skill-panels-refused"
        />
      ) : panels === undefined ? (
        <p className="pending" aria-busy="true">
          <LoaderCircle className="node-icon spinning" />
          Reading the skill's panels…
        </p>
      ) : (
        panels.map((panel) => (
          <StudioPanelSection
            key={keyOf(panel.key)}
            panel={panel}
            name={name}
            current={wanted}
            offerFor={panel.key.kind === SKILL_RUNS_PANEL ? offerFor : undefined}
            onPress={panel.key.kind === SKILL_RUNS_PANEL ? onPress : undefined}
          />
        ))
      )}
    </div>
  );
}

/** What a panel is held by: `runs`, or `<extension>/<panel>` for one an extension adds. */
export function keyOf(key: SkillPanelKey): string {
  return key.kind === "extension" ? `${key.extension}/${key.panel}` : key.kind;
}

/** One panel on a skill's tab: its heading, who adds it, and its blocks or why it has none. An
 *  extension's panel is handed no catalogue: its blocks run no verb of purlis's. */
function StudioPanelSection({
  panel,
  name,
  current,
  offerFor,
  onPress,
}: {
  panel: SkillPanel;
  name: string;
  current?: string;
  offerFor?: (id: string) => Offer | undefined;
  onPress?: (offer: Offer) => void;
}) {
  const heading = useId();
  return (
    <section
      aria-labelledby={heading}
      data-testid={
        panel.key.kind === SKILL_RUNS_PANEL ? "skill-runs" : `skill-panel-${keyOf(panel.key)}`
      }
    >
      <h2 id={heading}>
        {panel.title}
        {panel.by !== null && <span className="quiet">{` · ${panel.by}`}</span>}
      </h2>
      {panel.refused !== null ? (
        <EmptyState
          variant="panel"
          headline={`${panel.title} could not be shown for ${name}`}
          body={panel.refused}
          testid="skill-panel-refused"
        />
      ) : (
        <AnsweredBlocks
          blocks={panel.blocks}
          label={panel.title}
          current={current}
          offerFor={offerFor}
          onPress={onPress}
        />
      )}
    </section>
  );
}
