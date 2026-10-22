// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useState } from "react";
import type { PlaneId } from "./bindings";
import { Notice, NoticeOf } from "./Notice";
import { askSettingsAction } from "./settings/links";
import { draftReadySays, useSkillDrafts } from "./skillDraftAsks";
import { SKILL_STUDIO_SHOW, useSkillStudioOn } from "./skillStudio";

/**
 * **"Draft ready in Skill studio", on the tab of the chat that drafted it** (SK-9, #2772,
 * decision 38): a chat's write to a skill's files is kept as a draft skill, and this is all its
 * tab says of it. Trying, accepting and rejecting it are the studio's alone, and nothing else
 * asks for it: the Inbox does not list it.
 *
 * - **Open in Skill studio**: the studio, where the draft waits with its diff.
 * - **Dismiss**: puts it away until the chat drafts again.
 */
export function DraftReadyNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const on = useSkillStudioOn();
  const drafts = useSkillDrafts(plane, 0, on);
  const offScreen = useContext(NoticeOf) !== null;
  const [putAway, setPutAway] = useState<string>();
  const mine = drafts.filter((draft) => draft.chat === session).map((draft) => draft.name);
  const key = mine.join("\n");
  if (!on || mine.length === 0 || putAway === key) return null;
  return (
    <Notice
      cause={`draft-ready:${session}`}
      at="pane"
      label="Draft ready in Skill studio"
      fixes={[
        {
          label: "Open in Skill studio",
          onPress: () => askSettingsAction(plane, SKILL_STUDIO_SHOW),
        },
      ]}
      onDismiss={() => setPutAway(key)}
    >
      {draftReadySays(mine, offScreen)}
    </Notice>
  );
}
