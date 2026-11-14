// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import {
  INBOX_SCOPES,
  keepInboxScope,
  keepScopeOf,
  keptInboxScope,
  keptScopeOf,
  scopeDrawn,
  widerWaiting,
} from "./inboxScope";
import { forgetProjectViews, keepFacet, keptFacet } from "./projectViews";

afterEach(forgetProjectViews);

describe("the shared scope switch", () => {
  it("says Workspace, Project and All, narrowest first, as Chats and Asks do (ADR 0121, amended 2026-11-12)", () => {
    expect(INBOX_SCOPES.map((one) => one.says)).toEqual(["Workspace", "Project", "All"]);
  });

  it("is every project's until the person picks another (ADR 0121 decision 4)", () => {
    expect(keptInboxScope("/p")).toBe("all");
    expect(keptInboxScope(undefined)).toBe("all");
  });

  it("keeps a pick for its project, and the default as nothing", () => {
    keepInboxScope("/p", "workspace");
    expect(keptInboxScope("/p")).toBe("workspace");
    expect(keptInboxScope("/other")).toBe("all");
    expect(keptFacet("/p", "inbox")).toEqual({ scope: "workspace" });
    keepInboxScope("/p", "project");
    expect(keptFacet("/p", "inbox")).toEqual({ scope: "project" });
    keepInboxScope("/p", "all");
    expect(keptFacet("/p", "inbox")).toBeUndefined();
  });

  it("reads what is kept as untrusted: a value that is no scope is the default", () => {
    for (const held of ["project", 3, null, ["project"], { scope: "everything" }, { scope: 1 }]) {
      keepFacet("/p", "inbox", held);
      expect(keptInboxScope("/p")).toBe("all");
    }
    keepFacet("/p", "inbox", Object.create({ scope: "project" }));
    expect(keptInboxScope("/p")).toBe("all");
  });

  it("leaves the Browsers grid on the project's until the person picks another", () => {
    expect(keptScopeOf("/p", "browsers")).toBe("project");
    keepScopeOf("/p", "browsers", "project");
    expect(keptFacet("/p", "browsers")).toBeUndefined();
  });

  it("is drawn as the project's where the picked one is not offered", () => {
    expect(scopeDrawn("workspace", { project: 2 })).toBe("project");
    expect(scopeDrawn("workspace", { project: 2, workspace: 0 })).toBe("workspace");
    expect(scopeDrawn("all", { project: 2, all: 5 })).toBe("all");
    // Updates has no every project's: the default is drawn as the project's there.
    expect(scopeDrawn("all", { project: 2, workspace: 1 })).toBe("project");
  });

  it("names the nearest wider scope with something waiting", () => {
    expect(widerWaiting("workspace", { all: 5, project: 2, workspace: 0 })).toEqual({
      scope: "project",
      count: 2,
    });
    expect(widerWaiting("workspace", { all: 5, project: 0, workspace: 0 })).toEqual({
      scope: "all",
      count: 5,
    });
    expect(widerWaiting("project", { project: 0 })).toBeUndefined();
    expect(widerWaiting("project", { all: 0, project: 0 })).toBeUndefined();
    expect(widerWaiting("all", { all: 0, project: 0 })).toBeUndefined();
  });
});
