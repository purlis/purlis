// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ON_THIS_MACHINE, ONCE_ON_THIS_MACHINE } from "./dismissals";
import { drawThemeFor, drawWhatIsInForce, forgetExtensionThemes } from "./Extensions";
import type { PlaneId } from "./bindings";
import { DEFAULT_THEME, drawIn, inForce } from "./theme/theme";
import {
  aboutThisMachine,
  atCreation,
  firstFollowsSystem,
  firstFrameOnce,
  forgetTheirTheme,
  GLOBAL,
  sayAboutThisMachine,
  theirTheme,
  theirThemeOnce,
  type Reading,
  type TheirReading,
} from "./windowprefs";

const PATH = "/home/op/.config/charter/theme.json";
/** A colour that is a value, taken from a theme so that no colour is written here. */
const RAISED = DEFAULT_THEME.values["surface.raised"];
const NONE: Reading = { path: "", found: false, document: null, trouble: null };
const NO_THEME: TheirReading = {
  path: "",
  found: false,
  trouble: null,
  theme: null,
  complaints: [],
  floors: null,
  mine: [],
  parts_later: false,
  format_1: false,
  more: null,
};
/** The person's theme as the core resolves it: complete, and named theirs. */
const MINE = { ...DEFAULT_THEME, name: "Mine", values: { ...DEFAULT_THEME.values } };
MINE.values["surface.base"] = RAISED;

const handed = (theme: Partial<TheirReading>) => {
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: NONE,
    theme: {
      path: PATH,
      found: true,
      trouble: null,
      theme: null,
      complaints: [],
      floors: null,
      ...theme,
    },
  };
};

beforeEach(() => {
  Reflect.deleteProperty(globalThis, GLOBAL);
  sayAboutThisMachine("theme", undefined);
  sayAboutThisMachine("consent floors", undefined);
  drawIn(DEFAULT_THEME);
});
afterEach(() => {
  clearMocks();
  drawIn(DEFAULT_THEME);
});

describe("what the window was handed as it was created", () => {
  it("is nothing at all when it was handed nothing", () => {
    expect(atCreation()).toEqual({ layout: NONE, theme: NO_THEME, first: null });
  });

  it("costs the preference and never the window when it is not what the Rust side writes", () => {
    for (const given of [7, "layout", { layout: 3, theme: { found: "yes", trouble: 4 } }]) {
      (globalThis as Record<string, unknown>)[GLOBAL] = given;

      const { layout, theme } = atCreation();
      expect(layout).toEqual(NONE);
      expect(theme.found).toBe(false);
      expect(theme.trouble).toBeNull();
    }
  });
});

