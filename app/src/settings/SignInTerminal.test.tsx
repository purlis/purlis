// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import type { Terminal } from "@xterm/xterm";
import { DEFAULT_THEME, drawIn, xtermLook, xtermTheme } from "../theme/theme";
import { PURLIS_LIGHT } from "../theme/handed.testkit";
import { SignInTerminal } from "./SignInTerminal";

/**
 * **The strip's terminal follows a theme drawn while it is up** (#4122), as a chat's does. The
 * real xterm: what is asserted is its own `options`, which is what it draws from.
 */
const made: Terminal[] = [];
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

  it("keeps its size: a login's screen is the runner's", () => {
    terminal();

    act(() => drawIn(PURLIS_LIGHT));

    expect([made[0].cols, made[0].rows]).toEqual([80, 16]);
  });

  it("stops following once it is gone", () => {
    const { unmount } = terminal();
    const was = made[0];
    unmount();

    expect(() => drawIn(PURLIS_LIGHT)).not.toThrow();
    expect(was.options.theme).toEqual(xtermTheme(DEFAULT_THEME));
  });
});
