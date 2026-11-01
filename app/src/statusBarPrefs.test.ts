// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  DEFAULT_STATUS_BAR,
  MOST_HIDDEN,
  STATUS_BAR,
  HIDING_REFUSED,
  MOST_ORDERED,
  hideStatusItem,
  loadStatusBar,
  moveStatusItem,
  reordered,
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

describe("the order the person gave the status items (#3030)", () => {
  it("is none until the person moves one", () => {
    expect(STATUS_BAR.value().order).toEqual([]);
    expect(loadStatusBar({ status: { hidden: ["purlis.pin"] } }).prefs.order).toEqual([]);
  });

  it("moves one item past its neighbour and leaves every other where it stands", () => {
    expect(reordered(["a", "b", "c", "d"], ["a", "b", "c", "d"], "c", "b")).toEqual([
      "a",
      "c",
      "b",
      "d",
    ]);
    expect(reordered(["a", "b", "c", "d"], ["a", "b", "c", "d"], "a", "c")).toEqual([
      "b",
      "c",
      "a",
      "d",
    ]);
  });

  it("keeps the place of an item absent today, beside the ones it stood beside", () => {
    // `c` is not on the bar today: `d` going to the front moves nothing around it.
    expect(reordered(["a", "c", "b", "d"], ["a", "b", "d"], "d", "a")).toEqual([
      "d",
      "a",
      "c",
      "b",
    ]);
    expect(reordered(["a", "c", "b", "d"], ["a", "b", "d"], "d", "b")).toEqual([
      "a",
      "c",
      "d",
      "b",
    ]);
  });

  it("takes in what is on the bar and was never listed, where the bar draws it", () => {
    expect(reordered(["z"], ["a", "b", "c"], "c", "b")).toEqual(["a", "c", "b", "z"]);
    expect(reordered(["b"], ["a", "b", "c"], "c", "a")).toEqual(["c", "a", "b"]);
  });

  it("is written to the layout file under status and read back, with what was hidden", async () => {
    const written: string[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "write_layout") written.push((args as { text: string }).text);
      return null;
    });
    hideStatusItem("purlis.pin", true);
    moveStatusItem("purlis.branches", "purlis.todos", ["purlis.todos", "purlis.branches"]);
    const said = () => JSON.parse(written.at(-1) ?? "{}").status;
    await expect.poll(said).toEqual({
      hidden: ["purlis.pin"],
      order: ["purlis.branches", "purlis.todos"],
    });
    const status = said();
    expect(loadStatusBar({ status }).prefs).toEqual(status);
  });

  it("survives an item hidden and shown again", () => {
    mockIPC(() => null);
    moveStatusItem("b", "a", ["a", "b"]);
    hideStatusItem("b", true);
    hideStatusItem("b", false);
    expect(STATUS_BAR.value().order).toEqual(["b", "a"]);
  });

  it("read from the file keeps each id once and says what it could not read", () => {
    const { prefs, said } = loadStatusBar({
      status: { order: ["purlis.pin", 3, "", "purlis.pin", "extension:prs/open"] },
    });
    expect(prefs).toEqual({ hidden: [], order: ["purlis.pin", "extension:prs/open"] });
    expect(said).toHaveLength(2);
    const notAList = loadStatusBar({ status: { hidden: ["purlis.pin"], order: "alphabetical" } });
    expect(notAList.prefs).toEqual({ hidden: ["purlis.pin"], order: [] });
    expect(notAList.said.join()).toContain('"status.order" "alphabetical"');
  });

  it("keeps at most its bound from the file", () => {
    const many = Array.from({ length: MOST_ORDERED + 5 }, (_, at) => `extension:x/${at}`);
    const { prefs, said } = loadStatusBar({ status: { order: many } });
    expect(prefs.order).toHaveLength(MOST_ORDERED);
    expect(said.join()).toContain(`at most ${MOST_ORDERED}`);
  });

  it("at its bound lets go of what the bar does not draw, never of what it does", () => {
    mockIPC(() => null);
    const gone = Array.from({ length: MOST_ORDERED - 1 }, (_, at) => `extension:gone/${at}`);
    STATUS_BAR.set({ hidden: [], order: gone });
    moveStatusItem("b", "a", ["a", "b"]);
    const { order } = STATUS_BAR.value();
    expect(order).toHaveLength(MOST_ORDERED);
    expect(order.indexOf("b")).toBeLessThan(order.indexOf("a"));
    expect(order.indexOf("b")).toBeGreaterThanOrEqual(0);
  });
});
