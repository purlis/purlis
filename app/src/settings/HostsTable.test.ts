// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { hostAndPort } from "./HostsTable";

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
