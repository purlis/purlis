// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { test } from "node:test";
import { annotations, readRecords } from "./deny-annotations.mjs";

const advisory = {
  type: "diagnostic",
  fields: {
    severity: "error",
    code: "unmaintained",
    message: "an advisory was found",
    advisory: { id: "RUSTSEC-2026-0001", package: "some-crate", title: "some-crate is unmaintained" },
    graphs: [{ Krate: { name: "some-crate", version: "1.2.3" } }],
  },
};

test("an advisory is annotated on Cargo.lock with its id, crate and title", () => {
  assert.deepEqual(annotations([advisory]), [
    "::error file=Cargo.lock,title=cargo-deny unmaintained::RUSTSEC-2026-0001 some-crate some-crate is unmaintained",
  ]);
});

test("a licence finding names its crate from the graph, and warnings are left out", () => {
  const licence = {
    type: "diagnostic",
    fields: {
      severity: "error",
      code: "rejected",
      message: "failed to satisfy license requirements",
      graphs: [{ Krate: { name: "odd", version: "0.1.0" } }],
    },
  };
  const warning = { type: "diagnostic", fields: { severity: "warning", code: "duplicate", message: "x" } };
  assert.deepEqual(annotations([licence, warning]), [
    "::error file=Cargo.lock,title=cargo-deny rejected::rejected odd 0.1.0 failed to satisfy license requirements",
  ]);
});

test("an error cargo-deny logged around its findings is annotated too, escaped", () => {
  const log = { type: "log", fields: { level: "ERROR", message: "failed to load advisory database: 100% bad\nline" } };
  assert.deepEqual(annotations([log]), [
    "::error file=Cargo.lock,title=cargo-deny error::failed to load advisory database: 100%25 bad line",
  ]);
});

test("a run with nothing to annotate says so, and lines that are not JSON are skipped", () => {
  const records = readRecords('not json\n{"type":"summary","fields":{}}\n');
  assert.equal(records.length, 1);
  const [line] = annotations(records);
  assert.match(line, /^::error file=Cargo.lock,title=cargo-deny failed with no finding recorded::/);
});

test("past the limit, a notice counts the rest", () => {
  const many = Array.from({ length: 12 }, () => advisory);
  const lines = annotations(many);
  assert.equal(lines.length, 11);
  assert.match(
    lines[10],
    /^::notice title=\+2 more cargo-deny findings::2 more; 12 in all\. The first 10 are annotated; the job's log has every one\.$/,
  );
});
