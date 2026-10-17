// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ON_THIS_MACHINE, ONCE_ON_THIS_MACHINE } from "./dismissals";
import { drawWhatIsInForce } from "./Extensions";
import { DEFAULT_THEME, drawIn, inForce } from "./theme/theme";
import {
  aboutThisMachine,
  atCreation,
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
};
/** The person's theme as the core resolves it: complete, and named theirs. */
const MINE = { ...DEFAULT_THEME, name: "Mine", values: { ...DEFAULT_THEME.values } };
MINE.values["surface.base"] = RAISED;

const handed = (theme: Partial<TheirReading>) => {
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: NONE,
    theme: { path: PATH, found: true, trouble: null, theme: null, complaints: [], ...theme },
  };
};

beforeEach(() => {
  Reflect.deleteProperty(globalThis, GLOBAL);
  sayAboutThisMachine("theme", undefined);
  drawIn(DEFAULT_THEME);
});
afterEach(() => {
  clearMocks();
  drawIn(DEFAULT_THEME);
});

describe("what the window was handed as it was created", () => {
  it("is nothing at all when it was handed nothing", () => {
    expect(atCreation()).toEqual({ layout: NONE, theme: NO_THEME });
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

  it("is not repainted when the core answers it again", async () => {
    handed({ theme: MINE });
    // As `main.tsx` draws it: once, so the core answering the same theme repaints nothing.
    const theirs = theirThemeOnce();
    if (theirs === undefined) throw new Error("the operator's theme was not read");
    drawIn(theirs);
    mockIPC((cmd) =>
      cmd === "theme_in_force"
        ? { theme: { ...MINE, values: { ...MINE.values } }, complaints: [], follows_system: false }
        : null,
    );

    await drawWhatIsInForce();

    expect(inForce()).toBe(theirs);
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
