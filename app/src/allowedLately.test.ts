// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { DispatchGrant, KeptBlocked, SandboxGrant } from "./bindings";
import { LATELY_MS, lately } from "./allowedLately";

/**
 * **What was allowed here in the last day** (D-1700-7): the empty Inbox's look back, read from
 * the grants the sources keep, so an Allow on a pane or before a relaunch is listed too.
 */

const NOW = Date.parse("2026-10-11T12:00:00Z");
const ago = (ms: number) => Math.floor((NOW - ms) / 1000);

const host = (id: string, at: number | null, more: Partial<SandboxGrant> = {}): SandboxGrant => ({
  id,
  what: "host",
  target: `${id}.example.com`,
  persona: null,
  level: "you",
  by: null,
  at,
  chat: "steward 3",
  locked: null,
  waiting: null,
  for_no_persona: false,
  expected: false,
  ...more,
});

const dispatch = (id: string, at: number, more: Partial<DispatchGrant> = {}): DispatchGrant => ({
  id,
  asking: "steward",
  target: "devops",
  level: "chat",
  by: null,
  at,
  chat: "steward 4",
  locked: null,
  waiting: false,
  declined: false,
  workspace: null,
  nowhere: null,
  ...more,
});

describe("what was allowed lately", () => {
  it("lists this machine's grants of the last day, newest first, with who they are for", () => {
    const listed = lately(
      [
        host("api", ago(60_000)),
        host("old", ago(LATELY_MS + 1000)),
        host("team", ago(1000), { by: "dana" }),
        host("unknown", null),
        host("out", ago(5_000), { what: "write", target: "/w/out", level: "chat" }),
      ],
      [dispatch("d1", ago(30_000)), dispatch("d2", ago(10_000), { declined: true })],
      NOW,
    );
    expect(listed.map((one) => [one.says, one.level, one.chat])).toEqual([
      ["Write in /w/out", "for that chat", "steward 3"],
      ["steward dispatches to devops", "for that chat", "steward 4"],
      ["Reach api.example.com", "for you on this machine", "steward 3"],
    ]);
  });

  it("draws every name a chat gave it with what draws as nothing written out (I-1)", () => {
    const [listed] = lately(
      [
        host("bidi", ago(1000), {
          what: "write",
          target: "/w/\u202egnp.exe",
          chat: "steward\u200b 3",
        }),
      ],
      [],
      NOW,
    );
    expect(listed?.says).toBe("Write in /w/⟦U+202E⟧gnp.exe");
    expect(listed?.chat).toBe("steward⟦U+200B⟧ 3");
  });

  it("lists the person's Keep blocked answers beside what was allowed (#1700)", () => {
    const kept: KeptBlocked[] = [
      { target: "pypi.org:443", chat: "steward 3", at: ago(2_000) },
      { target: "old.example.com:443", chat: null, at: ago(LATELY_MS + 1000) },
    ];
    const listed = lately([host("api", ago(60_000))], [], NOW, kept);
    expect(listed.map((one) => [one.says, one.level, one.chat])).toEqual([
      ["Kept pypi.org:443 blocked", "for that chat", "steward 3"],
      ["Reach api.example.com", "for you on this machine", "steward 3"],
    ]);
  });

  it("draws a host kept blocked with what draws as nothing written out (I-1)", () => {
    const [listed] = lately([], [], NOW, [
      { target: "a\u202eb.example.com:443", chat: "steward\u200b 3", at: ago(1000) },
    ]);
    expect(listed?.says).toBe("Kept a⟦U+202E⟧b.example.com:443 blocked");
    expect(listed?.chat).toBe("steward⟦U+200B⟧ 3");
  });
});
