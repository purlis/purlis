// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { LoaderCircle, Sparkles } from "lucide-react";
import { commands, type Studio } from "./bindings";
import { EmptyState } from "./EmptyState";

/**
 * **The Skill studio's tab** (SK-2, #2765): every built-in and project skill, with its level,
 * and why each one a chat at the plane root does not get is left out. Read only: making,
 * editing and granting skills come with #2770 and #2767.
 */
export function SkillStudioTab({ plane, changed }: { plane: string; changed: number }) {
  const [studio, setStudio] = useState<Studio>();
  const [refused, setRefused] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands.skillStudio(plane).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") {
        setStudio(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, changed]);

  if (refused !== undefined)
    return (
      <EmptyState
        headline="purlis could not read this project's skills"
        body={refused}
        role="status"
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
        body="It is being built behind a flag, and opens here once it is turned on on this machine."
        mark={Sparkles}
        testid="skill-studio-off"
      />
    );
  if (studio.skills.length === 0)
    return (
      <EmptyState
        headline="This project has no skills yet"
        body={
          <>
            A project skill is a folder in <code>skills/</code> at the project's root, holding its{" "}
            <code>SKILL.md</code>.
          </>
        }
        mark={Sparkles}
        testid="skill-studio-empty"
      />
    );
  return (
    <div className="skill-studio" data-testid="skill-studio">
      <p className="honest">
        Every skill a chat at the project's root gets, and why any other is left out.
      </p>
      <table className="ui-table" aria-label="Skills">
        <thead>
          <tr>
            <th scope="col">Skill</th>
            <th scope="col">Level</th>
            <th scope="col">In a chat</th>
            <th scope="col">What it does</th>
          </tr>
        </thead>
        <tbody>
          {studio.skills.map((skill) => (
            <tr key={`${skill.level}\u0000${skill.folder}`} data-testid={`skill-${skill.name}`}>
              <td>
                <code>{skill.name}</code>
              </td>
              <td>{skill.level}</td>
              <td>{skill.on ? "On" : `Left out: ${skill.left_out ?? ""}`}</td>
              <td>{skill.description}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
