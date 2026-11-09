// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands, type SigningLane, type SigningRetried } from "./bindings";

/** Told on `window` whenever work waiting on signing may have moved: set up, signed or sent. */
export const SIGNING_WAITING_CHANGED = "purlis:signing-waiting-changed";

/** Told on `window` when the one Ask about lanes waiting on signing was answered Set up signing
 *  (#5633): the project's window opens the setup dialog, ending in Sign and retry all. */
export const SIGNING_SETUP_ASKED = "purlis:signing-setup-asked";

/** What `SIGNING_SETUP_ASKED` carries: the project, and the forge setup is for where the lanes
 *  wait on more than one (#5790). */
export type SigningSetupAskedDetail = { plane: string; forge?: { kind: string; host: string } };

/** Asks `plane`'s window to open Set up signing for its waiting lanes: those on `forge`, where
 *  one is named (#5790), else every one. */
export function askSigningSetUp(plane: string, forge?: { kind: string; host: string }): void {
  window.dispatchEvent(
    new CustomEvent<SigningSetupAskedDetail>(SIGNING_SETUP_ASKED, { detail: { plane, forge } }),
  );
}

/** The forge a `set-up:<kind>:<host>` answer names (#5790), or none for a plain `set-up`. */
export function setUpForgeOf(option: string): { kind: string; host: string } | undefined {
  if (!option.startsWith("set-up:")) return undefined;
  const rest = option.slice("set-up:".length);
  const colon = rest.indexOf(":");
  if (colon <= 0 || colon === rest.length - 1) return undefined;
  return { kind: rest.slice(0, colon), host: rest.slice(colon + 1) };
}

/** The lane's key: its workspace, repo and piece. */
export const laneOf = (one: SigningLane): SigningLane => ({
  ws: one.ws,
  repo: one.repo,
  piece: one.piece,
});

/**
 * **Sign and retry `lanes`** (ADR 0116 §4): each signed with this Mac's key and sent again, and
 * every reader told. What each came to, or the one sentence why none was tried.
 */
export async function signAndRetry(
  plane: string,
  lanes: readonly SigningLane[],
): Promise<readonly SigningRetried[] | string> {
  const done = await commands.signingSignAndRetry(plane, lanes.map(laneOf));
  window.dispatchEvent(new Event(SIGNING_WAITING_CHANGED));
  return done.status === "ok" ? done.data : done.error;
}

/** **Sign and retry every lane of `plane` still waiting on signing** (#5633). */
export async function signAndRetryAll(plane: string): Promise<readonly SigningRetried[] | string> {
  const read = await commands.signingWaiting(plane);
  if (read.status === "error") return read.error;
  return signAndRetry(
    plane,
    read.data.filter((one) => !one.sent),
  );
}
