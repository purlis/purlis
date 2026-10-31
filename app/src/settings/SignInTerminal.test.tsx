// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import type { Terminal } from "@xterm/xterm";
import { DEFAULT_THEME, drawIn, xtermLook, xtermTheme } from "../theme/theme";
import { drawTerminalFont } from "../theme/terminalFont";
import { PURLIS_LIGHT } from "../theme/handed.testkit";
import { SignInTerminal } from "./SignInTerminal";

/**
 * **The strip's terminal follows a theme drawn while it is up** (#4122), as a chat's does. The
 * real xterm: what is asserted is its own `options`, which is what it draws from.
 */
const made: Terminal[] = [];
vi.mock("../theme/terminalFont", async (real) => {
  const font = await real<typeof import("../theme/terminalFont")>();
  return { ...font, drawTerminalFont: vi.fn(font.drawTerminalFont) };
});
vi.mock("@xterm/xterm", async (real) => {
  const xterm = await real<typeof import("@xterm/xterm")>();
  class Recorded extends xterm.Terminal {
    constructor(...args: ConstructorParameters<typeof xterm.Terminal>) {
      super(...args);
      made.push(this);
    }
  }
  return { ...xterm, Terminal: Recorded };
});

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

const terminal = () =>
  render(
    <SignInTerminal
      label="The runner's terminal on builds"
      size={{ columns: 80, rows: 16 }}
      printer={() => undefined}
      onType={() => undefined}
      live
    />,
  );

beforeEach(() => {
  made.length = 0;
  drawIn(DEFAULT_THEME);
});
afterEach(() => {
  cleanup();
  drawIn(DEFAULT_THEME);
});

describe("the sign-in strip's terminal and the theme in force", () => {
  it("is handed the new theme and its look when one is drawn while it is up", () => {
    terminal();
    expect(made[0].options.theme).toEqual(xtermTheme(DEFAULT_THEME));

    act(() => drawIn(PURLIS_LIGHT));

    expect(made[0].options.theme).toEqual(xtermTheme(PURLIS_LIGHT));
    expect(made[0].options).toMatchObject(xtermLook(PURLIS_LIGHT));
  });

  it("is told the theme's font", () => {
    terminal();
    vi.mocked(drawTerminalFont).mockClear();

    act(() => drawIn(PURLIS_LIGHT));

    expect(drawTerminalFont).toHaveBeenCalledWith(made[0], expect.any(String), false);
  });

  it("keeps its size: a login's screen is the runner's", () => {
    terminal();

    act(() => drawIn(PURLIS_LIGHT));

    expect([made[0].cols, made[0].rows]).toEqual([80, 16]);
  });

  it("passes on what is typed, and not the keyboard coming and going", async () => {
    const typed: string[] = [];
    render(
      <SignInTerminal
        label="The runner's terminal on builds"
        size={{ columns: 80, rows: 16 }}
        printer={() => undefined}
        onType={(text) => typed.push(text)}
        live
      />,
    );

    made[0].input("\x1b[I");
    made[0].input("\x1b[O");
    made[0].input("1");

    expect(typed).toEqual(["1"]);
  });

  it("stops following once it is gone", () => {
    const { unmount } = terminal();
    const was = made[0];
    unmount();

    expect(() => drawIn(PURLIS_LIGHT)).not.toThrow();
    expect(was.options.theme).toEqual(xtermTheme(DEFAULT_THEME));
  });
});
