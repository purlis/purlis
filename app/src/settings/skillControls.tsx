// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Settings › Skill sources** (#6031, `prototype/skill-policy` A): the project's
 * `[skill_policy]` read only, one row per key, each saying where it is set, and the skill acts
 * this machine recorded as a table under it. Kept in the shared file, which the team changes by a
 * reviewed change: nothing here writes it.
 */
import { useEffect, useState } from "react";
import {
  commands,
  type PlaneId,
  type SkillAct,
  type SkillActs,
  type SkillPolicy,
  type SourceOrigin,
} from "../bindings";
import { SETTINGS, usePlaneChanged } from "../planeChanged";
import { lineOf, useSkillPolicyRead } from "../skillPolicy";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "../ui/table/Table";
import type { RowIds } from "./components";
import type { LiveSetting } from "./groups";

/** No source asked about: Settings shows the table, not what it does to each source. */
const NO_SOURCES: readonly SourceOrigin[] = [];

/** The project's controls as Settings reads them, again as its settings change: one ask for every
 *  row drawn at once. */
function usePolicy(plane: PlaneId): SkillPolicy | string | undefined {
  return useSkillPolicyRead(plane, NO_SOURCES, usePlaneChanged([plane], SETTINGS));
}

/** What the controls row says while the table is not set: none, or why it cannot be read. */
export function controlsSaid(policy: SkillPolicy | string | undefined): string {
  if (policy === undefined) return "Reading…";
  if (typeof policy === "string") return policy;
  switch (policy.state.kind) {
    case "none":
      return `None: ${policy.file} holds no [skill_policy], so a skill installs from any source and no install, update or removal is audited.`;
    case "refused":
      return `${lineOf(policy.state.why)}.`;
    case "set":
      return policy.state.said;
  }
}

/** One key of the table, read only, set in the shared file: shown while the table is set. */
function keyRow(
  plane: PlaneId,
  id: string,
  label: string,
  help: string,
  value: (state: Extract<SkillPolicy["state"], { kind: "set" }>) => string,
): LiveSetting {
  return {
    id: `project.skill-sources.${id}`,
    label,
    help,
    writes: "nothing",
    useShown: function useSet() {
      const policy = usePolicy(plane);
      return typeof policy === "object" && policy.state.kind === "set";
    },
    useControl: function useKey() {
      const policy = usePolicy(plane);
      const set = typeof policy === "object" && policy.state.kind === "set" ? policy : undefined;
      return {
        ...(set === undefined
          ? {}
          : {
              heldBy: {
                label: "Team",
                says: `Set in ${set.file}, which your team sees. Change it there, by a reviewed change.`,
              },
            }),
        control: (ids: RowIds) => (
          <code id={ids.id} aria-describedby={ids.describedBy} data-testid={`skill-policy-${id}`}>
            {set?.state.kind === "set" ? value(set.state) : ""}
          </code>
        ),
      };
    },
  };
}

/** The rows of the controls: what they do while they are not set, then each key. */
export function skillControlsRows(plane: PlaneId): LiveSetting[] {
  return [
    {
      id: "project.skill-sources.controls",
      label: "Controls",
      help: "What this project's [skill_policy] holds the skills it installs and updates to.",
      writes: "nothing",
      useShown: function useNotSet() {
        const policy = usePolicy(plane);
        return !(typeof policy === "object" && policy.state.kind === "set");
      },
      useControl: function useControls() {
        const policy = usePolicy(plane);
        return {
          control: (ids: RowIds) => (
            <p
              id={ids.id}
              className="ui-setting-help"
              aria-describedby={ids.describedBy}
              data-testid="skill-policy-controls"
            >
              {controlsSaid(policy)}
            </p>
          ),
        };
      },
    },
    keyRow(
      plane,
      "allow",
      "Allow",
      "Only these sources install. Absent, every source not blocked does.",
      (state) =>
        state.allow === null ? "Not set: every source not blocked" : state.allow.join(" · "),
    ),
    keyRow(plane, "block", "Block", "Never installed. A block wins over an allow.", (state) =>
      state.block.length === 0 ? "Nothing" : state.block.join(" · "),
    ),
    keyRow(
      plane,
      "min-age",
      "Minimum age",
      "A commit installs or updates only once it is this old. 0 turns it off.",
      (state) =>
        state.min_age_days === 0
          ? "Off"
          : `${state.min_age_days} ${state.min_age_days === 1 ? "day" : "days"}`,
    ),
    keyRow(
      plane,
      "exempt",
      "Exempt commits",
      "Each installs younger, by a reviewed change.",
      (state) => (state.exempt.length === 0 ? "None" : state.exempt.join(" · ")),
    ),
  ];
}

