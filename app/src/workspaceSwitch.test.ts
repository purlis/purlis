// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  opensTheWorkspaces,
  pinKeySaid,
  pinOfKey,
  projectsFound,
  sideways,
  stepTarget,
  Swipe,
  workspacesFound,
  workspacesKeySaid,
} from "./workspaceSwitch";

const PINS = ["smart-ide", "charter", "skills"];

describe("a step through the pins (R112, R113)", () => {
  it("goes to the next and the previous pin, round the ends", () => {
    expect(stepTarget(PINS, "smart-ide", 1)).toBe("charter");
    expect(stepTarget(PINS, "skills", 1)).toBe("smart-ide");
    expect(stepTarget(PINS, "smart-ide", -1)).toBe("skills");
    expect(stepTarget(PINS, "charter", -1)).toBe("smart-ide");
  });

  it("goes from a workspace that is not pinned to the first pin, or back to the last", () => {
    expect(stepTarget(PINS, "docs", 1)).toBe("smart-ide");
    expect(stepTarget(PINS, "docs", -1)).toBe("skills");
    expect(stepTarget(PINS, undefined, 1)).toBe("smart-ide");
  });

  it("lands nowhere with no pins, or on the one pin already current", () => {
    expect(stepTarget([], "docs", 1)).toBeUndefined();
    expect(stepTarget(["main"], "main", 1)).toBeUndefined();
    expect(stepTarget(["main"], "docs", -1)).toBe("main");
  });
});

describe("a sideways wheel (R112)", () => {
  it("is a trackpad's sideways travel, or a plain wheel's with Shift held", () => {
    expect(sideways({ deltaX: 12, deltaY: 3, shiftKey: false })).toBe(12);
    expect(sideways({ deltaX: 0, deltaY: 40, shiftKey: true })).toBe(40);
    expect(sideways({ deltaX: -2, deltaY: 1, deltaMode: 1, shiftKey: false })).toBe(-32);
  });

  it("is not a wheel that goes mostly up or down", () => {
    expect(sideways({ deltaX: 3, deltaY: 12, shiftKey: false })).toBeUndefined();
    expect(sideways({ deltaX: 0, deltaY: 0, shiftKey: false })).toBeUndefined();
  });
});

describe("one swipe steps one pin (R112)", () => {
  it("steps once its travel passes 60px, the way it went", () => {
    const swipe = new Swipe();
    expect(swipe.feed(30, 0)).toBeUndefined();
    expect(swipe.feed(30, 10)).toBeUndefined();
    expect(swipe.feed(5, 20)).toBe(1);
    const back = new Swipe();
    expect(back.feed(-61, 0)).toBe(-1);
  });

  it("ignores the rest of the gesture, momentum and all, until the wheel is quiet", () => {
    const swipe = new Swipe();
    expect(swipe.feed(80, 0)).toBe(1);
    // A long trackpad momentum tail, each event well inside the quiet time of the last.
    for (let at = 16; at < 2000; at += 16) expect(swipe.feed(80, at)).toBeUndefined();
    // 220ms of quiet ends it; the next gesture steps again.
    expect(swipe.feed(80, 2000 + 221)).toBe(1);
  });

  it("starts a new count after a quiet that ended a gesture short of a step", () => {
    const swipe = new Swipe();
    expect(swipe.feed(50, 0)).toBeUndefined();
    expect(swipe.feed(50, 500)).toBeUndefined();
  });
});

describe("a pin's key (R111)", () => {
  const key = (key: string, held: Partial<Record<"meta" | "ctrl" | "alt" | "shift", boolean>>) => ({
    key,
    metaKey: held.meta ?? false,
    ctrlKey: held.ctrl ?? false,
    altKey: held.alt ?? false,
    shiftKey: held.shift ?? false,
  });

  it("is ⌘ and a digit on a Mac", () => {
    expect(pinOfKey(key("1", { meta: true }), true)).toBe(1);
    expect(pinOfKey(key("9", { meta: true }), true)).toBe(9);
    expect(pinOfKey(key("3", { ctrl: true }), true)).toBeUndefined();
  });

  it("is Ctrl and a digit elsewhere", () => {
    expect(pinOfKey(key("4", { ctrl: true }), false)).toBe(4);
    expect(pinOfKey(key("4", { meta: true }), false)).toBeUndefined();
  });

  it("is never 0, a letter, or a digit with Shift or Alt held", () => {
    expect(pinOfKey(key("0", { meta: true }), true)).toBeUndefined();
    expect(pinOfKey(key("p", { meta: true }), true)).toBeUndefined();
    expect(pinOfKey(key("2", { meta: true, shift: true }), true)).toBeUndefined();
    expect(pinOfKey(key("2", { meta: true, alt: true }), true)).toBeUndefined();
  });

  it("is said the platform's way", () => {
    expect(pinKeySaid(2, true)).toBe("⌘2");
    expect(pinKeySaid(2, false)).toBe("Ctrl+2");
  });
});

describe("the switcher's filter (R98)", () => {
  const PROJECTS = [
    { name: "charter", workspaces: ["smart-ide", "charter", "skills"] },
    { name: "volaticloud", workspaces: ["console-ux", "platform"] },
    { name: "easydmarc-umbrella", workspaces: ["keycloak", "invites"] },
  ];

  it("keeps every project with nothing typed", () => {
    expect(projectsFound(PROJECTS, " ")).toEqual([0, 1, 2]);
  });

  it("finds a project by its name or by one of its workspaces', case aside", () => {
    expect(projectsFound(PROJECTS, "VOLA")).toEqual([1]);
    expect(projectsFound(PROJECTS, "key")).toEqual([2]);
    expect(projectsFound(PROJECTS, "zzz")).toEqual([]);
  });

  it("narrows a project's workspaces to the ones the typing finds", () => {
    expect(workspacesFound(PROJECTS[0], "sk")).toEqual(["skills"]);
    expect(workspacesFound(PROJECTS[1], "volati")).toEqual(["console-ux", "platform"]);
    expect(workspacesFound(PROJECTS[2], "")).toEqual(["keycloak", "invites"]);
  });
});

describe("the workspace popover's key (the ruling of 2026-10-24)", () => {
  const key = (
    code: string,
    held: Partial<Record<"meta" | "ctrl" | "alt" | "shift", boolean>>,
  ) => ({
    key: "π",
    code,
    metaKey: held.meta ?? false,
    ctrlKey: held.ctrl ?? false,
    altKey: held.alt ?? false,
    shiftKey: held.shift ?? false,
  });

  it("is ⌥⌘P on a Mac, by the physical key", () => {
    expect(opensTheWorkspaces(key("KeyP", { meta: true, alt: true }), true)).toBe(true);
    // ⌘P alone stays the palette's finder.
    expect(opensTheWorkspaces(key("KeyP", { meta: true }), true)).toBe(false);
    expect(opensTheWorkspaces(key("KeyO", { meta: true, alt: true }), true)).toBe(false);
  });

  it("is Ctrl+Alt+Shift+P elsewhere", () => {
    expect(opensTheWorkspaces(key("KeyP", { ctrl: true, alt: true, shift: true }), false)).toBe(
      true,
    );
    expect(opensTheWorkspaces(key("KeyP", { ctrl: true, shift: true }), false)).toBe(false);
  });

  it("is said the platform's way", () => {
    expect(workspacesKeySaid(true)).toBe("⌥⌘P");
    expect(workspacesKeySaid(false)).toBe("Ctrl+Alt+Shift+P");
  });
});