describe("the operator's own theme", () => {
  it("is the theme the core resolved, drawn as it was handed", () => {
    handed({ theme: MINE });

    const theirs = theirTheme();

    expect(theirs).toEqual(MINE);
    expect(aboutThisMachine()).toEqual([]);
  });

  it("says in the Inbox what the core had to put right, and where the file is", () => {
    handed({
      theme: MINE,
      complaints: [
        { said: "surface.bass is not a purlis token", token: "surface.bass" },
        { said: "surface.base is not a colour" },
      ],
    });

    expect(theirTheme()).toEqual(MINE);
    const [said] = aboutThisMachine();
    expect(said.subject).toBe("theme");
    expect(said.detail).toContain(PATH);
    expect(said.detail).toContain("surface.bass is not a purlis token; surface.base is");
  });

  it("that breaks a consent floor says so in one Notice, with the core's sentence", () => {
    const floors = "Mine breaks the consent floors (the state marks meet the mark floor)";
    handed({ theme: MINE, floors });

    expect(theirTheme()).toEqual(MINE);
    const [said] = aboutThisMachine();
    expect(said.subject).toBe("consent floors");
    expect(said.detail).toBe(floors);
    expect(said.remedy).toContain(PATH);
    expect(said.builtIn).toBe(true);
  });

  it("that could not be read leaves the built-in, and the Inbox says why", () => {
    handed({ trouble: `${PATH} is not JSON: trailing comma` });

    expect(theirTheme()).toBeUndefined();
    expect(aboutThisMachine()[0].detail).toContain("is not JSON");
    expect(aboutThisMachine()[0].remedy).toContain(PATH);
  });

  it("is not there on a machine with no file, and nothing is said", () => {
    handed({ found: false });

    expect(theirTheme()).toBeUndefined();
    expect(aboutThisMachine()).toEqual([]);
  });

  it("costs nothing but itself when what was handed is not a theme", () => {
    handed({ theme: "Mine" as unknown as TheirReading["theme"], complaints: [7] as never });

    expect(theirTheme()).toBeUndefined();
    expect(atCreation().theme.complaints).toEqual([]);
  });

  it("is drawn again at once for a project no level of which chooses, before the core is asked", async () => {
    handed({ theme: MINE });
    // As `main.tsx` draws it: once, so drawing it again is the same theme and repaints nothing.
    const theirs = theirThemeOnce();
    if (theirs === undefined) throw new Error("the operator's theme was not read");
    drawIn(theirs);
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      return cmd === "theme_levels_adopt" ? [] : null;
    });

    // No `await`: the person's own theme is drawn in the frame of the switch, before the core
    // is asked anything. It is asked after, for a chat's theme draft there alone (#2332), and an
    // answer that names none keeps what is drawn.
    const drawn = drawThemeFor(new Set(), null, { plane: "p-1" as PlaneId });
    expect(inForce()).toBe(theirs);
    await drawn;
    await Promise.resolve();
    expect(inForce()).toBe(theirs);
    forgetExtensionThemes();
  });

  it("asks the core for the person's theme when it was handed before its parts were measured (#4337)", async () => {
    const parts = "@layer parts {}";
    handed({ theme: MINE, parts_later: true });
    const theirs = theirThemeOnce();
    if (theirs === undefined) throw new Error("the operator's theme was not read");
    drawIn(theirs);
    mockIPC((cmd) =>
      cmd === "theme_in_force"
        ? {
            theme: { ...MINE, parts },
            complaints: [],
            follows_system: false,
            floors: null,
            options: [],
          }
        : null,
    );

    await drawThemeFor(new Set(), null, { plane: "p-1" as PlaneId });

    expect(inForce().parts).toBe(parts);
    forgetExtensionThemes();
  });

  it("draws the parts that landed in the frame of a project switch, whatever the core says after (#4337)", async () => {
    const parts = "@layer parts {}";
    handed({ theme: MINE, parts_later: true });
    const theirs = theirThemeOnce();
    if (theirs === undefined) throw new Error("the operator's theme was not read");
    drawIn(theirs);
    let answer = { ...MINE, parts };
    mockIPC((cmd) => {
      return cmd === "theme_in_force"
        ? {
            theme: answer,
            complaints: [],
            follows_system: false,
            floors: null,
            options: [],
          }
        : null;
    });
    await drawThemeFor(new Set(), null, { plane: "p-1" as PlaneId });

    // Away and back. The core is still asked after the frame, for a chat's theme draft (#2332), so
    // it answers with something else: only a window that asked it for the theme would draw that.
    drawIn(DEFAULT_THEME);
    answer = { ...MINE, parts: "@layer parts { /* the core's */ }" };
    const switched = drawThemeFor(new Set(), null, { plane: "p-2" as PlaneId });
    expect(inForce().parts).toBe(parts);
    await switched;
    await Promise.resolve();
    expect(inForce().parts).toBe(parts);
    forgetExtensionThemes();
  });

  it("gives way to the levels with no project in front, where a machine default may choose (#2602)", async () => {
    handed({ theme: MINE });
    drawIn(DEFAULT_THEME);
    const CHOSEN = { ...MINE, name: "Chosen" };
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      return cmd === "theme_in_force"
        ? { theme: CHOSEN, complaints: [], follows_system: false, floors: null, options: [] }
        : null;
    });

    await drawWhatIsInForce();

    expect(asked).toContain("theme_in_force");
    expect(inForce().name).toBe("Chosen");
    forgetExtensionThemes();
  });
});

