// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import {
  commands,
  type HarnessKind,
  type Owner,
  type StudioHarness,
  type StudioNotSupported,
  type StudioSkill,
} from "./bindings";
import { Notice } from "./Notice";
import { SkillCompatBadges } from "./SkillCompatBadges";
import { Badge } from "./ui/badge/Badge";
import { Fact, Facts } from "./ui/card/Facts";
import { Switch } from "./ui/choice/Switch";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { List, ListRow, ListRowSection, type Selection } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ViewPageHeader } from "./ui/panel/ViewFrame";
import { harnessOfId, harnessSkillId, skillOfRowId, skillRowId } from "./skillStudio";

/** Whose a skill from outside the project is, as a row says it. */
export function ownerShown(owner: Owner | null): string {
  if (owner === null) return "";
  if (owner.kind === "you") return "You";
  if (owner.kind === "repo") return `Repo ${owner.name}`;
  return `Plugin ${owner.name}`;
}

/** Who a harness or repo skill is from, as its row says it. */
export function fromWhom(skill: StudioSkill): string {
  return ownerShown(skill.owner);
}

/**
 * **One row of the Harness skills page** (#3692): a program's own skill; a project skill a repo
 * leaves out of that program's chats (SK-53); the line of a program with none of its own; or a
 * declared program skills are not supported on (SK-30).
 */
export type HarnessRow =
  | { kind: "own"; id: string; harness: HarnessKind; title: string; skill: StudioSkill }
  | { kind: "left-out"; id: string; harness: HarnessKind; title: string; skill: StudioSkill }
  | { kind: "none"; id: string; harness: HarnessKind; title: string }
  | { kind: "not-supported"; id: string; one: StudioNotSupported };

/** **One group of the page**, under its program's name. */
export interface HarnessGroup {
  key: string;
  title: string;
  /** How many skills of its own the program has. */
  count?: number;
  rows: HarnessRow[];
}

/**
 * **The page's groups** (#3692): each program with its own skills, then the project's skills a
 * repo leaves out of its chats; a program with none of its own says so in one row; the declared
 * programs skills are not supported on come last.
 */
export function harnessGroups(
  harnesses: readonly StudioHarness[],
  notSupported: readonly StudioNotSupported[],
): HarnessGroup[] {
  const groups: HarnessGroup[] = harnesses.map((harness) => {
    const own: HarnessRow[] = harness.skills.map((skill) => ({
      kind: "own",
      id: harnessSkillId(harness.harness, skill),
      harness: harness.harness,
      title: harness.title,
      skill,
    }));
    const none: HarnessRow[] =
      own.length === 0
        ? [
            {
              kind: "none",
              id: `${harness.harness}/none`,
              harness: harness.harness,
              title: harness.title,
            },
          ]
        : [];
    const leftOut: HarnessRow[] = harness.repo_left_out.map((skill) => ({
      kind: "left-out",
      id: `${harness.harness}/left-out/${skillRowId(skill)}\u0000${skill.left_out ?? ""}`,
      harness: harness.harness,
      title: harness.title,
      skill,
    }));
    return {
      key: harness.harness,
      title: harness.title,
      count: harness.skills.length,
      rows: [...own, ...none, ...leftOut],
    };
  });
  if (notSupported.length > 0)
    groups.push({
      key: "not-supported",
      title: "Programs that take no skills",
      rows: notSupported.map((one) => ({
        kind: "not-supported",
        id: `not-supported/${one.name}`,
        one,
      })),
    });
  return groups;
}

/**
 * **The row a place names** (#3692): by its id, or, for a place that names a program's skill only
 * by its name (`harnessSkillNamed`), that program's first row of that name.
 */
export function harnessRowAt(
  groups: readonly HarnessGroup[],
  id: string | undefined,
): HarnessRow | undefined {
  if (id === undefined) return undefined;
  const rows = groups.flatMap((group) => group.rows);
  const exact = rows.find((row) => row.id === id);
  if (exact !== undefined) return exact;
  const named = harnessOfId(id);
  if (named === undefined) return undefined;
  const name = skillOfRowId(named.row);
  return rows.find(
    (row) => row.kind === "own" && row.harness === named.harness && row.skill.name === name,
  );
}

