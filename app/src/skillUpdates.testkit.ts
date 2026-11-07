// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **An update offered and its review, as the core answers them** (#5354): what the tests and
 * stories of Review updates fake `skill_updates` and `skill_update_review` with.
 */
import type { SkillGained, SourceRemote } from "./bindings";
import { PROJECT } from "./skillChanges";

export const commit = (c: string) => ({ id: c.repeat(40), short: c.repeat(7) });

/** A summary that holds nothing. */
export const EMPTY_SUMMARY = {
  empty: true,
  scripts: [],
  hosts: [],
  commands: [],
  outside: [],
  unseen: [],
  not_text: [],
};

const REMOTE: SourceRemote = {
  url: "https://example.com/skills",
  branch: null,
  label: "example",
};

/** The update of the project skill `name` from `remote`, offered. */
export const updateOffer = (name: string, remote: SourceRemote = REMOTE) => ({
  level: PROJECT,
  name,
  label: "project",
  remote,
  pinned: commit("1"),
  newer: commit("2"),
  edited: false,
});

/** The review of `name`'s update, which gains `gained`, its SKILL.md's diff `diff`. */
export const updateReview = (
  name: string,
  gained: SkillGained[] = [],
  diff = "@@ -6,1 +6,1 @@\n-Post a note.\n+Post a short note, then stop.\n",
) => ({
  name,
  pinned: commit("1"),
  newer: commit("2"),
  from_pin: true,
  added: EMPTY_SUMMARY,
  removed: EMPTY_SUMMARY,
  files: [{ path: "SKILL.md", change: "changed", diff }],
  edited: [],
  local_edits: [],
  skill: { name, summary: EMPTY_SUMMARY, files: [] },
  brought: [],
  grant: null,
  widenings: [],
  accepted: { commit: "2".repeat(40), skills: [{ name, digest: `d-${name}` }] },
  gained,
});
