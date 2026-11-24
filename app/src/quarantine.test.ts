// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { isQuarantined, quarantineIdOf } from "./quarantine.testkit";

const task = {
  name: "a flaky test",
  file: { name: "src/Thing.test.ts" },
  suite: { name: "a suite", type: "suite", suite: { name: "src/Thing.test.ts", type: "suite" } },
};
const listed = (id: string) => {
  const file = join(mkdtempSync(join(tmpdir(), "quarantine-")), "quarantine.toml");
  writeFileSync(file, `[[test]]\nid = "${id}"\nticket = 7002\nadded = "2026-11-20"\n`);
  return file;
};

describe("a vitest test in the quarantine", () => {
  const id = "src/Thing.test.ts > a suite > a flaky test";

  it("is named by its file, its suites and itself, as a flake of it is", () => {
    expect(
      quarantineIdOf({ ...task, suite: { name: "a suite", type: "suite", suite: undefined } }),
    ).toBe(id);
  });

  it("is skipped by a pull request run, and the queue run", () => {
    for (const event of ["pull_request", "merge_group"]) {
      const env = { CI: "true", GITHUB_EVENT_NAME: event, PURLIS_QUARANTINE_FILE: listed(id) };
      expect(isQuarantined({ ...task, suite: { name: "a suite", type: "suite" } }, env)).toBe(true);
    }
  });

  it("runs in the nightly run", () => {
    const env = { CI: "true", GITHUB_EVENT_NAME: "schedule", PURLIS_QUARANTINE_FILE: listed(id) };
    expect(isQuarantined({ ...task, suite: { name: "a suite", type: "suite" } }, env)).toBe(false);
  });

  it("leaves a test that is not listed alone", () => {
    const env = {
      CI: "true",
      GITHUB_EVENT_NAME: "pull_request",
      PURLIS_QUARANTINE_FILE: listed(id),
    };
    expect(
      isQuarantined(
        { ...task, name: "another test", suite: { name: "a suite", type: "suite" } },
        env,
      ),
    ).toBe(false);
  });
});
