// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { LoaderCircle, Sparkles } from "lucide-react";
import { commands, type Studio, type StudioSkill } from "./bindings";
import { EmptyState } from "./EmptyState";
import { Notice } from "./Notice";
import { Select } from "./primitives/Select";
import {
  AT_THE_ROOT,
  inWorkspace,
  scopeChat,
  scopeKey,
  scopeOfKey,
  skillStudioTitle,
  skillStudioView,
  whoseChats,
  type StudioScope,
} from "./skillStudio";
import { skillView } from "./skillRuns";
import { PersonaSkillAdd, PersonaSkillRemove } from "./personaSkills";
import type { ViewRef } from "./tabs";

/**
 * **The Skill studio's tab** (SK-2, #2765; SK-4, #2767): one chat's skill set, at the project's
 * root or in one workspace. Every skill the chat gets, with its level, and every one it does not
 * get with the one line that says why: not granted, turned off, hidden by a narrower copy (both
 * copies are listed), or not a skill. A local skill offers Share. Making and editing skills come
 * with #2770.
 */
export function SkillStudioTab({
  plane,
  scope,
  changed,
  onOpenView,
}: {
  plane: string;
  /** Whose chats it shows: a chat at the project's root, or in one workspace. */
  scope: StudioScope;
  changed: number;
  /** Opens the studio on another workspace's chats, or a skill's own tab with its runs
   *  (SK-3), each in its own tab. */
  onOpenView?: (view: ViewRef, title: string) => void;
}) {
  const [studio, setStudio] = useState<Studio>();
  const [refused, setRefused] = useState<string>();
  const [shareRefused, setShareRefused] = useState<string>();
  const [shared, setShared] = useState(0);
  const { workspace, persona } = whoseChats(scope);
  const changedHere = () => setShared((n) => n + 1);
  useEffect(() => {
    let live = true;
    void commands.skillStudio(plane, workspace, persona).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") {
        setStudio(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, workspace, persona, changed, shared]);

  if (refused !== undefined)
    return (
      <EmptyState
        headline="purlis could not read this project's skills"
        body={refused}
        testid="skill-studio-refused"
      />
    );
  if (studio === undefined)
    return (
      <p className="pending" aria-busy="true">
        <LoaderCircle className="node-icon spinning" />
        Reading the project's skills…
      </p>
    );
  if (!studio.on)
    return (
      <EmptyState
        headline="The Skill studio is not on yet"
        body="Turn it on in Settings › You › Previews to try it while it is being built."
        mark={Sparkles}
        testid="skill-studio-off"
      />
    );

  const share = (skill: StudioSkill) => {
    void commands.skillShare(plane, workspace, skill.name).then((answer) => {
      if (answer.status === "error") setShareRefused(answer.error);
      else {
        setShareRefused(undefined);
        setShared((n) => n + 1);
      }
    });
  };
  const picker = (
    <div className="vault-tools">
      <Select
        className="search-scope"
        aria-label="Whose chats"
        value={scopeKey(scope)}
        options={[
          AT_THE_ROOT,
          ...studio.workspaces.map(inWorkspace),
          ...(persona === null ? [] : [scope]),
        ].map((one) => ({ value: scopeKey(one), label: scopeChat(one) }))}
        onValueChange={(to) => {
          const next = scopeOfKey(to);
          onOpenView?.(skillStudioView(next), skillStudioTitle(next));
        }}
      />
      {persona !== null && (
        /* The persona's own skills list (#2768): every skill choice is the studio's. */
        <PersonaSkillAdd
          plane={plane}
          persona={persona}
          known={studio.skills.filter((skill) => !skill.named).map((skill) => skill.written)}
          onChanged={changedHere}
        />
      )}
    </div>
  );
  if (studio.skills.length === 0)
    return (
      <div className="skill-studio">
        {picker}
        <EmptyState
          headline="This project has no skills yet"
          body="The skills this project's chats get are listed here, with any that are left out and why."
          mark={Sparkles}
          testid="skill-studio-empty"
        />
      </div>
    );
  // The first row of a name is the copy the chat gets, or its only copy; each row after it is a
  // copy that one hides.
  const seen = new Map<string, number>();
  const testid = (name: string) => {
    const before = seen.get(name) ?? 0;
    seen.set(name, before + 1);
    return before === 0 ? `skill-${name}` : `skill-${name}-hidden-${before}`;
  };
  return (
    <div className="skill-studio" data-testid="skill-studio">
      {picker}
      <p className="honest">
        {`Every skill ${lowerFirst(scopeChat(scope))} gets, and why any other is left out.`}
        {persona !== null && " A persona brings its skills into every workspace."}
      </p>
      {shareRefused !== undefined && (
        <Notice
          cause="skill-share-refused"
          tone="trouble"
          label="The skill was not shared"
          onDismiss={() => setShareRefused(undefined)}
        >
          {shareRefused}
        </Notice>
      )}
      <table className="ui-table" aria-label="Skills">
        <thead>
          <tr>
            <th scope="col">Skill</th>
            <th scope="col">Level</th>
            <th scope="col">Granted to</th>
            <th scope="col">In a chat</th>
            <th scope="col">What it does</th>
          </tr>
        </thead>
        <tbody>
          {studio.skills.map((skill) => (
            <tr key={`${skill.level}\u0000${skill.folder}`} data-testid={testid(skill.name)}>
              <td>
                {onOpenView === undefined ? (
                  <code>{skill.name}</code>
                ) : (
                  <button
                    type="button"
                    className="ui-button ui-button-ghost ui-button-sm"
                    aria-label={`Open ${skill.name}`}
                    onClick={() => onOpenView(skillView(skill.name), skill.name)}
                  >
                    <code>{skill.name}</code>
                  </button>
                )}
              </td>
              <td>
                {skill.label}
                {skill.local && (
                  <>
                    {" "}
                    <button
                      type="button"
                      className="ui-button"
                      onClick={() => share(skill)}
                      title="Give it to the team: the next save commits it with the project"
                    >
                      Share
                    </button>
                  </>
                )}
              </td>
              <td>{skill.grant ?? ""}</td>
              <td>
                {skill.on ? (skill.unchecked ?? "On") : `Left out: ${skill.left_out ?? ""}`}
                {persona !== null && skill.named && (
                  <>
                    {" "}
                    <PersonaSkillRemove
                      plane={plane}
                      persona={persona}
                      written={skill.written}
                      onChanged={changedHere}
                    />
                  </>
                )}
              </td>
              <td>{skill.description}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** `text` with its first letter lower-cased, to go inside a sentence. */
function lowerFirst(text: string): string {
  return text.charAt(0).toLowerCase() + text.slice(1);
}
