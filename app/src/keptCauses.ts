// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// No import: this is read by `Notice.tsx`, which the theme specimen draws, and the specimen
// loads nothing that reads the person's state or the core (`specimen/Specimen.test.tsx`).
// `dismissals.ts`, which keeps the dismissals themselves, reads both.

/**
 * **The families of cause a dismissal is kept for**, each read from the core by a source that
 * settles it: a dormant pin (`plane_pins`' `missing`), what a relaunch did to a chat
 * (`opened_chats`, a reattach whose state the session host could not hand over among it, #2006),
 * a doctor finding that stands as a Notice (`plane_doctor`, #1250), and a sandbox change that left
 * chats behind (`chats_on_older_sandbox`, #1428), a remembered
 * project that is gone (`recent_planes`' `gone`, #1291: `dismissals.ts`' `GONE`), and a
 * workspace's repo left off its default branch or on a moved remote (`stale_clones`, #1985), and
 * a workspace's clone made outside purlis that the person has not trusted (`clones_to_vouch`). A
 * family is the cause up to its first `:`.
 */
export const KEPT: ReadonlySet<string> = new Set([
  "pin-dormant",
  "chat-resumed",
  "chat-guessed",
  "chat-fresh",
  "chat-untaken",
  "doctor-finding",
  "sandbox-changed",
  "project-gone",
  "stale-clone",
  "clone-vouch",
]);
