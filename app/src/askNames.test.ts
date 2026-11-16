// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { Shown } from "./bindings";
import { tabsMarked, unlockAskAt } from "./askNames";

/** A key missing from your keys (W12, T1a-27's): no chat's, naming the chats it stopped. */
const missing = (names: number[]): Shown => ({
  session: 0,
  ask: "model-key-missing:01JA0000000000000000000000",
  says: "A model key is missing",
  options: [{ id: "dismiss", label: "Dismiss", allows: false }],
  source: "model-key-missing",
  chain: ["purlis"],
  answer: { via: "hook" },
  names,
});

describe("a no-chat model key Ask marks the tabs of the chats it names (T1a-61, R-592)", () => {
  it("a_no_chat_model_key_ask_marks_each_named_chats_tab", () => {
    const ask = missing([3, 5]);
    expect([...tabsMarked([ask], [3, 4, 5])].sort()).toEqual([3, 5]);
    // Chat 3 closes: its tab is unmarked, and chat 5's stays marked while the Ask stands.
    expect([...tabsMarked([ask], [4, 5])]).toEqual([5]);
  });

  it("marks nothing for an Ask that names no chat, or for another kind of no-chat Ask", () => {
    expect([...tabsMarked([missing([])], [3])]).toEqual([]);
    const signIn: Shown = { ...missing([3]), source: "sign-in", answer: { via: "hook" } };
    expect([...tabsMarked([signIn], [3])]).toEqual([]);
  });
});

describe("your keys locked at a start marks the tab only when unattended (R-592)", () => {
  it("stands in place in New chat, marking no tab, while the person is there", () => {
    expect(unlockAskAt(true)).toEqual({ at: "new-chat", marksTab: false });
  });

  it("goes to Needs you, marking its tab, when the chat is unattended", () => {
    expect(unlockAskAt(false)).toEqual({ at: "needs-you", marksTab: true });
  });
});