/** What a program's own skill's row says under its name: what it does, or why it has no
 *  switch. */
function lineOf(skill: StudioSkill): string {
  if (skill.switch?.kind === "not") return `${capitalised(skill.switch.why)}.`;
  return skill.description === "" ? "It says nothing of what it does." : skill.description;
}

/**
 * **The Harness skills page's list** (#3692, S9, `prototype/studio-secondary-pages` variant A):
 * each program's own skills under its name, each with whose it is and one mark. The row chosen
 * opens beside the list ({@link HarnessItem}), with its switch.
 */
export function HarnessList({
  groups,
  chosen,
  onChoose,
}: {
  groups: readonly HarnessGroup[];
  chosen: HarnessRow | undefined;
  onChoose: (id: string) => void;
}) {
  return (
    <List
      aria-label="Each program's own skills"
      className="skill-library"
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={chosen === undefined ? new Set() : new Set([chosen.id])}
      onSelectionChange={(keys: Selection) => {
        const [first] = keys === "all" ? [] : [...keys];
        if (first !== undefined) onChoose(String(first));
      }}
    >
      {groups.map((group) => (
        <ListRowSection
          key={group.key}
          id={`group-${group.key}`}
          title={group.title}
          count={group.count}
          aria-label={group.title}
        >
          {group.rows.map((row) => (
            <HarnessListRow key={row.id} row={row} />
          ))}
        </ListRowSection>
      ))}
    </List>
  );
}

function HarnessListRow({ row }: { row: HarnessRow }) {
  if (row.kind === "none")
    return (
      <ListRow id={row.id} textValue={row.title} data-testid={`harness-none-${row.harness}`}>
        <RowLabel>{`${row.title} has no skills of its own here.`}</RowLabel>
      </ListRow>
    );
  if (row.kind === "not-supported")
    return (
      <ListRow
        id={row.id}
        textValue={row.one.title}
        data-testid={`skill-studio-not-supported-${row.one.name}`}
      >
        <RowLabel>{row.one.title}</RowLabel>
        <RowTrail>
          <Badge>No skills</Badge>
        </RowTrail>
        <RowBelow clamp={2}>{row.one.said}</RowBelow>
      </ListRow>
    );
  const { skill } = row;
  if (row.kind === "left-out")
    return (
      <ListRow
        id={row.id}
        textValue={skill.name}
        data-testid={`repo-left-out-${row.harness}-${skill.name}`}
      >
        <RowLabel>{skill.name}</RowLabel>
        <RowTrail>
          <Badge>{skill.label}</Badge>
          <Badge variant="waiting">Left out by a repo</Badge>
        </RowTrail>
        <RowBelow clamp={1}>{skill.left_out ?? ""}</RowBelow>
      </ListRow>
    );
  return (
    <ListRow id={row.id} textValue={skill.name} data-testid={`found-${row.harness}-${skill.name}`}>
      <RowLabel>{skill.name}</RowLabel>
      <RowTrail>
        <Badge>{fromWhom(skill)}</Badge>
        {skill.switch?.kind === "not" ? (
          <Badge>Read-only</Badge>
        ) : (
          !skill.on && <Badge variant="waiting">Off here</Badge>
        )}
      </RowTrail>
      <RowBelow clamp={1}>{lineOf(skill)}</RowBelow>
    </ListRow>
  );
}

/**
 * **The row chosen on Harness skills, beside the list** (#3692): its facts (the program, whose it
 * is, how a chat takes it, which programs it works on) and, where the program can leave it out of
 * a chat on its own, its switch for these chats. A plugin's skill is read-only, with why: whether
 * a plugin is on is the program's own to say (#3467). A switch writes this machine's own file at
 * the project's root and the workspace's settings in a workspace, and reaches chats started after
 * it.
 */