/** When an act was recorded, as its row says it: 13 Nov, 14:02; a dash where the record does
 *  not say. */
export function actAt(at: number | null): string {
  if (at === null) return "—";
  return new Date(at * 1000).toLocaleString("en-GB", {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
}

const ACTS: Record<SkillAct["act"], string> = {
  install: "Install",
  update: "Update",
  remove: "Remove",
};

const BY: Record<SkillAct["by"], string> = {
  window: "You, in the Skill studio",
  terminal: "You, at a terminal",
};

/** **The recorded skill acts**, newest first, as a table. */
export function SkillActsTable({ acts, ids }: { acts: SkillActs; ids: RowIds }) {
  if (acts.acts.length === 0)
    return (
      <p id={ids.id} className="ui-setting-help" data-testid="skill-acts-none">
        No skill install, update or removal is recorded on this machine yet.
      </p>
    );
  // In a group of its own, as Accounts' table is (#6301): the row's control slot sizes to it, so
  // the table, which stacks by its frame's width, has the slot's width to read.
  return (
    <div
      id={ids.id}
      role="group"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      <Table
        className="ui-record-table"
        aria-label="Recorded skill acts"
        aria-describedby={ids.describedBy}
        stacks
        data-testid="skill-acts"
      >
        <TableHeader>
          <TableColumn id="when" isRowHeader>
            When
          </TableColumn>
          <TableColumn id="act">Act</TableColumn>
          <TableColumn id="skills">Skills</TableColumn>
          <TableColumn id="source">Source</TableColumn>
          <TableColumn id="level">Level</TableColumn>
          <TableColumn id="by">By</TableColumn>
        </TableHeader>
        <TableBody>
          {acts.acts.map((one, at) => (
            <TableRow
              key={`${one.at}\u0000${at}`}
              id={`${one.at}:${at}`}
              textValue={`${ACTS[one.act]} ${one.skills.join(", ")}`}
            >
              <TableCell>{actAt(one.at)}</TableCell>
              <TableCell heading="Act">{ACTS[one.act]}</TableCell>
              <TableCell heading="Skills">{one.skills.join(", ")}</TableCell>
              <TableCell heading="Source">
                <span className="ui-record-where">
                  {one.source} @ {one.pin.slice(0, 7)}
                </span>
              </TableCell>
              <TableCell heading="Level">{one.level}</TableCell>
              <TableCell heading="By">{BY[one.by]}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      {acts.more && (
        <p className="ui-setting-help">Older acts are recorded too; the newest 50 are listed.</p>
      )}
    </div>
  );
}

/** The row of the recorded acts. */
export function skillActsRow(plane: PlaneId): LiveSetting {
  return {
    id: "project.skill-acts.list",
    label: "Installs, updates and removals",
    help: "As this machine's audit recorded them, newest first. The record does not say which project each was made in.",
    writes: "nothing",
    useControl: function useActs() {
      const [acts, setActs] = useState<SkillActs | string>();
      const changed = usePlaneChanged([plane], SETTINGS);
      useEffect(() => {
        let live = true;
        void commands.skillActs(plane).then(
          (answer) => {
            if (live) setActs(answer.status === "ok" ? answer.data : answer.error);
          },
          (why: unknown) => {
            if (live) setActs(`purlis could not read the recorded skill acts: ${String(why)}`);
          },
        );
        return () => {
          live = false;
        };
        // The row is drawn under the same key for every project: read again on another.
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, [plane, changed]);
      return {
        grouped: true,
        control: (ids: RowIds) =>
          acts === undefined || typeof acts === "string" ? (
            <p id={ids.id} className="ui-setting-help">
              {acts ?? "Reading…"}
            </p>
          ) : (
            <SkillActsTable acts={acts} ids={ids} />
          ),
      };
    },
  };
}
