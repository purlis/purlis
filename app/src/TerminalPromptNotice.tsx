// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef } from "react";
import type { Shown } from "./bindings";
import { agoSaid } from "./inboxRules";
import { useNow } from "./InboxRow";
import { Notice } from "./Notice";
import { STRIP, waitOf } from "./waitingOn";

/**
 * **The chat on screen is stopped on a prompt in its own terminal** (#5215, the approved
 * prototype): one row of the band under the head, with Go to prompt, which puts the keyboard in
 * the terminal below. It answers nothing: the person answers in the terminal, where the harness
 * shows the prompt in full.
 *
 * A chat that is not on screen says so with its own Notice (`TaskPromptNotice`).
 */
export function TerminalPromptNotice({
  session,
  waiting,
}: {
  /** The chat the pane shows. */
  session: number;
  /** The project's asks, as the registry derived them last. */
  waiting: readonly Shown[] | undefined;
}) {
  const wait = waitOf(waiting, session);
  const now = useNow();
  const at = useRef<HTMLSpanElement>(null);
  if (wait === undefined || wait.kind === "prose") return null;
  const since = waiting?.find((ask) => ask.ask === wait.ask)?.since;
  const age = since == null ? undefined : agoSaid(since * 1000, now);
  return (
    <Notice
      at="pane"
      cause={`terminal-prompt:${wait.ask}`}
      tone="attention"
      label={STRIP.prompt}
      waits
      mark="prompt"
      sub={age === undefined ? STRIP.answerBelow : `${STRIP.answerBelow} · ${age}`}
      fixes={[
        {
          label: STRIP.goToPrompt,
          onPress: () =>
            at.current
              ?.closest(".pane-frame")
              ?.querySelector<HTMLElement>(".xterm-helper-textarea")
              ?.focus(),
        },
      ]}
    >
      <span ref={at} className="terminal-prompt-says">
        {wait.says}
      </span>
    </Notice>
  );
}
