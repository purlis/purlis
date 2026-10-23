// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { BUILT_IN_ICONS } from "./icons";
import { BUILT_IN, CLASSIC } from "./theme";

/**
 * The registry's answer to "what has contributed what to this window" has to include the themes
 * purlis itself ships. Those are compiled into the core (`purlis_core::theme::built_ins`, whose
 * tests hold them to `purlis_core::extension::BUILT_IN_THEMES`) and handed to the window as it
 * is created (`BUILT_IN` here). The icon theme is still the window's own bundle's.
 *
 * Lists that must agree, with the agreement tested rather than remembered. Without this, a
 * third built-in would ship, the registry would go on naming two, and the one surface that is
 * supposed to be the whole answer would quietly stop being it — which is the failure mode ADR
 * 0041 builds the registry to prevent, arrived at from the inside.
 */
/// From `app`, which is vitest's working directory here — the same anchor
/// `literals.test.ts` uses to read the real source tree rather than a fixture.
const CORE = join(process.cwd(), "..", "crates", "purlis-core", "src", "extension.rs");

describe("charter's own themes", () => {
  it("are the same two the core's registry names", () => {
    const source = readFileSync(CORE, "utf8");
    expect(source.length, `${CORE} read as empty`).toBeGreaterThan(0);
    const declared = /BUILT_IN_THEMES:\s*\[&str;\s*\d+\]\s*=\s*\[([^\]]*)\]/.exec(source);
    expect(declared, `BUILT_IN_THEMES was not found in ${CORE}`).not.toBeNull();

    const named = [...(declared?.[1] ?? "").matchAll(/"([^"]+)"/g)].map((hit) => hit[1]);
    // Each of the window's handed picks but Classic's own id is one the registry names: the
    // default's dark; Purlis's light the window asks the core for (#3034).
    for (const id of Object.keys(BUILT_IN).filter((one) => one !== CLASSIC))
      expect(named, id).toContain(id);
    expect(named.sort()).toEqual(["purlis-dark", "purlis-light"]);
  });
});

describe("charter's own icon themes (FM-3)", () => {
  it("are the ones the core's registry names", () => {
    const source = readFileSync(CORE, "utf8");
    const declared = /BUILT_IN_ICON_THEMES:\s*\[&str;\s*\d+\]\s*=\s*\[([^\]]*)\]/.exec(source);
    expect(declared, `BUILT_IN_ICON_THEMES was not found in ${CORE}`).not.toBeNull();
    const named = [...(declared?.[1] ?? "").matchAll(/"([^"]+)"/g)].map((hit) => hit[1]);
    expect(named.sort()).toEqual(Object.keys(BUILT_IN_ICONS).sort());
  });
});
