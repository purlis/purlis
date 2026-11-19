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
};

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
