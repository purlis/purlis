// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { PlaneId } from "./bindings";
import { moveSaid, putAwayMove, useChatMoves } from "./chatMoves";
import { Notice } from "./Notice";

/**
 * **This chat's move, on its pane** (#2048): that it is moving and moves at its next pause, with
 * Cancel move while it has not; or, where it did not move, why, until it is dismissed. The chat
 * keeps running where it is in every case.
 */
export function MoveNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const one = useChatMoves(plane)[session];
  if (one === undefined) return null;
  const said = moveSaid(one);
  if (said.trouble)
    return (
      <Notice
        cause={`chat-move-failed:${session}`}
        at="pane"
        tone="trouble"
        label="Move"
        onDismiss={() => putAwayMove(plane, session)}
      >
        {said.words}
      </Notice>
    );
  return (
    <Notice
      cause={`chat-move:${session}`}
      at="pane"
      label="Move"
      fixes={[{ label: "Cancel move", onPress: () => putAwayMove(plane, session) }]}
    >
      {said.words}
    </Notice>
  );
}
