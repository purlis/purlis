// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **A project without skill controls** (#6031), as `skill_policy` answers it: what a test or
 *  story that draws Sources fakes the command with when its project holds no `[skill_policy]`. */
export const NO_SKILL_POLICY = {
  file: "charter.toml",
  state: { kind: "none" as const },
  sources: [],
};
