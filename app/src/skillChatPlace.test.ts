// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { askAChat, chatPlace } from "./skillChanges";

const PLANE = "/Users/someone/purlis-demo";

describe("where a studio chat starts, as its button names it (#5184, #5248)", () => {
  it("names the project for a project's skill, shared or local", () => {
    expect(chatPlace({ at: "project", local: false }, PLANE)).toBe("at purlis-demo");
    expect(chatPlace({ at: "project", local: true }, PLANE)).toBe("at purlis-demo");
  });

  it("names the workspace for a workspace's skill, shared or local", () => {
    expect(chatPlace({ at: "workspace", name: "purlis-demo", local: false }, PLANE)).toBe(
      "at purlis-demo",
    );
    expect(chatPlace({ at: "workspace", name: "purlis-demo", local: true }, PLANE)).toBe(
      "at purlis-demo",
    );
  });

  it("names the persona a persona's skill's chat runs as", () => {
    expect(chatPlace({ at: "persona", name: "steward", local: false }, PLANE)).toBe("as steward");
  });

  it("puts the place between the ask and what it asks for", () => {
    expect(askAChat({ at: "project", local: false }, "write these", PLANE)).toBe(
      "Ask a chat at purlis-demo to write these",
    );
  });
});
