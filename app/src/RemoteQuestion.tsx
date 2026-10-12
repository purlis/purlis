// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Notice } from "./Notice";
import type { RemoteQuestion } from "./bindings";

/**
 * **The question about a clone's remote, as a Notice in the Saving view** (#1848).
 *
 * An automatic save pushes with the person's purlis sign-in only to the remote recorded for the
 * clone. A clone purlis has no remote recorded for, on a machine with no forge CLI to push it
 * with, waits for one save from the window: the Notice names the remote, and its press records
 * it and saves (`confirm_remote`). `name` is what the sentence calls the clone: "this project",
 * or a repo's name.
 */
export function RemoteQuestionNotice({
  question,
  name,
  busy,
  onConfirm,
}: {
  question: RemoteQuestion;
  name: string;
  busy: boolean;
  onConfirm: () => void;
}) {
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
