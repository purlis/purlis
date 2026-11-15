// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// A plain module, so a model such as `chatLog.ts` (and through it `tabs.ts`, which the test
// setup loads) names a harness without loading Settings' profile pages and the terminal they
// draw.

/** The harnesses a profile can be made of, as the core names them. */
export const HARNESSES = [
  { value: "claude", label: "Claude Code" },
  { value: "codex", label: "Codex" },
  { value: "opencode", label: "opencode" },
];