describe("the first frame, the levels resolved once at creation (#2602, #2582)", () => {
  const frame = (name: string, light: boolean, more: boolean) => ({
    light,
    more,
    theme: { ...MINE, name },
    floors: null,
  });
  const created = (first: unknown) => {
    (globalThis as Record<string, unknown>)[GLOBAL] = { layout: NONE, theme: NO_THEME, first };
    forgetTheirTheme();
  };
  /** The system's appearance and contrast, as `matchMedia` answers them. */
  const system = ({ light = false, more = false }) =>
    vi.stubGlobal("matchMedia", (media: string) => ({
      matches:
        (light && media === "(prefers-color-scheme: light)") ||
        (more && media === "(prefers-contrast: more)"),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));

  afterEach(() => {
    vi.unstubAllGlobals();
    forgetTheirTheme();
    forgetExtensionThemes();
  });

  it("is the frame the core resolved, for the contrast the core read", () => {
    created({ follows_system: false, contrast: "more", frames: [frame("Sharp", false, true)] });
    // The core's native read wins over a page that has not caught up.
    system({ more: false });
    expect(firstFrameOnce()?.theme.name).toBe("Sharp");
    expect(firstFollowsSystem()).toBe(false);
  });

  it("is the page's own prefers-contrast where the core read none", () => {
    const frames = [frame("Plain", false, false), frame("Sharp", false, true)];
    created({ follows_system: false, contrast: null, frames });
    system({ more: true });
    expect(firstFrameOnce()?.theme.name).toBe("Sharp");
    created({ follows_system: false, contrast: null, frames });
    system({ more: false });
    expect(firstFrameOnce()?.theme.name).toBe("Plain");
  });

  it("is the frame for the system's appearance while the theme follows it", () => {
    const frames = [frame("Night", false, false), frame("Day", true, false)];
    created({ follows_system: true, contrast: "standard", frames });
    system({ light: true });
    expect(firstFrameOnce()?.theme.name).toBe("Day");
    expect(firstFollowsSystem()).toBe(true);
    created({ follows_system: true, contrast: "standard", frames });
    system({ light: false });
    expect(firstFrameOnce()?.theme.name).toBe("Night");
  });

  it("reads what the window asks for a frame's parts by, and takes anything else for none (#4410)", () => {
    created({
      follows_system: false,
      contrast: "standard",
      frames: [{ ...frame("Heavy", false, false), parts_later: 3 }],
    });
    expect(firstFrameOnce()?.parts_later).toBe(3);
    forgetTheirTheme();
    created({
      follows_system: false,
      contrast: "standard",
      frames: [{ ...frame("Heavy", false, false), parts_later: "3" }],
    });
    expect(firstFrameOnce()?.parts_later).toBeNull();
  });

  it("says on the first frame which of the person's colours it left out (#3023)", () => {
    const yours = () => aboutThisMachine().filter((one) => one.subject === "your colours");
    const why = "your colour for text.primary is left out: it would put words under their floor";
    created({
      follows_system: false,
      contrast: "standard",
      frames: [{ ...frame("Team", false, false), mine: [why] }],
    });
    expect(firstFrameOnce()?.mine).toEqual([why]);
    expect(yours().map((one) => one.detail)).toEqual([why]);
    expect(yours()[0].settings).toBe("you.colours");

    // A frame that left nothing out says nothing, and an old answer is taken back.
    created({ follows_system: false, contrast: "standard", frames: [frame("Team", false, false)] });
    expect(firstFrameOnce()?.mine).toEqual([]);
    expect(yours()).toEqual([]);
  });

  it("is nothing when nothing was handed, or what was handed is not a frame", () => {
    created(undefined);
    expect(firstFrameOnce()).toBeUndefined();
    created({ follows_system: false, contrast: "standard", frames: [{ theme: "Mine" }] });
    expect(firstFrameOnce()).toBeUndefined();
  });

  it("is not drawn again when the window asks what is in force and is answered the same", async () => {
    created({ follows_system: false, contrast: "standard", frames: [frame("Team", false, false)] });
    const drawn = firstFrameOnce()?.theme;
    if (drawn === undefined) throw new Error("no first frame");
    drawIn(drawn);
    mockIPC((cmd) =>
      cmd === "theme_in_force"
        ? // The core answers a fresh copy of the same theme.
          { theme: { ...drawn }, complaints: [], follows_system: false, floors: null, options: [] }
        : cmd === "theme_levels_adopt"
          ? []
          : null,
    );

    await drawThemeFor(new Set(), "team", { plane: "p-1" as PlaneId });

    expect(inForce()).toBe(drawn);
  });
});

describe("the operator's own theme, for more contrast (#2328)", () => {
  /** The same theme's appearance tagged high contrast, as the core resolves it. */
  const SHARP = { ...MINE, name: "Mine", values: { ...MINE.values } };
  SHARP.values["surface.base"] = DEFAULT_THEME.values["text.primary"];
  const floors = "Mine breaks the consent floors (an ask contrasts with its surface)";

  /** The system asking for more contrast, or not. */
  const asking = (more: boolean) =>
    vi.stubGlobal("matchMedia", (media: string) => ({
      matches: more && media === "(prefers-contrast: more)",
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));

  afterEach(() => {
    vi.unstubAllGlobals();
    forgetTheirTheme();
  });

  it("is its appearance tagged high contrast while the system asks for more, with its floors", () => {
    handed({ theme: MINE, more: { theme: SHARP, floors, mine: [] } });
    asking(true);
    forgetTheirTheme();

    expect(theirThemeOnce()).toEqual(SHARP);
    const [said] = aboutThisMachine();
    expect(said.subject).toBe("consent floors");
    expect(said.detail).toBe(floors);

    asking(false);
    expect(theirThemeOnce()).toEqual(MINE);
  });

  it("is the one theme there is when it has no appearance tagged high contrast", () => {
    handed({ theme: MINE });
    asking(true);
    forgetTheirTheme();

    expect(atCreation().theme.more).toBeNull();
    expect(theirThemeOnce()).toEqual(MINE);
  });
});

describe("what is seen once on this machine (#1501)", () => {
  it("is the same key and the same causes the core keeps", () => {
    // The core drops any other cause under the key, so a cause the window keeps and the core
    // does not would show its Notice again at every launch.
    const core = readFileSync(
      join(process.cwd(), "..", "crates", "purlis-core", "src", "windowprefs.rs"),
      "utf8",
    );
    const key = /pub const ON_THIS_MACHINE: &str = "([^"]*)";/.exec(core)?.[1];
    const list = /pub const ONCE_ON_THIS_MACHINE: &\[&str\] = &\[([^\]]*)\];/.exec(core)?.[1];
    expect(key).toBe(ON_THIS_MACHINE);
    expect([...(list ?? "").matchAll(/"([^"]*)"/g)].map((one) => one[1])).toEqual([
      ...ONCE_ON_THIS_MACHINE,
    ]);
  });
});
