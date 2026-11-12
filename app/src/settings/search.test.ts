// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { everySetting, found, isSetHere, offeredLevels } from "./search";
import type { FileSetting, SettingsGroup } from "./groups";

/**
 * **One settings search across every level** (#2920, N27; #2814·4): every setting of every level
 * the tab offers, built from each level's own builder over a file nothing has been read into, so
 * the palette and the Settings search list the same settings by the same words without reading a
 * file or asking the core.
 */

afterEach(() => clearMocks());

const ids = (groups: readonly SettingsGroup[]) =>
  groups.flatMap((one) => one.settings.map((setting) => setting.id));

describe("every setting at every level", () => {
  it("offers You alone outside a project, Project in one, and Workspace about one", () => {
    expect(offeredLevels()).toEqual(["you"]);
    expect(offeredLevels("p")).toEqual(["you", "project"]);
    expect(offeredLevels("p", "w")).toEqual(["you", "project", "workspace"]);
    expect(offeredLevels(undefined, "w")).toEqual(["you"]);
  });

  it("lists each level's settings by their addresses, in the level's order", () => {
    const levels = everySetting({ plane: "p", workspace: "w" });

    expect(levels.map((one) => one.level)).toEqual(["you", "project", "workspace"]);
    const [you = [], project = [], workspace = []] = levels.map((one) => ids(one.groups));
    expect(you).toContain("you.text.window");
    expect(project).toContain("project.saving.plane.mode");
    expect(project).toContain("project.sandbox.sandbox.mode");
    expect(workspace).toContain("workspace.live.live");
    // Every setting once, at one address.
    const all = [...you, ...project, ...workspace];
    expect(new Set(all).size).toBe(all.length);
  });

  it("builds without asking the core anything", () => {
    const asked: string[] = [];
    mockIPC((command) => {
      asked.push(command);
      return null;
    });

    everySetting({ plane: "p", workspace: "w" });

    expect(asked).toEqual([]);
  });

  it("holds the Skills groups back while the Skill studio is off", () => {
    const groups = (on: boolean) =>
      everySetting({ plane: "p", workspace: "w", skillStudio: on }).flatMap((one) =>
        one.groups.map((group) => group.id),
      );
    expect(groups(false)).not.toContain("project.skills");
    expect(groups(true)).toContain("project.skills");
  });
});

describe("what a search finds", () => {
  const levels = everySetting({ plane: "p", workspace: "w" });

  it("finds a setting at every level by its label, whatever its case", () => {
    const hits = found(levels, "THEME");
    expect(hits.map((one) => one.level)).toEqual(["you", "project", "workspace"]);
    expect(ids(hits.find((one) => one.level === "workspace")?.groups ?? [])).toContain(
      "workspace.appearance.theme.use",
    );
  });

  it("finds a setting by its help", () => {
    const hits = found(levels, "cloned in this workspace");
    // A group found by its own words keeps every setting it holds.
    expect(hits.flatMap((one) => ids(one.groups))).toEqual([
      "workspace.repos.cloned",
      "workspace.repos.absent",
    ]);
  });

  it("finds a group by the other words it is called (#5688)", () => {
    expect(found(levels, "telemetry").flatMap((one) => one.groups.map((g) => g.id))).toEqual([
      "you.reports",
    ]);
  });

  it("finds a file setting by its key, as Copy key gives it", () => {
    expect(found(levels, "plane.autosave_after").flatMap((one) => ids(one.groups))).toEqual([
      "project.saving.plane.autosave_after",
    ]);
  });

  it("leaves out a level where nothing matches, and finds nothing for words nothing holds", () => {
    expect(found(levels, "window text size").map((one) => one.level)).toEqual(["you"]);
    expect(found(levels, "zzz no such words")).toEqual([]);
  });

  it("leaves every level whole for no words", () => {
    expect(found(levels, "  ")).toEqual(levels);
  });
});

describe("what is set here", () => {
  const setting = (
    everySetting({ plane: "p" }).find((one) => one.level === "project")?.groups ?? []
  )
    .flatMap((one) => one.settings)
    .find((one) => one.id === "project.saving.plane.mode") as FileSetting;
  const file = (which: "shared" | "local", text: string) => ({
    which,
    file: which === "shared" ? "charter.toml" : "charter.local.toml",
    exists: true,
    text,
    refusals: [],
    parsed: true,
    fields:
      text === "" ? [] : [{ path: [{ key: "plane" }, { key: "mode" }], value: "manual", line: 1 }],
    entries: null,
  });

  it("is a file setting this level's files hold a value of", () => {
    expect(
      isSetHere(setting, {
        shared: file("shared", "[plane]\nmode = 'manual'"),
        local: file("local", ""),
      } as never),
    ).toBe(true);
    expect(
      isSetHere(setting, { shared: file("shared", ""), local: file("local", "") } as never),
    ).toBe(false);
  });
});
