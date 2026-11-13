// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The reads that follow a move of the project's git standing start apart** (felt speed round
 * 3, H8).
 *
 * Auto-save fetches every five minutes, and the core then says the project's git standing moved
 * (`plane-changed`, `GIT`). Four reads follow that word, each a few seconds of git in the core:
 * the title bar's save standing (`usePlaneSaving`), the workspace's Saving rows
 * (`useRepoSaving`), the Inbox's alerts (`useAlerts`) and the stale repos
 * (`useStaleClones`). Started in the same second, each waited on the others' git, and the
 * slowest took over three seconds.
 *
 * So each starts at its own delay after a move, in order of how soon the person looks at it: the
 * title bar's at once. A reader later in the order finds the clones' shared standings already
 * read by the one before it. Only a move waits: a reader's first read, a return to the window
 * and a press still read at once.
 */
export const AFTER_GIT_MOVED_MS = {
  planeSaving: 0,
  workspaceSaving: 1_500,
  alerts: 3_000,
  staleClones: 4_500,
} as const;
