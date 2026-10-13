// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import {
  INBOX_SCOPES,
  keepInboxScope,
  keptInboxScope,
  scopeDrawn,
  widerWaiting,
} from "./inboxScope";
import { forgetProjectViews, keepFacet, keptFacet } from "./projectViews";

afterEach(forgetProjectViews);

describe("the Inbox's scope", () => {
  it("offers every project's, this project's and this workspace's, widest first", () => {
    expect(INBOX_SCOPES.map((one) => one.says)).toEqual([
      "All projects",
      "This project",
      "This workspace",
    ]);
  });

  it("is the project's until the person picks another", () => {
    expect(keptInboxScope("/p")).toBe("project");
    expect(keptInboxScope(undefined)).toBe("project");
  });

  it("keeps a pick for its project, and the default as nothing", () => {
    keepInboxScope("/p", "workspace");
    expect(keptInboxScope("/p")).toBe("workspace");
    expect(keptInboxScope("/other")).toBe("project");
    expect(keptFacet("/p", "inbox")).toEqual({ scope: "workspace" });
    keepInboxScope("/p", "project");
    expect(keptFacet("/p", "inbox")).toBeUndefined();
  });

  it("reads what is kept as untrusted: a value that is no scope is the default", () => {
    for (const held of ["all", 3, null, ["all"], { scope: "everything" }, { scope: 1 }]) {
      keepFacet("/p", "inbox", held);
      expect(keptInboxScope("/p")).toBe("project");
    }
    keepFacet("/p", "inbox", Object.create({ scope: "all" }));
    expect(keptInboxScope("/p")).toBe("project");
  });

  it("is drawn as the project's where the picked one is not offered", () => {
    expect(scopeDrawn("workspace", { project: 2 })).toBe("project");
    expect(scopeDrawn("workspace", { project: 2, workspace: 0 })).toBe("workspace");
    expect(scopeDrawn("all", { project: 2, all: 5 })).toBe("all");
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
