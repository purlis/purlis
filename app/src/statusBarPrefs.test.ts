// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  DEFAULT_STATUS_BAR,
  MOST_HIDDEN,
  STATUS_BAR,
  HIDING_REFUSED,
  hideStatusItem,
  loadStatusBar,
} from "./statusBarPrefs";
import { forgetThisLaunch } from "./regions";
import { aboutThisMachine, GLOBAL } from "./windowprefs";

/**
 * **The status items the person hid** (N28, #2913): kept in the layout file under `status`, by
 * each item's id, bounded, and read back field by field.
 */

beforeEach(() => forgetThisLaunch());
afterEach(() => {
  Reflect.deleteProperty(globalThis, GLOBAL);
  clearMocks();
});

describe("the status items the person hid", () => {
  it("are none until the person hides one", () => {
    expect(loadStatusBar({ version: 2, regions: [] })).toEqual({
      prefs: DEFAULT_STATUS_BAR,
      said: [],
    });
    expect(STATUS_BAR.value().hidden).toEqual([]);
    expect(STATUS_BAR.written()).toBeUndefined();
  });

  it("are written to the layout file under status, and shown again by the same id", async () => {
    const written: string[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "write_layout") written.push((args as { text: string }).text);
      return null;
    });
    hideStatusItem("purlis.todos", true);
    await expect.poll(() => written.length).toBe(1);
    expect(JSON.parse(written[0]).status).toEqual({ hidden: ["purlis.todos"] });
    hideStatusItem("purlis.todos", false);
    await expect.poll(() => written.length).toBe(2);
    expect(JSON.parse(written[1]).status).toBeUndefined();
  });

  it("read from the file keep each id once, and say what they could not read", () => {
    const { prefs, said } = loadStatusBar({
      status: { hidden: ["purlis.pin", 3, "", "purlis.pin", "extension:prs/open"] },
    });
    expect(prefs.hidden).toEqual(["purlis.pin", "extension:prs/open"]);
    expect(said).toHaveLength(2);
    expect(loadStatusBar({ status: "all" }).prefs).toEqual(DEFAULT_STATUS_BAR);
    expect(loadStatusBar({ status: "all" }).said.join()).toContain('"status" "all"');
  });

  it("keep at most their bound", () => {
    const many = Array.from({ length: MOST_HIDDEN + 5 }, (_, at) => `extension:x/${at}`);
    const { prefs, said } = loadStatusBar({ status: { hidden: many } });
    expect(prefs.hidden).toHaveLength(MOST_HIDDEN);
    expect(said.join()).toContain(`at most ${MOST_HIDDEN}`);
  });

  it("say in the Inbox why one past the bound stays shown", () => {
    mockIPC(() => null);
    for (let at = 0; at < MOST_HIDDEN; at += 1) hideStatusItem(`extension:x/${at}`, true);
    hideStatusItem("purlis.todos", true);
    expect(STATUS_BAR.value().hidden).not.toContain("purlis.todos");
    expect(aboutThisMachine().some((one) => one.subject === HIDING_REFUSED)).toBe(true);
    hideStatusItem("extension:x/0", false);
    expect(aboutThisMachine().some((one) => one.subject === HIDING_REFUSED)).toBe(false);
  });
});
