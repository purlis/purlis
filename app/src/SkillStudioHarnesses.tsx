// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import {
  commands,
  type HarnessKind,
  type Owner,
  type StudioHarness,
  type StudioSkill,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { SkillCompatBadges } from "./SkillCompatBadges";
import { Button } from "./ui/button/Button";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";

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
 * **Each harness's own, in the Skill studio** (SK-6, #2769): the skills the program finds itself,
 * in the person's own folders, its plugins, and the workspace's repos. purlis lists them and never
 * changes them. Each has a switch where the program can leave it out of one chat, and says plainly
 * where it cannot: a plugin's skill is shown read-only, naming its plugin, since whether a plugin
 * is on is the program's own to say (#3467). A switch writes this machine's own file at the project's root
 * and the workspace's settings in a workspace, and reaches chats started after it. Under each, the
 * project's skills a repo leaves out of its chats there, with why (SK-53): never a silent drop.
 */
export function SkillStudioHarnesses({
  plane,
  workspace,
  harnesses,
  onSwitched,
}: {
  plane: string;
  workspace: string | null;
  harnesses: readonly StudioHarness[];
  onSwitched: () => void;
}) {
  const [refused, setRefused] = useState<string>();
  const [busy, setBusy] = useState<string>();
  const turn = (harness: HarnessKind, id: string, on: boolean) => {
    const doing = `${harness}\u0000${id}`;
    setBusy(doing);
    void commands
      .skillStudioSwitch(plane, workspace, harness, id, on)
      .then((answer) => {
        if (answer.status === "error") setRefused(answer.error);
        else {
          setRefused(undefined);
          onSwitched();
        }
      })
      .finally(() => setBusy(undefined));
  };
  const switchFor = (harness: HarnessKind, id: string, on: boolean) => (
    <Button size="sm" isDisabled={busy !== undefined} onPress={() => turn(harness, id, !on)}>
      {on ? "Turn off" : "Turn on"}
    </Button>
  );
  return (
    <section className="skill-studio-harnesses" aria-label="Skills from each program">
      {refused !== undefined && (
        <Notice
          cause="skill-studio-switch-refused"
          tone="trouble"
          label="Nothing was changed"
          onDismiss={() => setRefused(undefined)}
        >
          {refused}
        </Notice>
      )}
      {harnesses.map((harness) => (
        <div key={harness.harness} data-testid={`skill-studio-${harness.harness}`}>
          <h3>{harness.title}</h3>
          <p className="honest">
            {`The skills ${harness.title} finds itself. purlis lists them and never changes them; a switch here reaches the chats started after it. A plugin's skills come with the plugin, which ${harness.title} turns on or off itself.`}
          </p>
          {harness.skills.length === 0 ? (
            <p className="honest">{`${harness.title} has no skills of its own here.`}</p>
          ) : (
            <Table aria-label={`${harness.title} skills`}>
              <TableHeader>
                <TableColumn isRowHeader>Skill</TableColumn>
                <TableColumn>From</TableColumn>
                <TableColumn>In a chat</TableColumn>
                <TableColumn>What it does</TableColumn>
                <TableColumn>Works on</TableColumn>
              </TableHeader>
              <TableBody>
                {harness.skills.map((skill) => (
                  <TableRow
                    key={`${skill.level}\u0000${skill.folder}`}
                    id={`${skill.level}\u0000${skill.folder}`}
                    textValue={skill.name}
                    data-testid={`found-${harness.harness}-${skill.name}`}
                  >
                    <TableCell>
                      <code>{skill.name}</code>
                    </TableCell>
                    <TableCell>{fromWhom(skill)}</TableCell>
                    <TableCell>
                      {skill.on ? "On" : `Left out: ${skill.left_out ?? ""}`}
                      {skill.switch?.kind === "alone" && (
                        <> {switchFor(harness.harness, skill.name, skill.on)}</>
                      )}
                      {skill.switch?.kind === "not" && (
                        <span className="honest">{` ${skill.switch.why}.`}</span>
                      )}
                    </TableCell>
                    <TableCell>{skill.description}</TableCell>
                    <TableCell>
                      <SkillCompatBadges
                        name={skill.name}
                        compat={skill.compat}
                        lint={skill.lint}
                      />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {harness.repo_left_out.length > 0 && (
            <>
              <p className="honest">
                {`${harness.title} chats in a repo here start without ${counted(harness.repo_left_out.length, "skill", "skills")} of this project's:`}
              </p>
              <Table aria-label={`What a repo leaves out of ${harness.title} chats`}>
                <TableHeader>
                  <TableColumn isRowHeader>Skill</TableColumn>
                  <TableColumn>Level</TableColumn>
                  <TableColumn>Why</TableColumn>
                </TableHeader>
                <TableBody>
                  {harness.repo_left_out.map((skill) => {
                    const id = `${skill.level}\u0000${skill.folder}\u0000${skill.left_out ?? ""}`;
                    return (
                      <TableRow
                        key={id}
                        id={id}
                        textValue={skill.name}
                        data-testid={`repo-left-out-${harness.harness}-${skill.name}`}
                      >
                        <TableCell>
                          <code>{skill.name}</code>
                        </TableCell>
                        <TableCell>{skill.label}</TableCell>
                        <TableCell>{`Left out: ${skill.left_out ?? ""}`}</TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </>
          )}
        </div>
      ))}
    </section>
  );
}
