// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import {
  commands,
  type HarnessKind,
  type Owner,
  type StudioClash,
  type StudioHarness,
  type StudioPlugin,
  type StudioSkill,
  type StudioSwitched,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { SkillCompatBadges } from "./SkillCompatBadges";
import { Button } from "./ui/button/Button";

/** Whose a skill from outside the project is, as a row says it. */
export function ownerShown(owner: Owner | null): string {
  if (owner === null) return "";
  if (owner.kind === "you") return "You";
  if (owner.kind === "repo") return `Repo ${owner.name}`;
  return owner.name;
}

/** Who a harness or repo skill is from, as its row says it. */
export function fromWhom(skill: StudioSkill): string {
  return ownerShown(skill.owner);
}

/** What the project has a plugin at, in the chats the studio shows, as its row says it. */
export function pluginShown(plugin: StudioPlugin, title: string): string {
  if (plugin.pinned !== null) return plugin.pinned;
  if (plugin.state === "on") return "On";
  if (plugin.state === "off") return "Off";
  return `${title} decides`;
}

/**
 * **The project's skill that clashes with one a program or repo brings** (SK-6, #2769; ADR 0099
 * §3): both load, so the row says so and names the other copy.
 */
export function clashSaid(clash: StudioClash): string {
  const owner = clash.owner;
  const whose =
    owner.kind === "repo"
      ? `the repo ${owner.name}`
      : owner.kind === "you"
        ? `your own ${clash.harness} skills`
        : `${owner.name} in ${clash.harness}`;
  if (clash.outcome === "repo-wins")
    return `${whose} has a skill of this name too, and ${clash.harness} chats there get the repo's copy alone`;
  if (clash.outcome === "either-one")
    return `${whose} has a skill of this name too, and ${clash.harness} chats there get both under one name and may use either: rename that one`;
  return `${whose} has a skill of this name too, and a chat there gets both`;
}

/**
 * **Each harness's own, in the Skill studio** (SK-6, #2769): the plugins and skills the program
 * finds itself, in the person's own folders, its plugins, and the workspace's repos. purlis lists
 * them and never changes them. Each has a switch where the program can leave it out of one chat,
 * and says plainly where it cannot. A switch writes this machine's own file at the project's root
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
  const turn = (harness: HarnessKind, what: StudioSwitched, id: string, on: boolean) => {
    const doing = `${harness}\u0000${what}\u0000${id}`;
    setBusy(doing);
    void commands
      .skillStudioSwitch(plane, workspace, harness, what, id, on)
      .then((answer) => {
        if (answer.status === "error") setRefused(answer.error);
        else {
          setRefused(undefined);
          onSwitched();
        }
      })
      .finally(() => setBusy(undefined));
  };
  // `back` is what the button that undoes an off says: "Turn on", or, where the program decides
  // whether one is on (Codex's plugins), "Leave to <program>".
  const switchFor = (
    harness: HarnessKind,
    what: StudioSwitched,
    id: string,
    on: boolean,
    back = "Turn on",
  ) => (
    <Button size="sm" isDisabled={busy !== undefined} onPress={() => turn(harness, what, id, !on)}>
      {on ? "Turn off" : back}
    </Button>
  );
  return (
    <section className="skill-studio-harnesses" aria-label="Skills and plugins from each program">
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
            {`The skills and plugins ${harness.title} finds itself. purlis lists them and never changes them; a switch here reaches the chats started after it.`}
          </p>
          {harness.trouble !== null && <p className="honest">{harness.trouble}</p>}
          {harness.plugins.length > 0 && (
            <table className="ui-table" aria-label={`${harness.title} plugins`}>
              <thead>
                <tr>
                  <th scope="col">Plugin</th>
                  <th scope="col">Installed</th>
                  <th scope="col">In a chat</th>
                </tr>
              </thead>
              <tbody>
                {harness.plugins.map((plugin) => (
                  <tr key={plugin.id} data-testid={`plugin-${harness.harness}-${plugin.id}`}>
                    <td>
                      <code>{plugin.name}</code>
                    </td>
                    <td>{plugin.origin}</td>
                    <td>
                      {pluginShown(plugin, harness.title)}
                      {plugin.pinned === null &&
                        harness.unsupported === null &&
                        plugin.installed && (
                          <>
                            {" "}
                            {switchFor(
                              harness.harness,
                              "plugin",
                              plugin.id,
                              plugin.state !== "off",
                              harness.only_off === null ? "Turn on" : `Leave to ${harness.title}`,
                            )}
                          </>
                        )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {harness.unsupported !== null && <p className="honest">{harness.unsupported}.</p>}
          {harness.only_off !== null && (
            <p className="honest">{`${harness.title}'s own config decides whether a plugin is on; purlis can only turn one off for these chats.`}</p>
          )}
          {harness.skills.length === 0 ? (
            <p className="honest">{`${harness.title} has no skills of its own here.`}</p>
          ) : (
            <table className="ui-table" aria-label={`${harness.title} skills`}>
              <thead>
                <tr>
                  <th scope="col">Skill</th>
                  <th scope="col">From</th>
                  <th scope="col">In a chat</th>
                  <th scope="col">What it does</th>
                  <th scope="col">Works on</th>
                </tr>
              </thead>
              <tbody>
                {harness.skills.map((skill) => (
                  <tr
                    key={`${skill.level}\u0000${skill.folder}`}
                    data-testid={`found-${harness.harness}-${skill.name}`}
                  >
                    <td>
                      <code>{skill.name}</code>
                    </td>
                    <td>{fromWhom(skill)}</td>
                    <td>
                      {skill.on ? "On" : `Left out: ${skill.left_out ?? ""}`}
                      {skill.switch?.kind === "alone" && (
                        <> {switchFor(harness.harness, "skill", skill.name, skill.on)}</>
                      )}
                      {skill.switch?.kind === "not" && (
                        <span className="honest">{` ${skill.switch.why}.`}</span>
                      )}
                    </td>
                    <td>{skill.description}</td>
                    <td>
                      <SkillCompatBadges
                        name={skill.name}
                        compat={skill.compat}
                        lint={skill.lint}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {harness.repo_left_out.length > 0 && (
            <>
              <p className="honest">
                {`${harness.title} chats in a repo here start without ${counted(harness.repo_left_out.length, "skill", "skills")} of this project's:`}
              </p>
              <table
                className="ui-table"
                aria-label={`What a repo leaves out of ${harness.title} chats`}
              >
                <thead>
                  <tr>
                    <th scope="col">Skill</th>
                    <th scope="col">Level</th>
                    <th scope="col">Why</th>
                  </tr>
                </thead>
                <tbody>
                  {harness.repo_left_out.map((skill) => (
                    <tr
                      key={`${skill.level}\u0000${skill.folder}\u0000${skill.left_out ?? ""}`}
                      data-testid={`repo-left-out-${harness.harness}-${skill.name}`}
                    >
                      <td>
                        <code>{skill.name}</code>
                      </td>
                      <td>{skill.label}</td>
                      <td>{`Left out: ${skill.left_out ?? ""}`}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
        </div>
      ))}
    </section>
  );
}
