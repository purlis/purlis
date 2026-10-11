// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0
import type { GrantLevel } from "./bindings";

/**
 * **The words an Allow says, written once for the window** (#1700, `docs/ui-copy.md`).
 *
 * The asks registry (`app/src-tauri/src/asking.rs`, `allow_said`) writes the labels every Notice
 * and the Inbox draw from an ask's options, so those spell none of their own. What the window
 * draws with no ask behind it (a refusals list's Allow) and the window tests' stand-in for the
 * registry take them from here. `allowLabels.test.ts` reads `asking.rs` and fails when the two
 * drift apart.
 */

/** What an Allow at each level says where every level is offered side by side. */
export const ALLOW_SAID: Readonly<Record<GrantLevel, string>> = {
  chat: "Allow for this chat",
  you: "Allow for me on this machine",
  project: "Allow for everyone in this project",
};

/**
 * What a sandbox ask's Allow says at each level: its main button is the wider one, this project
 * on this machine (#1666, N-3), so the this-chat Allow says "only".
 */
export const SANDBOX_ALLOW_SAID: Readonly<Record<GrantLevel, string>> = {
  ...ALLOW_SAID,
  chat: "Allow only for this chat",
};

/** What Keep blocked says, for a dispatch and a sandbox ask alike. */
export const KEEP_SAID = "Keep blocked";

/** What a dispatch's Never for this pair says. */
export const NEVER_SAID = "Never for this pair";
