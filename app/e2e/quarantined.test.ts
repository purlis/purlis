// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { quarantinedSpecs } from "./quarantined.js";

const file = join(mkdtempSync(join(tmpdir(), "quarantine-")), "quarantine.toml");
writeFileSync(
  file,
  '[[test]]\nid = "e2e/specs/thing.e2e.ts"\nticket = 7003\nadded = "2026-11-19"\n',
);

describe("a quarantined spec file", () => {
  it("is left out of a pull request run and of the queue run", () => {
    for (const event of ["pull_request", "merge_group"]) {
      expect(
        quarantinedSpecs({ CI: "true", GITHUB_EVENT_NAME: event, PURLIS_QUARANTINE_FILE: file }),
      ).toEqual(["./specs/thing.e2e.ts"]);
    }
  });

  it("is run by the nightly run", () => {
    expect(
      quarantinedSpecs({ CI: "true", GITHUB_EVENT_NAME: "schedule", PURLIS_QUARANTINE_FILE: file }),
    ).toEqual([]);
  });
});
