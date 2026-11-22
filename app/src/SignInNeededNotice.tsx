// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PlaneId, SignInNeeded } from "./bindings";
import { signInNeededOf, useChatsHere, useChatsSelect } from "./chatState";
import { Notice } from "./Notice";
import { askSettingsLink, type SettingsLink } from "./settings/links";

/** Settings › You › Accounts, where one sign-in per forge host is made (ADR 0128). */
const ACCOUNTS: SettingsLink = { group: "you.accounts" };

/** What the Notice says of `needed`: which forge, and whether the sign-in is missing or ended,
 *  then what the chat can't do meanwhile and what to do. */
export function signInNeededSaid(needed: SignInNeeded): string {
  const state = needed.ended
    ? `Your sign-in to ${needed.host} has ended`
    : `There is no sign-in to ${needed.host} for this project`;
  return `${state}, so purlis can't do this chat's forge work there. Sign in under Accounts, then ask the chat to try again.`;
}

/** What names this Notice's cause: the chat, the host and the state, so another host or another
 *  state is a new cause and brings a dismissed one back. */
export function signInNeededCause(session: number, needed: SignInNeeded): string {
  return `forge-sign-in-needed:${session}:${needed.host}:${needed.ended ? "ended" : "missing"}`;
}

/**
 * **A forge sign-in this chat needs, on its pane** (ADR 0128, #7290): its route found the
 * project's sign-in for the host missing or ended. A standing fault until the person acts, so a
 * Notice: **Sign in** opens Settings at Accounts, and **Dismiss** puts it away until another host
 * or another state brings it back. A forge that merely could not be reached is never told.
 */
export function SignInNeededNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const needed = useChatsSelect(useChatsHere(), (states) => signInNeededOf(states, session));
  const [dismissed, setDismissed] = useState<string | null>(null);
  if (needed === undefined) return null;
  const cause = signInNeededCause(session, needed);
  if (dismissed === cause) return null;
  return (
    <Notice
      cause={cause}
      at="pane"
      label="Forge sign-in"
      tone="trouble"
      link={{ label: "Sign in", onPress: () => askSettingsLink(plane, ACCOUNTS) }}
      onDismiss={() => setDismissed(cause)}
    >
      {signInNeededSaid(needed)}
    </Notice>
  );
}
