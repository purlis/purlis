// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useState } from "react";
import type { PlaneId } from "./bindings";
import { Notice, NoticeOf } from "./Notice";
import { draftReadySays, useSkillDrafts } from "./skillDraftAsks";
import { AT_THE_ROOT, OPEN_SKILL_STUDIO, askSkillStudio, useSkillStudioOn } from "./skillStudio";

/**
 * **"Draft ready in Skill studio", on the tab of the chat that drafted it** (SK-9, #2772,
 * decision 38): a chat's write to a skill's files is kept as a draft skill, and this is all its
 * tab says of it. Trying, accepting and rejecting it are the studio's alone, and nothing else
 * asks for it: the Inbox does not list it.
 *
 * - **Open Skill studio**: the studio's Drafts, where the draft waits with its diff (#3681), at
 *   that draft's own page where the chat drafted one (S8, #3690).
 * - **Dismiss**: puts it away until the chat drafts again.
 */
export function DraftReadyNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const on = useSkillStudioOn();
  const drafts = useSkillDrafts(plane, 0, on);
  const offScreen = useContext(NoticeOf) !== null;
  const [putAway, setPutAway] = useState<string>();
  const drafted = drafts.filter((draft) => draft.chat === session);
  const mine = drafted.map((draft) => draft.name);
  const key = mine.join("\n");
  if (!on || mine.length === 0 || putAway === key) return null;
  return (
    <Notice
      cause={`draft-ready:${session}`}
      at="pane"
      label="Draft ready in Skill studio"
      fixes={[
        {
          label: OPEN_SKILL_STUDIO,
          onPress: () =>
            askSkillStudio(
              plane,
              AT_THE_ROOT,
              "drafts",
              drafted.length === 1 ? drafted[0].id : undefined,
            ),
        },
      ]}
      onDismiss={() => setPutAway(key)}
    >
      {draftReadySays(mine, offScreen)}
    </Notice>
  );
}
