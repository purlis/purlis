// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a skill's page reads beyond the studio's answer** (#3682): its totals by what it ran
 * on, by persona and by workspace (SK-20), and its studio panels (ADR 0101 §6): purlis's Runs
 * and Grades panels, and each panel an extension adds, answered in panel blocks by the core.
 */
import { useEffect, useId, useState } from "react";
import { RotateCw } from "lucide-react";
import {
  SKILL_RUNS_PANEL,
  type SkillPanel,
  type SkillPanelKey,
  type SkillStats,
  type StatsGroup,
  type StatsRow,
} from "./bindings";
import type { Offer } from "./actions";
import { counted } from "./dispatches";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";
import { AnsweredBlocks, type RowOpens } from "./Views";

/** What a read of the core answered: its data, or why it was refused; nothing while it is read. */
export type Read<T> = { data?: T; refused?: string };

/**
 * **One read of the core, again whenever `again` changes** (a skill's page, #3682): what it
 * answered, kept until the next answer, so a page drawn again never blanks what it showed.
 */
export function useRead<T>(
  read: () => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
  again: readonly unknown[],
): Read<T> {
  const [answer, setAnswer] = useState<Read<T>>({});
  useEffect(() => {
    let live = true;
    void read()
      .then((said) => {
        if (!live) return;
        setAnswer(said.status === "ok" ? { data: said.data } : { refused: said.error });
      })
      .catch((err: unknown) => {
        if (live) setAnswer({ refused: String(err) });
      });
    return () => {
      live = false;
    };
    // The read is the caller's, asked again on what it names.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, again);
  return answer;
}

/**
 * **A read on its way, as every studio page holds it** (S11, #3662; #3978): the layout stays,
 * with placeholder rows, and after about a second a line says what is being read. `testid` is
 * the page's own.
 */
export function ReadingRows({
  what,
  rows = 3,
  testid,
}: {
  /** What is read, as the sentence names it: "the grades of deploy". */
  what: string;
  rows?: number;
  testid: string;
}) {
  const [late, setLate] = useState(false);
  useEffect(() => {
    const after = window.setTimeout(() => setLate(true), 1000);
    return () => window.clearTimeout(after);
  }, []);
  return (
    <div className="skill-studio-placeholders" aria-busy="true" data-testid={testid}>
      {Array.from({ length: rows }, (_, at) => (
        <div key={at} className="skill-studio-placeholder" aria-hidden="true" />
      ))}
      <p role="status" className="honest">
        {late ? `Reading ${what}…` : ""}
      </p>
    </div>
  );
}

/**
 * **A read that failed, as every studio page says it** (S11, #3662; #3978): in place, where its
 * data would be, with why and Try again, never hiding the rest of the page. `testid` is the
 * page's own.
 */
export function NotRead({
  what,
  why,
  onAgain,
  testid,
  variant = "panel",
  size,
}: {
  /** What was read, as the sentence names it: "the grades of deploy". */
  what: string;
  why: string | undefined;
  onAgain: () => void;
  testid: string;
  /** A panel inside a page, or the page's whole list. */
  variant?: "panel" | "page";
  /** Its Try again's size, where the page's rows are compact. */
  size?: "sm";
}) {
  return (
    <EmptyState
      variant={variant}
      headline={`purlis could not read ${what}`}
      body={why}
      role="status"
      testid={testid}
      action={
        <Button size={size} icon={<RotateCw aria-hidden="true" />} onPress={onAgain}>
          Try again
        </Button>
      }
    />
  );
}

/** How many runs a skill's totals count, all told. */
export function runsOf(stats: SkillStats): number {
  return stats.by_harness.reduce((all, row) => all + row.runs, 0);
}

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
export function SkillTotals({
  name,
  stats,
  refused,
}: {
  name: string;
  /** Its totals, once read. */
  stats: SkillStats | undefined;
  /** Why they could not be read. */
  refused: string | undefined;
}) {
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
        <Table
          key={key}
          caption={heading}
          aria-label={`Runs of ${name}, ${heading.toLowerCase()}`}
          data-testid={`skill-totals-${key}`}
        >
          <TableHeader>
            <TableColumn isRowHeader>{column}</TableColumn>
            <TableColumn>Runs</TableColumn>
            <TableColumn align="end">Tool calls</TableColumn>
            <TableColumn align="end">Errors</TableColumn>
            <TableColumn align="end">Refused</TableColumn>
            <TableColumn align="end">Asks</TableColumn>
            <TableColumn align="end">Tokens</TableColumn>
            <TableColumn align="end">Time</TableColumn>
          </TableHeader>
          <TableBody>
            {stats[key].map((row: StatsRow) => {
              const id = `${row.group.kind}:${row.group.kind === "named" ? row.group.name : ""}`;
              return (
                <TableRow key={id} id={id} textValue={groupSaid(row.group, none)}>
                  <TableCell>{groupSaid(row.group, none)}</TableCell>
                  <TableCell>
                    {counted(row.runs, "run", "runs")}
                    {row.ended < row.runs ? `, ${row.ended} ended` : ""}
                  </TableCell>
                  <TableCell align="end">{row.tool_calls}</TableCell>
                  <TableCell align="end">{row.tool_errors}</TableCell>
                  <TableCell align="end">{row.refusals}</TableCell>
                  <TableCell align="end">{row.asks}</TableCell>
                  <TableCell align="end">{row.tokens ?? "Not reported"}</TableCell>
                  <TableCell align="end">{timeSaid(row.ms)}</TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      ))}
    </section>
  );
}

/** What a panel is held by: `runs`, or `<extension>/<panel>` for one an extension adds. */
export function keyOf(key: SkillPanelKey): string {
  return key.kind === "extension" ? `${key.extension}/${key.panel}` : key.kind;
}

/** **One panel on a skill's page** (ADR 0101 §6): its heading, who adds it, and its blocks or why
 *  it has none. An extension's panel is handed no catalogue: its blocks run no verb of purlis's. */
export function SkillPanelSection({
  panel,
  name,
  current,
  offerFor,
  onPress,
  rowAction,
}: {
  panel: SkillPanel;
  name: string;
  current?: string;
  offerFor?: (id: string) => Offer | undefined;
  onPress?: (offer: Offer) => void;
  rowAction?: RowOpens;
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
          rowAction={rowAction}
        />
      )}
    </section>
  );
}
