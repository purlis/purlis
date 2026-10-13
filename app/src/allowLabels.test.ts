// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0
/// <reference types="node" />
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { ALLOW_SAID, KEEP_SAID, NEVER_SAID, SANDBOX_ALLOW_SAID } from "./allowLabels";

/**
 * **The window's Allow labels are the registry's** (#1700). `asking.rs` writes the labels each
 * ask offers; `allowLabels.ts` holds the same words for what the window draws without an ask.
 * This reads the Rust source, builds nothing, and fails when either side changes alone.
 */

const ASKING = resolve(dirname(fileURLToPath(import.meta.url)), "../src-tauri/src/asking.rs");
const source = readFileSync(ASKING, "utf8");

/** The body of `fn name` in `asking.rs`, up to the first line that closes it. */
function body(name: string): string {
  const at = source.indexOf(`fn ${name}(`);
  expect(at, `asking.rs has fn ${name}`).toBeGreaterThanOrEqual(0);
  const end = source.indexOf("\n}\n", at);
  return source.slice(at, end);
}

/** The string an arm of `allow_said` matching `pattern` answers. */
function arm(pattern: string): string {
  const escaped = pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const found = new RegExp(`${escaped}\\s*=>\\s*"([^"]*)"`).exec(body("allow_said"));
  expect(found, `allow_said has an arm ${pattern}`).not.toBeNull();
  return found?.[1] ?? "";
}

/** The value of `const name: &str` in `asking.rs`. */
function constant(name: string): string {
  const found = new RegExp(`const ${name}: &str = "([^"]*)";`).exec(source);
  expect(found, `asking.rs has const ${name}`).not.toBeNull();
  return found?.[1] ?? "";
}

describe("the Allow labels (#1700)", () => {
  it("are the words asking.rs gives a dispatch ask, level by level", () => {
    expect(ALLOW_SAID).toEqual({
      once: arm("GrantLevel::Once"),
      chat: arm("GrantLevel::Chat"),
      you: arm("GrantLevel::You"),
      machine: arm("GrantLevel::Machine"),
      project: arm("GrantLevel::Project"),
    });
  });

  it("are the words asking.rs gives a sandbox ask, whose this-chat Allow says only", () => {
    expect(SANDBOX_ALLOW_SAID).toEqual({
      once: arm("GrantLevel::Once"),
      chat: arm("GrantLevel::Chat if sandbox"),
      you: arm("GrantLevel::You"),
      machine: arm("GrantLevel::Machine"),
      project: arm("GrantLevel::Project"),
    });
    expect(SANDBOX_ALLOW_SAID.chat).not.toBe(ALLOW_SAID.chat);
  });

  it("say Keep blocked and Never for this pair as asking.rs does", () => {
    expect(KEEP_SAID).toBe(constant("KEEP_SAID"));
    expect(NEVER_SAID).toBe(constant("NEVER_SAID"));
  });
});
