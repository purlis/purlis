// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_THEME,
  apply,
  drawIn,
  facesAfterTheFirstFrame,
  fetchFacesWith,
  forgetFaces,
  holdFacesForTheFirstFrame,
  type Font,
  type Person,
  type Theme,
} from "./theme";

/** Every face made, by the name it was made under and its weight. */
let madeFaces: string[] = [];
/** Every digest asked for. */
let fetched: string[] = [];

class StandInFace {
  constructor(
    readonly family: string,
    readonly bytes: ArrayBuffer,
    readonly descriptors: { weight: string },
  ) {
    madeFaces.push(`${family} ${descriptors.weight}`);
  }
  load() {
    return Promise.resolve(this);
  }
}

beforeEach(() => {
  madeFaces = [];
  fetched = [];
  forgetFaces();
  vi.stubGlobal("FontFace", StandInFace);
  Object.defineProperty(document, "fonts", {
    value: { add: () => undefined },
    configurable: true,
  });
  fetchFacesWith(async (asset) => {
    fetched.push(asset);
    return new ArrayBuffer(8);
  });
});

afterEach(() => {
  fetchFacesWith(undefined);
  vi.unstubAllGlobals();
});

const comfortable: Person = { density: "comfortable", fonts: {} };

function font(name: string, asset: string): Font {
  return {
    family: name,
    stack: `"${name}", monospace`,
    faces: [{ asset, weight: 400, style: "normal" }],
  };
}

function themeWith(fonts: Partial<Record<"ui" | "mono" | "terminal", Font>>): Theme {
  const core = { family: null, stack: "sans-serif", faces: [] };
  return { ...DEFAULT_THEME, fonts: { ui: core, mono: core, terminal: core, ...fonts } };
}

/** Lets the faces' promises settle. */
const settled = () => new Promise((done) => setTimeout(done, 0));

describe("a theme's faces", () => {
  it("wait for the first frame to be on screen, and are asked for then (#3034)", async () => {
    holdFacesForTheFirstFrame();
    const theme = themeWith({ ui: font("purlis-ui-666666666666", "d".repeat(64)) });
    drawIn(theme, document.createElement("div"));
    await settled();
    // Nothing is asked for, decoded or made on the way to the first frame (ADR 0026).
    expect(fetched).toEqual([]);
    expect(madeFaces).toEqual([]);
    facesAfterTheFirstFrame();
    await settled();
    expect(fetched).toEqual(["d".repeat(64)]);
    expect(madeFaces).toEqual(["purlis-ui-666666666666 400"]);
    // Let go once: a later theme's faces are asked for as it is drawn.
    facesAfterTheFirstFrame();
    drawIn(themeWith({ ui: font("purlis-ui-777777777777", "e".repeat(64)) }));
    await settled();
    expect(fetched).toEqual(["d".repeat(64), "e".repeat(64)]);
  });

  it("are made under each slot's name when two slots share one file, which is asked for once", async () => {
    const shared = "a".repeat(64);
    apply(
      themeWith({
        mono: font("purlis-mono-111111111111", shared),
        terminal: font("purlis-terminal-222222222222", shared),
      }),
      document.createElement("div"),
      false,
      comfortable,
    );
    await settled();
    expect(madeFaces.sort()).toEqual([
      "purlis-mono-111111111111 400",
      "purlis-terminal-222222222222 400",
    ]);
    expect(fetched).toEqual([shared]);
  });

  it("are made for a second theme that shares a Regular with the first", async () => {
    const regular = "b".repeat(64);
    const root = document.createElement("div");
    apply(themeWith({ ui: font("purlis-ui-333333333333", regular) }), root, false, comfortable);
    await settled();
    apply(themeWith({ ui: font("purlis-ui-444444444444", regular) }), root, false, comfortable);
    await settled();
    expect(madeFaces).toEqual(["purlis-ui-333333333333 400", "purlis-ui-444444444444 400"]);
    expect(fetched).toEqual([regular]);
    // Drawing the first again makes nothing new.
    apply(themeWith({ ui: font("purlis-ui-333333333333", regular) }), root, false, comfortable);
    await settled();
    expect(madeFaces).toHaveLength(2);
  });

  it("are not asked for a slot the person set", async () => {
    apply(
      themeWith({ ui: font("purlis-ui-555555555555", "c".repeat(64)) }),
      document.createElement("div"),
      false,
      { density: "comfortable", fonts: { ui: '"Mine", sans-serif' } },
    );
    await settled();
    expect(fetched).toEqual([]);
  });
});
