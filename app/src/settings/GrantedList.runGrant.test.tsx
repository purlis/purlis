// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { SandboxGrant } from "../bindings";
import { grantSaid } from "./GrantedList";

describe("a Run grant in the Allowed list", () => {
  it("says what it runs, for which chat, and at which scope", () => {
    const grant: SandboxGrant = {
      id: "chat\u0001run-grant\u00017",
      what: "run-grant",
      target: "/usr/bin/curl with TOKEN from vault team",
      persona: null,
      level: "chat",
      by: null,
      at: null,
      chat: "Deploy",
      locked: null,
      waiting: null,
      for_no_persona: false,
      expected: false,
    };
    const said = grantSaid(grant);
    expect(said).toContain("Run /usr/bin/curl with TOKEN from vault team");
    expect(said).toContain("This chat");
    expect(said).toContain("from Deploy");
  });
});
