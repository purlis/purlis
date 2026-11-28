// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Why } from "./bindings";

/**
 * **Why a chat was not hibernated, in the window's words** (SC-4c, #7632; `docs/ui-copy.md`,
 * *Why a chat is not hibernated*). The core says which reason as a typed value
 * (`purlis_core::rested::NotRested`), for the person's Hibernate and the app's own alike, and
 * these are its words: what happened, then what to do where there is something to do.
 */
export const NOT_HIBERNATED: Record<Why, string> = {
  working:
    "Not hibernated: this chat is not at the end of a turn. Hibernate it once its turn ends.",
  asking: "Not hibernated: this chat is asking you something. Answer it first.",
  paused: "Not hibernated: this chat is paused. Let it go on first.",
  hibernated: "Not hibernated: this chat is already hibernated. Resume it to go on.",
  ended: "Not hibernated: this chat has ended.",
  cannot_resume: "Not hibernated: this chat's harness cannot resume a conversation by itself.",
  cannot_see_its_own_work:
    "Not hibernated: purlis cannot see the background work this chat's harness leaves running.",
  own_work: "Not hibernated: this chat has background work running. Wait for it to finish.",
  line_coming:
    "Not hibernated: purlis is about to type a line into this chat, such as a task's report. Hibernate it once that turn ends.",
  task_open:
    "Not hibernated: a task this chat dispatched is still running or has not reported. Wait for its report.",
  report_owed:
    "Not hibernated: this chat is a task, and the chat that dispatched it is waiting on its report.",
  observed: "Not hibernated: this chat is only watched here, not run by purlis.",
  on_a_runner: "Not hibernated: this chat runs on a runner, not on this machine.",
  child_run: "Not hibernated: this is a sub-agent of a chat, not a chat of its own.",
  looked_at: "Not hibernated: someone looked at this chat or typed in it recently.",
};
