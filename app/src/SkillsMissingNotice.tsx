// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useEffect, useState } from "react";
import { commands, type PlaneId, type SkillsMissing } from "./bindings";
import { counted } from "./dispatches";
import { Notice, NoticeOf } from "./Notice";
import { askSettingsAction } from "./settings/links";
import { skillStudioAs } from "./skillStudio";

/**
 * **The skills a chat's persona names that it started without, on its tab** (SK-5, #2768; spec
 * decision 41): never a silent drop. An installed skill this machine does not have for the chat's
 * harness, and a built-in skill purlis has none of, each named as the persona's skills write it.
 * It offers
 *
 * - **Open in Skill studio**: the studio filtered to the persona, where the name is taken out or
 *   the skill is seen for what it is;
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
          onPress: () => askSettingsAction(plane, skillStudioAs(said.persona)),
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
  if (said.missing.length > 0)
    lines.push(
      `${chat} started without ${counted(said.missing.length, "skill", "skills")} ${said.persona} names, not installed on this machine: ${said.missing.join(", ")}.`,
    );
  if (said.no_built_in.length > 0)
    lines.push(
      `${said.persona} names ${counted(said.no_built_in.length, "built-in skill", "built-in skills")} purlis does not have: ${said.no_built_in.join(", ")}.`,
    );
  return lines.join(" ");
}
