// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Notice } from "./Notice";
import type { RemoteQuestion } from "./bindings";

/**
 * **The question about a clone's remote, as a Notice in the Saving view** (#1848, #1852).
 *
 * An automatic save pushes with the person's purlis sign-in only to the remote recorded for the
 * clone. Where it cannot, this Notice names the remote and the person's press settles it:
 *
 * - `not-saved`: purlis has no remote recorded, and there is no forge CLI to push with. *Save
 *   once from here* records the remote and saves (`confirm_remote`).
 * - `changed`: the remote is not the one purlis last saved to (D-254-6). *Save to B and remember
 *   it* records the new one and saves (`confirm_remote`); *Keep A* points the clone back at the
 *   recorded one and saves (`keep_recorded_remote`). Either way the next save does not ask.
 *
 * `name` is what the sentence calls the clone: "this project", or a repo's name.
 */
export function RemoteQuestionNotice({
  question,
  name,
  busy,
  onConfirm,
  onKeep,
}: {
  question: RemoteQuestion;
  name: string;
  busy: boolean;
  onConfirm: () => void;
  onKeep: () => void;
}) {
  if (question.kind === "changed" && question.recorded != null) {
    return (
      <Notice
        cause={`saving-remote-changed:${name}`}
        tone="trouble"
        fixes={[
          { label: `Save to ${question.now} and remember it`, onPress: onConfirm, busy },
          { label: `Keep ${question.recorded}`, onPress: onKeep, busy },
        ]}
      >
        {`The remote of ${name} changed from ${question.recorded} to ${question.now} since ` +
          `purlis last saved it, so its automatic saves do not push. Choose where it saves.`}
      </Notice>
    );
  }
  return (
    <Notice
      cause={`saving-remote-not-saved:${name}`}
      tone="trouble"
      fixes={[{ label: "Save once from here", onPress: onConfirm, busy }]}
    >
      {`purlis has not saved ${name} from the window yet, so its automatic saves do not push ` +
        `to ${question.now}. Save it once from here to confirm that remote: from then on ` +
        `automatic saves push with your purlis sign-in.`}
    </Notice>
  );
}
