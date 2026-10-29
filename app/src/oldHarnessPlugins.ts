// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import { askOpenProposal } from "./proposedChangeOpen";

/**
 * **Remove on the old `[harness_plugins]` keys** (#3467): the core holds purlis's proposed
 * change for each file that holds them, and the first one's tab opens, where Apply is pressed.
 * Nothing is written before. The Inbox's Ask (`asks.ts`) and the studio's Needs you (#3689) both
 * run it; the answer is the core's refusal, if any.
 */
export async function proposeOldKeysRemoval(
  plane: string,
): Promise<{ status: "ok" } | { status: "error"; error: string }> {
  const held = await commands.removeOldHarnessPlugins(plane);
  if (held.status === "error") return held;
  const [first] = held.data;
  if (first !== undefined) askOpenProposal(plane, first.id, first.file);
  return { status: "ok" };
}
