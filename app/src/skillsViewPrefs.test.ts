// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DEFAULT_SKILLS_VIEW, SKILLS_VIEW, loadSkillsView } from "./skillsViewPrefs";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";

/**
 * **The Skills view's Only this chat's, kept for the window** (#3308): in the layout file under
 * `skills`, read back field by field, so it survives a restart.
 */

beforeEach(() => forgetThisLaunch());
afterEach(() => {
  Reflect.deleteProperty(globalThis, GLOBAL);
  clearMocks();
});

describe("the Skills view's Only this chat's", () => {
  it("is off until the person presses it, and the file says nothing of it then", () => {
    expect(loadSkillsView({ version: 2, regions: [] })).toEqual({
      value: DEFAULT_SKILLS_VIEW,
      said: [],
    });
    expect(SKILLS_VIEW.value().onlyThisChat).toBe(false);
    expect(SKILLS_VIEW.written()).toBeUndefined();
  });

  it("is written to the layout file under skills, and taken out again when it is off", async () => {
    const written: string[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "write_layout") written.push((args as { text: string }).text);
      return null;
    });
    SKILLS_VIEW.set({ onlyThisChat: true });
    await expect.poll(() => written.length).toBe(1);
    expect(JSON.parse(written[0]).skills).toEqual({ onlyThisChat: true });
    SKILLS_VIEW.set({ onlyThisChat: false });
    await expect.poll(() => written.length).toBe(2);
    expect(JSON.parse(written[1]).skills).toBeUndefined();
  });

  it("comes back from the file the window was handed, as a restart reads it", () => {
    Reflect.set(globalThis, GLOBAL, {
      layout: {
        path: "/c/layout.json",
        found: true,
        document: { version: 2, regions: [], skills: { onlyThisChat: true } },
        trouble: null,
      },
    });
    expect(SKILLS_VIEW.value().onlyThisChat).toBe(true);
  });

  it("reads a value it does not know as the whole list, and says so", () => {
    expect(loadSkillsView({ skills: { onlyThisChat: "yes" } })).toEqual({
      value: DEFAULT_SKILLS_VIEW,
      said: [expect.stringContaining('"skills"')],
    });
    expect(loadSkillsView({ skills: "chat" }).value).toEqual(DEFAULT_SKILLS_VIEW);
  });
});
