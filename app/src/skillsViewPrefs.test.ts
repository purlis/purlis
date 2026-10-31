// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DEFAULT_SKILLS_VIEW, SKILLS_VIEW, loadSkillsView } from "./skillsViewPrefs";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";

/**
 * **The Skills view's folded groups, kept for the window** (#3281, #3693): in the layout file
 * under `skills`, read back field by field, so they survive a restart. The view follows the chat
 * in front, so an older purlis's Only this chat's is read past and never written again.
 */

beforeEach(() => forgetThisLaunch());
afterEach(() => {
  Reflect.deleteProperty(globalThis, GLOBAL);
  clearMocks();
});

describe("the Skills view's folded groups", () => {
  it("start with Left out and the built-in skills folded, and the file says nothing of it", () => {
    expect(loadSkillsView({ version: 2, regions: [] })).toEqual({
      value: DEFAULT_SKILLS_VIEW,
      said: [],
    });
    expect(SKILLS_VIEW.value().folded).toEqual(["built-in", "left-out"]);
    expect(SKILLS_VIEW.written()).toBeUndefined();
  });

  it("are written under skills once they differ, and taken out once they are as they start", async () => {
    const written: string[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "write_layout") written.push((args as { text: string }).text);
      return null;
    });
    SKILLS_VIEW.set({ folded: ["built-in"] });
    await expect.poll(() => written.length).toBe(1);
    expect(JSON.parse(written[0]).skills).toEqual({ folded: ["built-in"] });
    SKILLS_VIEW.set({ folded: [] });
    await expect.poll(() => written.length).toBe(2);
    expect(JSON.parse(written[1]).skills).toEqual({ folded: [] });
    SKILLS_VIEW.set(DEFAULT_SKILLS_VIEW);
    await expect.poll(() => written.length).toBe(3);
    expect(JSON.parse(written[2]).skills).toBeUndefined();
  });

  it("come back from the file the window was handed, as a restart reads them", () => {
    Reflect.set(globalThis, GLOBAL, {
      layout: {
        path: "/c/layout.json",
        found: true,
        document: { version: 2, regions: [], skills: { folded: ["needs-you"] } },
        trouble: null,
      },
    });
    expect(SKILLS_VIEW.value()).toEqual({ folded: ["needs-you"] });
  });

  it("read an older purlis's Only this chat's past, and say nothing of it", () => {
    expect(loadSkillsView({ skills: { onlyThisChat: true } })).toEqual({
      value: DEFAULT_SKILLS_VIEW,
      said: [],
    });
    expect(loadSkillsView({ skills: { onlyThisChat: true, folded: ["in-chat"] } })).toEqual({
      value: { folded: ["in-chat"] },
      said: [],
    });
  });

  it("are read as group names and nothing else, and held to a few", () => {
    const { value, said } = loadSkillsView({
      skills: { folded: ["project", 7, "no-such-group", "left-out", "program:Codex", "left-out"] },
    });
    expect(value).toEqual({ folded: ["left-out"] });
    expect(said).toEqual([]);
    // An older purlis's folds of its levels leave the groups folded as they start.
    expect(loadSkillsView({ skills: { folded: ["project", "program:Claude Code"] } })).toEqual({
      value: DEFAULT_SKILLS_VIEW,
      said: [],
    });
    expect(loadSkillsView({ skills: { folded: "project" } })).toEqual({
      value: DEFAULT_SKILLS_VIEW,
      said: [expect.stringContaining('"skills"')],
    });
    expect(loadSkillsView({ skills: "chat" }).value).toEqual(DEFAULT_SKILLS_VIEW);
  });
});
