// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { AnswersShown } from "../bindings";

/** Fixture data for the stories and tests of the sealed answers' row and Notices. */
export const HOLDS: AnswersShown = {
  state: "holds",
  deny: [],
  imported: [],
  old_files: [],
  import_failed: null,
  notices: [],
  device: { state: "confirmed", id: "01JABCDEFGHJKMNPQRSTVWXYZ0" },
  hello: { state: "unavailable", gap: null },
};

const FULL_GAP =
  "declared gap: on Windows any program you run can read the key purlis seals your answers with.";
const NARROWED_GAP =
  "declared gap, narrowed: the seal key is released only after Windows Hello at each start.";

/** A Windows machine with the opt-in off: the full gap. */
export const HELLO_OFF: AnswersShown = { ...HOLDS, hello: { state: "off", gap: FULL_GAP } };

/** A Windows machine with the opt-in on: the gap narrowed, never closed. */
export const HELLO_ON: AnswersShown = { ...HOLDS, hello: { state: "on", gap: NARROWED_GAP } };

export const WAITING_FOR_CONFIRM: AnswersShown = {
  ...HOLDS,
  device: { state: "unconfirmed", id: "01JABCDEFGHJKMNPQRSTVWXYZ0" },
};

export const IMPORTED: AnswersShown = {
  ...HOLDS,
  imported: ["steward -> devops", "qa -> prod"],
  old_files: ["app/dispatch-never.json"],
};

export const DENIED: AnswersShown = {
  ...HOLDS,
  state: "deny",
  deny: ["the project was moved: its answers are anchored at /old/place"],
  device: { state: "unset", id: null },
};

export const IMPORT_FAILED: AnswersShown = {
  ...HOLDS,
  state: "deny",
  deny: ["the nevers you said before are still in their old files and are not imported yet"],
  import_failed:
    "purlis could not read the nevers you said before, so it did not import them: they stay in their old file.",
};
