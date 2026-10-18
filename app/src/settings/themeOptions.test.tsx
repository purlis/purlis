// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { WindowOption } from "../bindings";
import { BUILT_IN, DEFAULT_THEME, drawIn } from "../theme/theme";
import { OptionsList, useOptionsInForce } from "./themeOptions";

/** The core's answer for a theme drawn with `options`. */
function answering(options: () => WindowOption[]) {
  mockIPC((cmd) => {
    if (cmd === "theme_in_force")
      return { theme: DEFAULT_THEME, complaints: [], follows_system: false, options: options() };
    return null;
  });
}

const BORDERS: WindowOption = {
  id: "borders",
  label: "Borders",
  description: "A rule between panes.",
  kind: "toggle",
  values: [
    { id: "off", label: "Off" },
    { id: "on", label: "On" },
  ],
  value: "on",
  default: "on",
  chosen: false,
};

const ACCENT: WindowOption = {
  id: "accent",
  label: "Accent",
  description: "",
  kind: "choice",
  values: [
    { id: "lavender", label: "Lavender" },
    { id: "teal", label: "Teal" },
  ],
  value: "teal",
  default: "lavender",
  chosen: true,
};

/** The options of the theme drawn, as Settings › Theme's Options row lists them. */
function Row() {
  return <OptionsList options={useOptionsInForce()} ids={{ id: "options" }} />;
}

afterEach(() => {
  cleanup();
  clearMocks();
  drawIn(DEFAULT_THEME);
});

describe("Settings › Theme › Options (#2325, #2331)", () => {
  it("shows each option of the theme drawn, its value and where the value comes from", async () => {
    answering(() => [BORDERS, ACCENT]);
    render(<Row />);
    const borders = await screen.findByText(/^Borders: On/);
    expect(borders.textContent).toContain("Theme default");
    expect(borders.getAttribute("title")).toBe("A rule between panes.");
    const accent = screen.getByText(/^Accent: Teal/);
    expect(accent.textContent).toContain("Chosen");
  });

  it("says when the theme drawn has no options", async () => {
    answering(() => []);
    render(<Row />);
    expect(await screen.findByText("The theme drawn here has no options.")).toBeTruthy();
  });

  it("asks the core again when the window draws another theme", async () => {
    let options: WindowOption[] = [];
    answering(() => options);
    render(<Row />);
    await screen.findByText("The theme drawn here has no options.");
    options = [BORDERS];
    act(() => drawIn(BUILT_IN["purlis-light"]));
    await waitFor(() => expect(screen.getByText(/^Borders: On/)).toBeTruthy());
  });
});
