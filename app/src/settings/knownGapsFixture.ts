// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { HarnessGaps, KnownGapShown } from "../bindings";

/** Fixture data for the stories and tests of the Known gaps list and the pane head's Notice. */
export const CODEX_HOOKS: KnownGapShown = {
  id: "hook-fails-open",
  said: "A hook that times out or dies lets the tool run, and a hook alone enforces these guards.",
  instead: "purlis bounds its own guard to 2 s.",
  from: "#6972 §4 gap 14; spike 03",
  families: ["guards.git", "guards.shape"],
  enforced: ["guards.git", "guards.shape"],
  conditional: false,
};

export const CLAUDE_READS: KnownGapShown = {
  id: "claude-code-reads",
  said: "Claude Code's own sandbox has no read list, only deny paths.",
  instead: "Seatbelt and Landlock compile a read list; Claude Code's does not.",
  from: "spec §11.5",
  families: ["sandbox"],
  enforced: ["sandbox"],
  conditional: false,
};

export const OLDER_CLAUDE: KnownGapShown = {
  id: "claude-code-minimum-version",
  said: "A Claude Code older than 2.1.285 does not take purlis's proxy.",
  instead: "Its connections are not judged by the policy.",
  from: "spec §11.7",
  families: ["network"],
  enforced: ["network"],
  conditional: true,
};

export const HARNESSES: HarnessGaps[] = [
  { harness: "claude", gaps: [CLAUDE_READS, OLDER_CLAUDE] },
  { harness: "codex", gaps: [CODEX_HOOKS] },
  { harness: "opencode", gaps: [] },
];
