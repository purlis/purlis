// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { KEPT } from "./dismissals";
import { NOTICE_KINDS, OTHER, aboutOf, kindOf, searchFinds } from "./noticeKinds";

/** **What kind of Notice a cause is** (Q182, Q183): the Notices view's filters, groups and
 *  Dismissed list name each one by it. */
describe("a Notice's kind", () => {
  it("names every family whose dismissal is kept, so a dismissed Notice says what it was", () => {
    for (const family of KEPT) expect(NOTICE_KINDS[family], family).toBeDefined();
  });

  it("groups a family nobody named with the other notices", () => {
    expect(kindOf("clone-vouch:repos/web|github.com/acme/web").many).toBe(
      "Clones made outside purlis",
    );
    expect(kindOf("session-saved")).toBe(OTHER);
  });

  it("says what a dismissed Notice was about: a clone by its folder, else its cause's rest", () => {
    expect(aboutOf("clone-vouch:repos/web|github.com/acme/web")).toBe("repos/web");
    expect(aboutOf("pin-dormant:delta")).toBe("delta");
  });

  it("finds a Notice by every word typed, whatever its case", () => {
    expect(searchFinds("repos/web was not cloned by purlis", "WEB cloned")).toBe(true);
    expect(searchFinds("repos/web was not cloned by purlis", "web api")).toBe(false);
    expect(searchFinds("anything", "  ")).toBe(true);
  });
});
