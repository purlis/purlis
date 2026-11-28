// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { PolicyLog, PolicyState } from "./bindings";

/**
 * **What a test or a story starts the project's policy from** (Q308a, instruction 45): a project
 * that names no template, so runs `watch`, the shipped default, with nothing set apart, nothing
 * waiting and its acceptance sealed.
 */
export const WATCH: PolicyState = {
  waiting: [],
  refused: [],
  not_in_force: [],
  unsealed: false,
  key_foreign: false,
  new_key: false,
  nevers_new_key: false,
  nevers_to_keep: null,
  nevers_unshowable: false,
  nevers_unvouched: false,
  nevers_to_seal: null,
  digest: "d1",
  template: "watch",
  template_default: true,
  overrides: [],
  locked: [],
};

/** Two lines of the enforcement log as the app writes them: a known rule each, and a host:port. */
export const TWO_LINES: PolicyLog = {
  count: 2,
  dropped: 0,
  lines: [
    { at: "2026-11-17T11:00:00Z", rule: "A3", family: "guards.git", session: 12, host: null },
    {
      at: "2026-11-17T10:00:00Z",
      rule: "host-ask",
      family: "asks",
      session: null,
      host: "example.com:443",
    },
  ],
};
