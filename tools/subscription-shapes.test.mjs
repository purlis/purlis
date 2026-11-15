// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The synthetic samples of a subscription-shaped credential (T1a-18, R-711) are labelled with the
// shape the spike recorded. The rules below are the ones
// `tests/fixtures/subscription-shapes/README.md` records, written as plainly as they read there,
// so a sample whose label disagrees with them fails here. T1a-28's classifier loads the same
// samples as its tests. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const SAMPLES = new URL("../tests/fixtures/subscription-shapes/samples.json", import.meta.url);

const CLAUDE_PLAN = "claude-plan";
const CHATGPT_PLAN = "chatgpt-plan";
const NOT_SUBSCRIPTION = "not-subscription";

// The credential values a request carries where the run's token belongs: the token of an
// `Authorization` header whose scheme is `Bearer` (any case), and the whole of an `x-api-key`.
function credentialValues(headers) {
  const values = [];
  for (const [name, value] of headers) {
    const header = name.toLowerCase();
    if (header === "authorization") {
      const bearer = /^bearer +(.*)$/i.exec(value.trim());
      if (bearer) values.push(bearer[1].trim());
    } else if (header === "x-api-key") {
      values.push(value.trim());
    }
  }
  return values;
}

// Rule C1: a Claude plan's OAuth token.
function isClaudePlan(value) {
  return value.startsWith("sk-ant-oat") || value.startsWith("sk-ant-ort");
}

function base64urlJson(segment) {
  if (!/^[A-Za-z0-9_-]+$/.test(segment)) return undefined;
  try {
    const parsed = JSON.parse(Buffer.from(segment, "base64url").toString("utf8"));
    return parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)
      ? parsed
      : undefined;
  } catch {
    return undefined;
  }
}

// Rule C2: a ChatGPT plan's token, a JWT whose payload carries OpenAI's auth claim.
function isChatgptPlan(value) {
  const segments = value.split(".");
  if (segments.length !== 3 || !/^[A-Za-z0-9_-]+$/.test(segments[2])) return false;
  const header = base64urlJson(segments[0]);
  const payload = base64urlJson(segments[1]);
  if (header === undefined || typeof header.alg !== "string" || payload === undefined) return false;
  const claim = payload["https://api.openai.com/auth"];
  return claim !== null && typeof claim === "object" && !Array.isArray(claim);
}

function classify(headers) {
  const values = credentialValues(headers);
  if (values.some(isClaudePlan)) return CLAUDE_PLAN;
  if (values.some(isChatgptPlan)) return CHATGPT_PLAN;
  return NOT_SUBSCRIPTION;
}

function samples() {
  return JSON.parse(readFileSync(SAMPLES, "utf8")).samples;
}

test("subscription_shape_fixtures_classify_as_recorded", () => {
  for (const sample of samples()) {
    assert.equal(classify(sample.headers), sample.label, `sample ${sample.name}`);
  }
});

test("the samples cover each shape and each negative the spike names", () => {
  const all = samples();
  const rules = new Set(all.map((sample) => sample.rule));
  for (const rule of ["C1", "C2", "N1", "N2", "N3", "N4"]) {
    assert.ok(rules.has(rule), `a sample for rule ${rule}`);
  }
  assert.deepEqual(
    new Set(all.map((sample) => sample.label)),
    new Set([CLAUDE_PLAN, CHATGPT_PLAN, NOT_SUBSCRIPTION]),
  );
  const names = all.map((sample) => sample.name);
  assert.equal(new Set(names).size, names.length, "sample names are unique");
});

test("every credential in the samples is marked synthetic", () => {
  for (const sample of samples()) {
    for (const value of credentialValues(sample.headers)) {
      const decoded = value
        .split(".")
        .map((segment) => Buffer.from(segment, "base64url").toString("latin1"))
        .join(".");
      assert.ok(
        value === "" || /SYNTHETIC/i.test(value) || /SYNTHETIC/i.test(decoded) || sample.synthetic,
        `sample ${sample.name} carries a value not marked synthetic`,
      );
    }
  }
});
