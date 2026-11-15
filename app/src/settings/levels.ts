// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// A plain module, so a model such as `chatLog.ts` (and through it `tabs.ts`, which the test
// setup loads) names a sandbox level without loading Settings' controls.

/** The three levels, strictest first, as the file writes them. */
export const LEVELS = ["on", "floor", "off"] as const;

/** What each level is called here, as on a chat's tab. */
export const LEVEL_SAID: Readonly<Record<(typeof LEVELS)[number], string>> = {
  on: "Full sandbox",
  floor: "Basic sandbox",
  off: "No sandbox",
};
