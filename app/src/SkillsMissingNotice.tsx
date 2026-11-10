// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useEffect, useState } from "react";
import { commands, type PlaneId, type SkillsMissing } from "./bindings";
import { counted } from "./dispatches";
import { Notice, NoticeOf } from "./Notice";
import { SHOW_IN_SKILLS, askChatSkills, skillsMissingPutAway } from "./chatSkillsAsk";

/**
 * **The skills a chat started without, on its tab** (SK-5, #2768; SK-8, #2771; spec decision
 * 41): never a silent drop. An installed skill this machine, or the runner the chat started on
 * (SK-5d, #2977), does not have for the chat's harness, a built-in skill purlis there has none
 * of, and another workspace's skill that workspace or the project does not have (SK-5b, #6040),
 * each named as the persona's skills write it; a skill the chat would have loaded that
 * could not be loaded, with why; and one the repo it works in leaves out, with why (SK-53). And
 * a skill of that repo with a name of purlis's own, which the chat's program loads beside
 * purlis's (SK-37). It offers
 *
 * - **Show in Skills** (S8, #3694): the sidebar Skills view on this chat, where what it started
 *   without sits in Needs you with its fix;
 * - **Dismiss**: puts it away for this run of the chat, off the Skills chip on its pane too.
 */
export function SkillsMissingNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const [said, setSaid] = useState<SkillsMissing | null>(null);
  const offScreen = useContext(NoticeOf) !== null;
  useEffect(() => {
    let gone = false;
    void commands
      .chatSkillsMissing(plane, session)
      .then((answer) => {
        if (!gone && answer.status === "ok") setSaid(answer.data);
      })
      .catch(() => {});
    return () => {
      gone = true;
    };
  }, [plane, session]);
  if (said === null) return null;
  const chat = offScreen ? "That chat" : "This chat";
  const putAway = () => {
    setSaid(null);
    skillsMissingPutAway(plane, session);
    void commands.putAwayChatSkillsMissing(plane, session).catch(() => {});
  };
  return (
    <Notice
      cause={`skills-missing:${session}`}
      at="pane"
      tone="trouble"
      label="Skills"
      persona={said.persona}
      fixes={[
        {
          label: SHOW_IN_SKILLS,
          onPress: () => askChatSkills(plane, session),
        },
      ]}
      onDismiss={putAway}
    >
      {missingSays(chat, said)}
    </Notice>
  );
}

/** What the Notice says of `said`, for `chat` ("This chat" or "That chat"). */
export function missingSays(chat: string, said: SkillsMissing): string {
  const lines: string[] = [];
  const persona = said.persona ?? "its persona";
  // Where it was looked for (#2977): this machine, or the runner a chat there started on.
  const where =
    said.runner === null
      ? "this machine"
      : said.runner === ""
        ? "its runner"
        : `runner ${said.runner}`;
  const there = said.runner === null ? "" : ` on ${where}`;
  if (said.missing.length > 0)
    lines.push(
      `${chat} started without ${counted(said.missing.length, "skill", "skills")} ${persona} names, not installed on ${where}: ${said.missing.join(", ")}.`,
    );
  if (said.no_built_in.length > 0)
    lines.push(
      `${persona} names ${counted(said.no_built_in.length, "built-in skill", "built-in skills")} purlis does not have${there}: ${said.no_built_in.join(", ")}.`,
    );
  // Another workspace's skill its persona names (SK-5b, #6040), each with the lint's fix.
  if (said.no_workspace.length > 0)
    lines.push(
      `${persona} names ${counted(said.no_workspace.length, "skill", "skills")} of a workspace the project does not have: ${said.no_workspace.join(", ")}. Fix the workspace's name.`,
    );
  if (said.not_in_its_workspace.length > 0)
    lines.push(
      `${persona} names ${counted(said.not_in_its_workspace.length, "skill", "skills")} the workspace named does not have: ${said.not_in_its_workspace.join(", ")}. Make the skill there, or fix the name.`,
    );
  if (said.broken.length > 0)
    lines.push(
      `${chat} started without ${counted(said.broken.length, "skill that could not be loaded", "skills that could not be loaded")}: ${said.broken
        .map((one) => `${one.name} (${one.label}), ${one.why}`)
        .join("; ")}.`,
    );
  if (said.by_a_repo.length > 0)
    lines.push(
      `${chat} started without ${counted(said.by_a_repo.length, "skill", "skills")} because of the repo it works in: ${said.by_a_repo
        .map((one) => `${one.name} (${one.label}), ${one.why}`)
        .join("; ")}.`,
    );
  if (said.beside_a_built_in.length > 0)
    lines.push(
      `${chat} works in a repo with its own copy of ${counted(said.beside_a_built_in.length, "built-in skill", "built-in skills")}, which it gets beside purlis's and may use instead: ${said.beside_a_built_in.join(", ")}.`,
    );
  return lines.join(" ");
}
