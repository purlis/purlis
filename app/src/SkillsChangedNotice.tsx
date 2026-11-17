// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { INSTRUCTIONS, usePlaneChanged } from "./planeChanged";
import { planeUpdatedAt } from "./PlaneUpdated";
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
 * Asked at the mount, and whenever a change on disk concerns what chats started with. Put away,
 * it stays away until the skills change again.
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
  /** What changed in this chat's skills, as a key, or none: one bulk change is many events on
   *  disk and one key, so its offer is shown once however many arrive (#5331). */
  const [changed, setChanged] = useState<string>();
  const [putAway, setPutAway] = useState<string>();
  useEffect(() => {
    let gone = false;
    void planeUpdatedAt(plane, changes).then((all) => {
      if (gone) return;
      const mine = all.find((one) => one.session === session && one.skills);
      // Back as it started (an Undo): the same change made again is news again.
      if (mine === undefined) setPutAway(undefined);
      setChanged(
        mine === undefined
          ? undefined
          : JSON.stringify([mine.skills_added, mine.skills_gone, mine.skills_edited]),
      );
    });
    return () => {
      gone = true;
    };
  }, [plane, session, changes]);
  if (changed === undefined || putAway === changed) return null;
  return (
    <Notice
      cause={`skills-changed:${session}`}
      at="pane"
      label="Skills changed"
      fixes={[
        { label: "Restart", onPress: onRestart },
        { label: WHAT_CHANGED, onPress: () => askChatSkills(plane, session) },
      ]}
      onDismiss={() => setPutAway(changed)}
    >
      {SKILLS_CHANGED}
    </Notice>
  );
}
