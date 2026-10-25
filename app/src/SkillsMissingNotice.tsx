// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useEffect, useState } from "react";
import { commands, type PlaneId, type SkillsMissing } from "./bindings";
import { counted } from "./dispatches";
import { Notice, NoticeOf } from "./Notice";
import { askSettingsAction } from "./settings/links";
import { SKILL_STUDIO_SHOW, skillStudioAs } from "./skillStudio";

/**
 * **The skills a chat started without, on its tab** (SK-5, #2768; SK-8, #2771; spec decision
 * 41): never a silent drop. An installed skill this machine does not have for the chat's
 * harness, and a built-in skill purlis has none of, each named as the persona's skills write it;
 * a skill the chat would have loaded that could not be loaded, with why; and one the repo it
 * works in leaves out, with why (SK-53). It offers
 *
 * - **Open in Skill studio**: the studio on the chat's persona, or on a chat at the project's
 *   root, where the name is taken out, the skill is fixed in its editor, or it is seen for what
 *   it is;
 * - **Dismiss**: puts it away for this run of the chat.
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
          label: "Open in Skill studio",
          onPress: () =>
            askSettingsAction(
              plane,
              said.persona === null ? SKILL_STUDIO_SHOW : skillStudioAs(said.persona),
            ),
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
  if (said.missing.length > 0)
    lines.push(
      `${chat} started without ${counted(said.missing.length, "skill", "skills")} ${persona} names, not installed on this machine: ${said.missing.join(", ")}.`,
    );
  if (said.no_built_in.length > 0)
    lines.push(
      `${persona} names ${counted(said.no_built_in.length, "built-in skill", "built-in skills")} purlis does not have: ${said.no_built_in.join(", ")}.`,
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
  return lines.join(" ");
}
