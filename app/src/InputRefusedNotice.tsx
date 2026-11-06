// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext } from "react";
import type { PlaneId } from "./bindings";
import { forgetInputRefused, useInputRefused } from "./inputRefused";
import { Notice, NoticeOf } from "./Notice";
import { clearedHere } from "./notesClearedEverywhere";

/**
 * **What a chat was not sent, on its pane** (#4734, ADR 0108). What a pane sends its chat waits
 * on the session host in the chat's own lane, never on the window's thread; a send the host
 * refused, or one to a chat whose runner is reconnecting, is said here once, with why. What was
 * typed after it was dropped too, so the person types it again. Dismiss puts it away, in every
 * window of the project (#5363).
 */
export function InputRefusedNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const why = useInputRefused(plane, session);
  const offScreen = useContext(NoticeOf) !== null;
  if (why === undefined) return null;
  const chat = offScreen ? "that chat" : "this chat";
  return (
    <Notice
      cause={`input-refused:${session}`}
      at="pane"
      tone="trouble"
      label="Not sent"
      onDismiss={() => {
        forgetInputRefused(plane, session);
        clearedHere(plane, session, { kind: "input-refused" });
      }}
    >
      purlis could not send {chat} what you typed, or anything after it: {why}. Type it again once
      the chat answers.
    </Notice>
  );
}
