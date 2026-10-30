// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { INSTRUCTIONS, usePlaneChanged } from "./planeChanged";
import { planeUpdatedAt } from "./PlaneUpdated";
import { useSkillStudioOn } from "./skillStudio";
import { askChatSkills } from "./chatSkillsAsk";

/** What the notice's second action is called (S8, #3694). */
export const WHAT_CHANGED = "What changed";

/** What the restart offer says (decision 15). */
export const SKILLS_CHANGED =
  "Its skills changed since it started. It keeps the skills it started with until it restarts on the same conversation.";

/**
 * **A running chat whose skills changed is told so, and offered a restart** (SK-7, #2770, ADR 0099
 * §5): the project's skills changed since it started in a way its harness does not take in by
 * itself (`chats_plane_updated`'s `skills`, the core's `Chats::skills_changed`). Restart restarts
 * it on the same conversation, once its turn has ended, as every restart does; What changed
 * (S8, #3694) shows this chat's skills in the sidebar Skills view, which names what is new, gone
 * and changed in its Needs you.
 *
 * Asked while the `skill-studio` flag is on: at the mount, and whenever a change on disk concerns
 * what chats started with. Put away, it
 * stays away until the skills change again.
 */
export function SkillsChangedNotice({
  plane,
  session,
  onRestart,
}: {
  plane: PlaneId;
  session: number;
  onRestart: () => void;
}) {
  const changes = usePlaneChanged([plane], INSTRUCTIONS);
  // While the flag is off no chat gets the project's skills, so none is asked about.
  const on = useSkillStudioOn();
  const [changed, setChanged] = useState(false);
  const [putAway, setPutAway] = useState<number>();
  useEffect(() => {
    if (!on) return;
    let gone = false;
    void planeUpdatedAt(plane, changes).then((all) => {
      if (!gone) setChanged(all.some((one) => one.session === session && one.skills));
    });
    return () => {
      gone = true;
    };
  }, [plane, session, changes, on]);
  if (!on || !changed || putAway === changes) return null;
  return (
    <Notice
      cause={`skills-changed:${session}`}
      at="pane"
      label="Skills changed"
      fixes={[
        { label: "Restart", onPress: onRestart },
        { label: WHAT_CHANGED, onPress: () => askChatSkills(plane, session) },
      ]}
      onDismiss={() => setPutAway(changes)}
    >
      {SKILLS_CHANGED}
    </Notice>
  );
}
