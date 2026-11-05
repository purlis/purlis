// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { askAChat, chatPlace } from "./skillChanges";

describe("where a studio chat starts, as its button names it (#5184)", () => {
  it("names the project's root for a project's skill", () => {
    expect(chatPlace({ at: "project", local: false })).toBe("at the project's root");
  });

  it("names the workspace for a workspace's skill, shared or local", () => {
    expect(chatPlace({ at: "workspace", name: "purlis-demo", local: false })).toBe(
      "at purlis-demo",
    );
    expect(chatPlace({ at: "workspace", name: "purlis-demo", local: true })).toBe("at purlis-demo");
  });

  it("names the persona a persona's skill's chat runs as", () => {
    expect(chatPlace({ at: "persona", name: "steward", local: false })).toBe("as steward");
  });

  it("puts the place between the ask and what it asks for", () => {
    expect(askAChat({ at: "workspace", name: "purlis-demo", local: false }, "write these")).toBe(
      "Ask a chat at purlis-demo to write these",
    );
  });
});
