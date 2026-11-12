// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { addedSaid, hostAndPort } from "./HostsTable";

describe("a host's port, as Your hosts shows it (R65)", () => {
  it.each([
    ["docs.github.com", "docs.github.com", "443"],
    ["netlog-git.invalid:8443", "netlog-git.invalid", "8443"],
    ["*.example.com", "*.example.com", "443"],
    ["10.0.0.5", "10.0.0.5", "443"],
    ["10.0.0.5:5432", "10.0.0.5", "5432"],
    ["[fd00::7]:8443", "[fd00::7]", "8443"],
    // A bare IPv6 address's last part is no port: HTTPS's is.
    ["fd00::7", "fd00::7", "443"],
  ])("reads %s as %s on %s", (written, host, port) => {
    expect(hostAndPort(written)).toEqual({ host, port });
  });
});

describe("when and by what a host was added, as Your hosts says it (R65, #6172)", () => {
  const now = new Date(Date.UTC(2026, 10, 12, 12, 0, 0));
  const at = now.getTime() / 1000;
  it.each([
    [undefined, "Earlier"],
    [{ at: at - 5, by: "you" as const }, "just now · you"],
    // A clock that moved: an add after now is just now.
    [{ at: at + 600, by: "you" as const }, "just now · you"],
    [{ at: at - 60, by: "block" as const }, "1 minute ago · a chat's block"],
    [{ at: at - 2 * 3600, by: "you" as const }, "2 hours ago · you"],
    [{ at: at - 86_400, by: "you" as const }, "1 day ago · you"],
    [{ at: at - 29 * 86_400, by: "block" as const }, "29 days ago · a chat's block"],
    [{ at: at - 30 * 86_400, by: "you" as const }, "13 Oct 2026 · you"],
  ])("says %o as %s", (added, said) => {
    expect(addedSaid(added, now)).toBe(said);
  });
});
