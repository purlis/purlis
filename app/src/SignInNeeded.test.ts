// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { ChatNoticed } from "./bindings";
import { noticed, nothingKnown, signInNeededOf } from "./chatState";
import { signInNeededCause, signInNeededSaid } from "./SignInNeededNotice";

/** What the core noticed of chat 4: nothing of Stuck?, work left or mode, and `sign_in` as given. */
const told = (sign_in: ChatNoticed["sign_in"]): ChatNoticed => ({
  plane: "/p",
  session: 4,
  stuck: null,
  left_work: null,
  mode: null,
  sign_in,
});

describe("a forge sign-in a chat needs", () => {
  it("is held for its chat once the core tells it, and for no other chat", () => {
    const states = noticed(nothingKnown, told({ host: "github.com", ended: false }));
    expect(signInNeededOf(states, 4)).toEqual({ host: "github.com", ended: false });
    expect(signInNeededOf(states, 5)).toBeUndefined();
  });

  it("changes the states only when the host or the state changed", () => {
    const first = noticed(nothingKnown, told({ host: "github.com", ended: false }));
    expect(noticed(first, told({ host: "github.com", ended: false }))).toBe(first);
    expect(noticed(first, told({ host: "github.com", ended: true }))).not.toBe(first);
  });

  it("says which forge, whether it is missing or ended, and where to sign in", () => {
    expect(signInNeededSaid({ host: "github.com", ended: false })).toBe(
      "There is no sign-in to github.com for this project, so purlis can't do this chat's forge work there. Sign in under Accounts, then ask the chat to try again.",
    );
    expect(signInNeededSaid({ host: "gitlab.com", ended: true })).toMatch(
      /^Your sign-in to gitlab\.com has ended, /,
    );
  });

  it("is a new cause for another host or another state, so a dismissal does not hide it", () => {
    const missing = signInNeededCause(4, { host: "github.com", ended: false });
    expect(signInNeededCause(4, { host: "github.com", ended: true })).not.toBe(missing);
    expect(signInNeededCause(4, { host: "gitlab.com", ended: false })).not.toBe(missing);
    expect(missing.split(":")[0]).toBe("forge-sign-in-needed");
  });
});