export function HarnessItem({
  plane,
  workspace,
  row,
  onSwitched,
}: {
  plane: string;
  workspace: string | null;
  row: HarnessRow | undefined;
  onSwitched: () => void;
}) {
  const [refused, setRefused] = useState<string>();
  const [busy, setBusy] = useState(false);
  if (row === undefined)
    return (
      <EmptyState
        headline="Pick a skill"
        body="Where it comes from, how a chat takes it and its switch open here."
        testid="harness-unpicked"
      />
    );
  if (row.kind === "none")
    return (
      <div className="skill-studio-item" data-testid="harness-skill-item">
        <ViewPageHeader name={row.title} description="It has no skills of its own here." />
        <p className="honest">
          {`A skill ${row.title} finds itself, in your own folders, in a plugin or in a repo, is listed here.`}
        </p>
      </div>
    );
  if (row.kind === "not-supported")
    return (
      <div className="skill-studio-item" data-testid="harness-skill-item">
        <ViewPageHeader name={row.one.title} tag={<Badge>No skills</Badge>} />
        <p className="honest">{row.one.said}</p>
      </div>
    );
  const { skill } = row;
  const these = workspace === null ? "this project's chats" : `${workspace}'s chats`;
  if (row.kind === "left-out")
    return (
      <div className="skill-studio-item" data-testid="harness-skill-item">
        <ViewPageHeader
          name={skill.name}
          tag={<Badge>{skill.label}</Badge>}
          mark={<Badge variant="waiting">Left out by a repo</Badge>}
          description={skill.description}
        />
        <div>
          <Facts aria-label={`About ${skill.name}`}>
            <Fact label="Program">{row.title}</Fact>
            <Fact label="Skill level">{skill.label}</Fact>
            <Fact label="In a chat">{`Left out: ${skill.left_out ?? ""}`}</Fact>
          </Facts>
        </div>
      </div>
    );
  const turn = (on: boolean) => {
    setBusy(true);
    void commands
      .skillStudioSwitch(plane, workspace, row.harness, skill.name, on)
      .then((answer) => {
        if (answer.status === "error") setRefused(answer.error);
        else {
          setRefused(undefined);
          onSwitched();
        }
      })
      .catch((err: unknown) => setRefused(String(err)))
      .finally(() => setBusy(false));
  };
  const readOnly = skill.switch?.kind === "not" ? skill.switch.why : undefined;
  const inAChat =
    readOnly !== undefined
      ? `${capitalised(readOnly)}.`
      : skill.on
        ? `On for ${these}`
        : `Left out: ${skill.left_out ?? ""}`;
  return (
    <div className="skill-studio-item" data-testid="harness-skill-item">
      <ViewPageHeader
        name={skill.name}
        tag={<Badge>{fromWhom(skill)}</Badge>}
        mark={
          readOnly !== undefined ? (
            <Badge>Read-only</Badge>
          ) : skill.on ? undefined : (
            <Badge variant="waiting">Off here</Badge>
          )
        }
        description={skill.description}
      />
      {refused !== undefined && (
        <div>
          <Notice
            cause="skill-studio-switch-refused"
            tone="trouble"
            label="Nothing was changed"
            onDismiss={() => setRefused(undefined)}
          >
            {refused}
          </Notice>
        </div>
      )}
      <div>
        <Facts aria-label={`About ${skill.name}`}>
          <Fact label="Program">{row.title}</Fact>
          <Fact label="From">{fromWhom(skill)}</Fact>
          <Fact label="In a chat">{inAChat}</Fact>
          <Fact label="Works on">
            <SkillCompatBadges name={skill.name} compat={skill.compat} lint={skill.lint} />
          </Fact>
        </Facts>
      </div>
      {skill.switch?.kind === "alone" && (
        <div>
          <Switch isSelected={skill.on} isDisabled={busy} onChange={turn}>
            {`On for ${these}`}
          </Switch>
        </div>
      )}
      <p className="honest">
        {`purlis lists ${row.title}'s own skills and never changes them; a switch reaches the chats started after it.`}
      </p>
    </div>
  );
}

/** `text` with its first letter upper-cased, to start a sentence. */
function capitalised(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
